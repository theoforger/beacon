// SPDX-License-Identifier: GPL-3.0-or-later

//! Cover art at the sizes the views draw it. Covers can be several thousand
//! pixels wide, so each is decoded once at the size needed and kept, rather
//! than loaded full size into every row.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use relm4::gtk::{self, gdk, gdk_pixbuf, prelude::*};

thread_local! {
    static TEXTURES: RefCell<HashMap<(PathBuf, i32), Option<gdk::Texture>>> =
        RefCell::new(HashMap::new());
}

/// A cover `size` pixels square, or `None` when there is none or it cannot
/// be read. Scale factor included by the caller.
pub fn texture(path: &Path, size: i32) -> Option<gdk::Texture> {
    TEXTURES.with_borrow_mut(|cache| {
        cache
            .entry((path.to_owned(), size))
            .or_insert_with(|| {
                let pixbuf = gdk_pixbuf::Pixbuf::from_file_at_scale(path, size, size, true);
                match pixbuf {
                    Ok(pixbuf) => Some(texture_from(&pixbuf)),
                    Err(err) => {
                        tracing::debug!("cannot read cover {}: {err}", path.display());
                        None
                    }
                }
            })
            .clone()
    })
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
            .halign(gtk::Align::Center)
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
        }
    }

    pub fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    pub fn set(&self, path: Option<&Path>) {
        let scale = self.root.scale_factor().max(1);
        let texture = path.and_then(|p| texture(p, self.size * scale));
        self.picture.set_paintable(texture.as_ref());
        self.placeholder.set_visible(texture.is_none());
        self.root.set_css_classes(if texture.is_some() {
            &["cover"]
        } else {
            &["cover", "cover-placeholder"]
        });
    }
}
