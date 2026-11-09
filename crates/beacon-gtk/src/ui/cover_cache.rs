// SPDX-License-Identifier: GPL-3.0-or-later

//! Decoding cover images and keeping the results. Covers can be several
//! thousand pixels wide, so each is decoded once, on GIO's worker threads,
//! at the size it is drawn, and kept while there is room: the cache holds
//! up to 64MB of pixels, dropping the covers used longest ago first.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::path::{Path, PathBuf};

use gtk::{gdk, gdk_pixbuf, gio, glib, prelude::*};

use crate::ui::widgets::smooth_texture::SmoothTexture;

/// A cover image and the pixels it is decoded at.
pub type Key = (PathBuf, i32);

/// Pixels kept, in bytes: hundreds of row covers and a few dozen large
/// ones. Covers on screen stay alive in their widgets whatever the cache
/// drops; a dropped one is decoded again if shown again.
const BYTES_KEPT: usize = 64 * 1024 * 1024;

/// A cache that keeps its most recently used entries up to a size in
/// bytes.
#[derive(Debug)]
struct Bounded<K, V> {
    entries: HashMap<K, (V, usize)>,
    /// Least recently used first.
    order: VecDeque<K>,
    bytes: usize,
    limit: usize,
}

impl<K: Eq + Hash + Clone, V> Bounded<K, V> {
    fn new(limit: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            limit,
        }
    }

    /// The entry for `key`, which becomes the most recently used.
    fn get(&mut self, key: &K) -> Option<&V> {
        if self.entries.contains_key(key)
            && let Some(at) = self.order.iter().position(|k| k == key)
            && let Some(used) = self.order.remove(at)
        {
            self.order.push_back(used);
        }
        self.entries.get(key).map(|(value, _)| value)
    }

    fn insert(&mut self, key: K, value: V, bytes: usize) {
        self.remove(&key);
        self.entries.insert(key.clone(), (value, bytes));
        self.order.push_back(key);
        self.bytes += bytes;
        // Always keep the newest, however large.
        while self.bytes > self.limit && self.order.len() > 1 {
            if let Some(oldest) = self.order.pop_front()
                && let Some((_, freed)) = self.entries.remove(&oldest)
            {
                self.bytes -= freed;
            }
        }
    }

    fn remove(&mut self, key: &K) {
        if let Some((_, freed)) = self.entries.remove(key) {
            self.bytes -= freed;
            self.order.retain(|k| k != key);
        }
    }
}

thread_local! {
    /// Decoded covers; `None` marks one that could not be read.
    static TEXTURES: RefCell<Bounded<Key, Option<SmoothTexture>>> =
        RefCell::new(Bounded::new(BYTES_KEPT));
    /// The size each grid cover was last decoded at, so a resize replaces
    /// the old texture in `TEXTURES` instead of adding to it.
    static FILLING: RefCell<HashMap<PathBuf, i32>> = RefCell::new(HashMap::new());
}

/// Device pixels to decode a cover drawn `width` logical pixels wide on a
/// screen of `scale`: exactly those, since even a few percent of scaling
/// when drawn softens the whole image.
pub fn device_pixels(width: i32, scale: f64) -> i32 {
    (f64::from(width) * scale).round() as i32
}

pub fn cached(key: &Key) -> Option<Option<SmoothTexture>> {
    TEXTURES.with_borrow_mut(|cache| cache.get(key).cloned())
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

/// Decoded pixels, which, unlike a `Pixbuf`, can leave the worker thread.
struct Pixels {
    width: i32,
    height: i32,
    rowstride: usize,
    alpha: bool,
    bytes: glib::Bytes,
}

/// Decode a cover at `size` pixels on its shorter side, in one pass: checked
/// against a Lanczos resize, this came closer than finishing a 2x decode
/// with gdk-pixbuf's high quality scaler.
fn decode_pixels(path: &Path, size: i32) -> Result<Pixels, glib::Error> {
    let dimensions = gdk_pixbuf::Pixbuf::file_info(path).map(|(_, width, height)| (width, height));
    let (width, height) = decode_box(dimensions, size);
    let pixbuf = gdk_pixbuf::Pixbuf::from_file_at_scale(path, width, height, true)?;
    Ok(Pixels {
        width: pixbuf.width(),
        height: pixbuf.height(),
        rowstride: pixbuf.rowstride() as usize,
        alpha: pixbuf.has_alpha(),
        bytes: pixbuf.read_pixel_bytes(),
    })
}

/// Decode a cover on GIO's worker threads, at `size` pixels on its shorter
/// side, to be cropped to a square.
pub async fn decode(path: PathBuf, size: i32) -> Option<SmoothTexture> {
    let source = path.clone();
    match gio::spawn_blocking(move || decode_pixels(&source, size)).await {
        Ok(Ok(pixels)) => {
            tracing::debug!("decoded cover {} at {size}px", path.display());
            Some(SmoothTexture::new(texture_from(&pixels)))
        }
        Ok(Err(err)) => {
            tracing::debug!("cannot read cover {}: {err}", path.display());
            None
        }
        Err(_) => None,
    }
}

fn texture_from(pixels: &Pixels) -> gdk::Texture {
    let format = if pixels.alpha {
        gdk::MemoryFormat::R8g8b8a8
    } else {
        gdk::MemoryFormat::R8g8b8
    };
    gdk::MemoryTexture::new(
        pixels.width,
        pixels.height,
        format,
        &pixels.bytes,
        pixels.rowstride,
    )
    .upcast()
}

/// Pixels to decode a cover drawn at `size` logical pixels. A widget not yet
/// on screen reports scale 1, so decode at least 2x to stay sharp on HiDPI
/// screens.
pub fn decode_size(size: i32, scale: i32) -> i32 {
    size * scale.max(2)
}

/// Keep a decoded cover. A grid cover's new size replaces its old one.
pub fn store(key: &Key, texture: Option<SmoothTexture>, filling: bool) {
    TEXTURES.with_borrow_mut(|cache| {
        if filling {
            let old = FILLING.with_borrow_mut(|sizes| sizes.insert(key.0.clone(), key.1));
            if let Some(old) = old.filter(|old| *old != key.1) {
                cache.remove(&(key.0.clone(), old));
            }
        }
        // A square of RGBA pixels, about; a failed decode costs nothing.
        let side = usize::try_from(key.1).unwrap_or(0);
        let bytes = if texture.is_some() {
            side * side * 4
        } else {
            0
        };
        cache.insert(key.clone(), texture, bytes);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cache_drops_its_oldest_entries_past_its_size() {
        let mut cache = Bounded::new(100);
        cache.insert("a", 1, 40);
        cache.insert("b", 2, 40);
        cache.insert("c", 3, 40);
        assert_eq!(cache.get(&"a"), None);
        assert_eq!(cache.get(&"b").copied(), Some(2));
        assert_eq!(cache.get(&"c").copied(), Some(3));
        assert_eq!(cache.bytes, 80);

        // Replacing an entry frees what it held, and makes it the newest.
        cache.insert("b", 4, 40);
        cache.insert("d", 5, 40);
        assert_eq!(cache.get(&"c"), None);
        assert_eq!(cache.get(&"b").copied(), Some(4));
        assert_eq!(cache.get(&"d").copied(), Some(5));

        // Using an entry keeps it: the one used longest ago goes instead.
        assert_eq!(cache.get(&"b"), Some(&4));
        cache.insert("e", 7, 40);
        assert_eq!(cache.get(&"d"), None);
        assert_eq!(cache.get(&"b"), Some(&4));

        // The newest stays even when it alone is over the size.
        cache.insert("huge", 6, 500);
        assert_eq!(cache.get(&"huge"), Some(&6));
        assert_eq!(cache.order.len(), 1);
    }

    #[test]
    fn covers_decode_sharp_enough_before_the_scale_is_known() {
        // Unrealized widgets report scale 1; HiDPI screens need at least 2x.
        assert_eq!(decode_size(48, 1), 96);
        assert_eq!(decode_size(48, 2), 96);
        assert_eq!(decode_size(48, 3), 144);
        assert_eq!(decode_size(160, 0), 320);
    }

    #[test]
    fn grid_covers_decode_at_the_device_pixels_they_are_drawn_at() {
        assert_eq!(device_pixels(185, 1.0), 185);
        assert_eq!(device_pixels(185, 2.0), 370);
        // Fractional scaling: 185 × 1.25 = 231.25.
        assert_eq!(device_pixels(185, 1.25), 231);
        assert_eq!(device_pixels(160, 1.5), 240);
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
