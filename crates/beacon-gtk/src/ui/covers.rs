// SPDX-License-Identifier: GPL-3.0-or-later

//! Cover art at the sizes the views draw it. Covers can be several thousand
//! pixels wide, so each is decoded once, asynchronously, at the size needed,
//! and kept. Decoding never blocks the UI: a cover shows its placeholder
//! until its image is ready.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use relm4::gtk::{self, gdk, gdk_pixbuf, gio, glib, prelude::*};

use crate::ui::widgets::smooth_texture::SmoothTexture;

type Key = (PathBuf, i32);

thread_local! {
    /// Decoded covers; `None` marks one that could not be read.
    static TEXTURES: RefCell<HashMap<Key, Option<SmoothTexture>>> =
        RefCell::new(HashMap::new());
}

fn cached(key: &Key) -> Option<Option<SmoothTexture>> {
    TEXTURES.with_borrow(|cache| cache.get(key).cloned())
}

/// The width and height to ask the decoder for, so that a `width` by
/// `height` image fills a `size` square once cropped: its shorter side
/// becomes `size` and the other keeps the aspect ratio (-1). With the
/// image's size unknown, it fits inside the square.
fn decode_box(dimensions: Option<(i32, i32)>, size: i32) -> (i32, i32) {
    match dimensions {
        Some((width, height)) if width > height => (-1, size),
        Some((width, height)) if width < height => (size, -1),
        _ => (size, size),
    }
}

/// Decode a cover on GIO's worker threads, at `size` pixels on its shorter
/// side, to be cropped to a square.
async fn decode(path: PathBuf, size: i32) -> Option<SmoothTexture> {
    let file = gio::File::for_path(&path);
    let result = async {
        let dimensions = gdk_pixbuf::Pixbuf::file_info_future(path.clone())
            .await
            .ok()
            .flatten()
            .map(|(_, width, height)| (width, height));
        let (width, height) = decode_box(dimensions, size);
        let stream = file.read_future(glib::Priority::LOW).await?;
        gdk_pixbuf::Pixbuf::from_stream_at_scale_future(&stream, width, height, true).await
    }
    .await;
    match result {
        Ok(pixbuf) => Some(SmoothTexture::new(texture_from(&pixbuf))),
        Err(err) => {
            tracing::debug!("cannot read cover {}: {err}", path.display());
            None
        }
    }
}

fn texture_from(pixbuf: &gdk_pixbuf::Pixbuf) -> gdk::Texture {
    let format = if pixbuf.has_alpha() {
        gdk::MemoryFormat::R8g8b8a8
    } else {
        gdk::MemoryFormat::R8g8b8
    };
    gdk::MemoryTexture::new(
        pixbuf.width(),
        pixbuf.height(),
        format,
        &pixbuf.read_pixel_bytes(),
        pixbuf.rowstride() as usize,
    )
    .upcast()
}

/// Pixels to decode a cover drawn at `size` logical pixels. A widget not yet
/// on screen reports scale 1, so decode at least 2x to stay sharp on HiDPI
/// screens.
fn decode_size(size: i32, scale: i32) -> i32 {
    size * scale.max(2)
}

/// A square cover with a placeholder icon when there is no art. Art that is
/// not square is cropped to the square.
#[derive(Debug, Clone)]
pub struct Cover {
    root: gtk::Overlay,
    art: gtk::Picture,
    placeholder: gtk::Image,
    /// Pixels the art is decoded at, before the screen's scale.
    size: i32,
    /// The cover this widget should show now; a decode that finishes after
    /// the widget moved on to another cover is dropped.
    wanted: Rc<RefCell<Option<Key>>>,
}

impl Cover {
    /// A cover `size` pixels square.
    pub fn new(size: i32) -> Self {
        let base = gtk::Box::builder()
            .width_request(size)
            .height_request(size)
            .build();
        let root = gtk::Overlay::builder()
            .child(&base)
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Center)
            .build();
        Self::build(root, size, size)
    }

    /// A cover as wide as it is given, at least `min` pixels, and as tall as
    /// it is wide, as GNOME Podcasts draws the Shows grid. Its art is
    /// decoded for `max` pixels. Put it in a container that measures height
    /// for width at the natural size (a grid view with a natural
    /// `vscroll-policy`).
    pub fn filling(min: i32, max: i32) -> Self {
        let root = gtk::Overlay::builder()
            .child(&square(min))
            .hexpand(true)
            .build();
        Self::build(root, min, max)
    }

    /// The art and placeholder over `root`'s child, which sets the size: the
    /// art never does, so a 2x texture never doubles the layout.
    fn build(root: gtk::Overlay, size: i32, decode: i32) -> Self {
        let art = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .can_shrink(true)
            .build();
        let placeholder = gtk::Image::builder()
            .icon_name("com.theoforger.Beacon-symbolic")
            .pixel_size(size / 3)
            .css_classes(["dim-label"])
            .build();
        root.add_overlay(&art);
        root.add_overlay(&placeholder);
        root.set_overflow(gtk::Overflow::Hidden);
        // No art until `set` gives some; `set(None)` changes nothing, so a
        // show without a cover keeps this.
        root.set_css_classes(&["cover", "cover-placeholder"]);
        // Decorative: the title next to it says what it is.
        root.set_accessible_role(gtk::AccessibleRole::Presentation);
        Self {
            root,
            art,
            placeholder,
            size: decode,
            wanted: Rc::default(),
        }
    }

    pub fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    pub fn set(&self, path: Option<&Path>) {
        let key = path.map(|p| {
            (
                p.to_owned(),
                decode_size(self.size, self.root.scale_factor()),
            )
        });
        if *self.wanted.borrow() == key {
            return;
        }
        self.wanted.replace(key.clone());
        let Some(key) = key else {
            self.show(None);
            return;
        };
        if let Some(texture) = cached(&key) {
            self.show(texture.as_ref());
            return;
        }
        self.show(None);
        let this = self.clone();
        glib::spawn_future_local(async move {
            let texture = decode(key.0.clone(), key.1).await;
            TEXTURES.with_borrow_mut(|cache| cache.insert(key.clone(), texture.clone()));
            if this.wanted.borrow().as_ref() == Some(&key) {
                this.show(texture.as_ref());
            }
        });
    }

    fn show(&self, texture: Option<&SmoothTexture>) {
        self.art.set_paintable(texture);
        self.placeholder.set_visible(texture.is_none());
        self.root.set_css_classes(if texture.is_some() {
            &["cover"]
        } else {
            &["cover", "cover-placeholder"]
        });
    }
}

/// An empty widget as wide as it is given, at least `min` pixels, and as
/// tall as it is wide: a `GtkPicture` of a square paintable measures its
/// height for its width.
pub fn square(min: i32) -> gtk::Picture {
    let pixel = gdk::MemoryTexture::new(
        1,
        1,
        gdk::MemoryFormat::R8g8b8a8,
        &glib::Bytes::from_static(&[0, 0, 0, 0]),
        4,
    );
    gtk::Picture::builder()
        .paintable(&pixel)
        .content_fit(gtk::ContentFit::Fill)
        .can_shrink(true)
        .width_request(min)
        .hexpand(true)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_decode_sharp_enough_before_the_scale_is_known() {
        // Unrealized widgets report scale 1; HiDPI screens need at least 2x.
        assert_eq!(decode_size(48, 1), 96);
        assert_eq!(decode_size(48, 2), 96);
        assert_eq!(decode_size(48, 3), 144);
        assert_eq!(decode_size(160, 0), 320);
    }

    #[test]
    fn covers_decode_with_the_shorter_side_filling_the_square() {
        // Wide and tall art keeps its aspect ratio, short side at `size`, so
        // cropping to the square never scales it up.
        assert_eq!(decode_box(Some((1920, 1080)), 96), (-1, 96));
        assert_eq!(decode_box(Some((600, 900)), 96), (96, -1));
        assert_eq!(decode_box(Some((3000, 3000)), 96), (96, 96));
        assert_eq!(decode_box(None, 96), (96, 96));
    }
}
