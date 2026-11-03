// SPDX-License-Identifier: GPL-3.0-or-later

//! Cover art at the sizes the views draw it (decoded and kept by
//! `cover_cache`). Decoding never blocks the UI: a cover shows its
//! placeholder until its image is ready. Covers that grow with the Shows grid are decoded
//! again at their new size once the window stops resizing, since a texture
//! drawn much smaller than its pixels looks soft.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use relm4::gtk::{self, gdk, glib, prelude::*};

use crate::ui::cover_cache::{Key, cached, decode, decode_size, device_pixels, store};
use crate::ui::widgets::smooth_texture::SmoothTexture;

thread_local! {
    /// Covers being decoded, with the other covers waiting for the same
    /// image: a long list asks for one show's cover once per row.
    static DECODING: RefCell<HashMap<Key, Vec<WeakCover>>> = RefCell::new(HashMap::new());
}

/// How long the grid must keep a size before its covers are decoded again.
const SETTLE: Duration = Duration::from_millis(150);

/// How a cover is sized.
#[derive(Debug, Clone, Copy)]
enum Mode {
    /// `size` logical pixels square.
    Fixed(i32),
    /// As wide as its column; decoded at the device pixels it is drawn at.
    Filling,
}

/// What a cover shows and wants, shared by its handles.
#[derive(Debug, Default)]
struct State {
    path: RefCell<Option<PathBuf>>,
    /// A filling cover's width in device pixels; 0 until first allocated.
    pixels: Cell<i32>,
    /// The cover this widget should show now; a decode that finishes after
    /// the widget moved on to another cover or size is dropped.
    wanted: RefCell<Option<Key>>,
    /// A decode waiting for the grid to stop resizing.
    settling: RefCell<Option<glib::SourceId>>,
}

/// A square cover with a placeholder icon when there is no art. Art that is
/// not square is cropped to the square.
#[derive(Debug, Clone)]
pub struct Cover {
    root: gtk::Overlay,
    art: gtk::Picture,
    placeholder: gtk::Image,
    mode: Mode,
    state: Rc<State>,
}

/// A cover held without keeping its widgets alive, for its own signal
/// handlers.
#[derive(Debug, Clone)]
struct WeakCover {
    root: glib::WeakRef<gtk::Overlay>,
    art: glib::WeakRef<gtk::Picture>,
    placeholder: glib::WeakRef<gtk::Image>,
    mode: Mode,
    state: Rc<State>,
}

impl WeakCover {
    fn upgrade(&self) -> Option<Cover> {
        Some(Cover {
            root: self.root.upgrade()?,
            art: self.art.upgrade()?,
            placeholder: self.placeholder.upgrade()?,
            mode: self.mode,
            state: self.state.clone(),
        })
    }
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
        Self::build(root, size, Mode::Fixed(size))
    }

    /// A cover as wide as it is given, at least `min` pixels, and as tall as
    /// it is wide, as GNOME Podcasts draws the Shows grid. Put it in a
    /// container that measures height for width at the natural size (a grid
    /// view with a natural `vscroll-policy`).
    pub fn filling(min: i32) -> Self {
        let root = gtk::Overlay::builder()
            .child(&square(min))
            .hexpand(true)
            .build();
        // Draws nothing; it reports the cover's size as the grid changes.
        let probe = gtk::DrawingArea::builder()
            .can_target(false)
            .can_focus(false)
            .build();
        root.add_overlay(&probe);
        let this = Self::build(root, min, Mode::Filling);
        let weak = this.downgrade();
        probe.connect_resize(move |probe, width, _| {
            if let Some(cover) = weak.upgrade() {
                cover.resized(probe, width);
            }
        });
        let weak = this.downgrade();
        probe.connect_scale_factor_notify(move |probe| {
            if let Some(cover) = weak.upgrade() {
                cover.resized(probe, probe.width());
            }
        });
        this
    }

    /// The art and placeholder over `root`'s child, which sets the size: the
    /// art never does, so a 2x texture never doubles the layout.
    fn build(root: gtk::Overlay, size: i32, mode: Mode) -> Self {
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
            mode,
            state: Rc::default(),
        }
    }

    fn downgrade(&self) -> WeakCover {
        WeakCover {
            root: self.root.downgrade(),
            art: self.art.downgrade(),
            placeholder: self.placeholder.downgrade(),
            mode: self.mode,
            state: self.state.clone(),
        }
    }

    pub fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    pub fn set(&self, path: Option<&Path>) {
        self.state.path.replace(path.map(Path::to_owned));
        self.load();
    }

    /// A filling cover was given `width` logical pixels: decode for it, at
    /// once the first time, then when the grid stops resizing.
    fn resized(&self, widget: &impl IsA<gtk::Widget>, width: i32) {
        let scale = widget
            .native()
            .and_then(|native| native.surface())
            .map_or_else(
                || f64::from(widget.scale_factor()),
                |surface| surface.scale(),
            );
        let pixels = device_pixels(width, scale);
        if width <= 0 || pixels == self.state.pixels.get() {
            return;
        }
        let first = self.state.pixels.replace(pixels) == 0;
        if let Some(waiting) = self.state.settling.take() {
            waiting.remove();
        }
        if first {
            self.load();
            return;
        }
        let weak = self.downgrade();
        let id = glib::timeout_add_local_once(SETTLE, move || {
            if let Some(cover) = weak.upgrade() {
                cover.state.settling.take();
                cover.load();
            }
        });
        self.state.settling.replace(Some(id));
    }

    /// Show the wanted cover at the wanted size, decoding it if needed.
    fn load(&self) {
        let path = self.state.path.borrow().clone();
        let key = match (path, self.mode) {
            (None, _) => None,
            (Some(path), Mode::Fixed(size)) => {
                Some((path, decode_size(size, self.root.scale_factor())))
            }
            (Some(path), Mode::Filling) => match self.state.pixels.get() {
                // Not allocated yet: decode once its size is known.
                0 => return,
                pixels => Some((path, pixels)),
            },
        };
        let previous = self.state.wanted.replace(key.clone());
        if previous == key {
            return;
        }
        let Some(key) = key else {
            self.show(None);
            return;
        };
        if let Some(texture) = cached(&key) {
            self.show(texture.as_ref());
            return;
        }
        // A new size of the same cover keeps the old art until the sharper
        // one is ready; a different cover shows the placeholder meanwhile.
        if previous.is_none_or(|(path, _)| path != key.0) {
            self.show(None);
        }
        let this = self.downgrade();
        let already = DECODING.with_borrow_mut(|decoding| match decoding.get_mut(&key) {
            Some(waiting) => {
                waiting.push(this.clone());
                true
            }
            None => {
                decoding.insert(key.clone(), Vec::new());
                false
            }
        });
        if already {
            return;
        }
        let filling = matches!(self.mode, Mode::Filling);
        glib::spawn_future_local(async move {
            let texture = decode(key.0.clone(), key.1).await;
            store(&key, texture.clone(), filling);
            let waiting = DECODING
                .with_borrow_mut(|decoding| decoding.remove(&key))
                .unwrap_or_default();
            for cover in std::iter::once(this).chain(waiting) {
                if let Some(cover) = cover.upgrade()
                    && cover.state.wanted.borrow().as_ref() == Some(&key)
                {
                    cover.show(texture.as_ref());
                }
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
