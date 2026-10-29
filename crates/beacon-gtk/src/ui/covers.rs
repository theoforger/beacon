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

type Key = (PathBuf, i32);

thread_local! {
    /// Decoded covers; `None` marks one that could not be read.
    static TEXTURES: RefCell<HashMap<Key, Option<gdk::Texture>>> =
        RefCell::new(HashMap::new());
}

fn cached(key: &Key) -> Option<Option<gdk::Texture>> {
    TEXTURES.with_borrow(|cache| cache.get(key).cloned())
}

/// Decode a cover at `size` pixels square on GIO's worker threads.
async fn decode(path: PathBuf, size: i32) -> Option<gdk::Texture> {
    let file = gio::File::for_path(&path);
    let result = async {
        let stream = file.read_future(glib::Priority::LOW).await?;
        gdk_pixbuf::Pixbuf::from_stream_at_scale_future(&stream, size, size, true).await
    }
    .await;
    match result {
        Ok(pixbuf) => Some(texture_from(&pixbuf)),
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

/// A square cover with a placeholder icon when there is no art.
#[derive(Debug, Clone)]
pub struct Cover {
    root: gtk::Overlay,
    picture: gtk::Picture,
    placeholder: gtk::Image,
    size: i32,
    /// The cover this widget should show now; a decode that finishes after
    /// the widget moved on to another cover is dropped.
    wanted: Rc<RefCell<Option<Key>>>,
}

impl Cover {
    pub fn new(size: i32) -> Self {
        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .width_request(size)
            .height_request(size)
            .build();
        let placeholder = gtk::Image::builder()
            .icon_name("com.theoforger.Beacon-symbolic")
            .pixel_size(size / 3)
            .css_classes(["dim-label"])
            .build();
        let root = gtk::Overlay::builder()
            .child(&picture)
            .overflow(gtk::Overflow::Hidden)
            .width_request(size)
            .height_request(size)
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Center)
            .css_classes(["cover"])
            .build();
        root.add_overlay(&placeholder);
        // Decorative: the title next to it says what it is.
        root.set_accessible_role(gtk::AccessibleRole::Presentation);
        Self {
            root,
            picture,
            placeholder,
            size,
            wanted: Rc::default(),
        }
    }

    pub fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    pub fn set(&self, path: Option<&Path>) {
        let scale = self.root.scale_factor().max(1);
        let key = path.map(|p| (p.to_owned(), self.size * scale));
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

    fn show(&self, texture: Option<&gdk::Texture>) {
        self.picture.set_paintable(texture);
        self.placeholder.set_visible(texture.is_none());
        self.root.set_css_classes(if texture.is_some() {
            &["cover"]
        } else {
            &["cover", "cover-placeholder"]
        });
    }
}
