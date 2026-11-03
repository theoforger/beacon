// SPDX-License-Identifier: GPL-3.0-or-later

//! Decoding cover images and keeping the results. Covers can be several
//! thousand pixels wide, so each is decoded once, on GIO's worker threads,
//! at the size it is drawn, and kept for the session.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use relm4::gtk::{gdk, gdk_pixbuf, gio, glib, prelude::*};

use crate::ui::widgets::smooth_texture::SmoothTexture;

/// A cover image and the pixels it is decoded at.
pub type Key = (PathBuf, i32);

thread_local! {
    /// Decoded covers; `None` marks one that could not be read.
    static TEXTURES: RefCell<HashMap<Key, Option<SmoothTexture>>> =
        RefCell::new(HashMap::new());
    /// The size each grid cover was last decoded at, so a resize replaces
    /// the old texture in `TEXTURES` instead of adding to it.
    static FILLING: RefCell<HashMap<PathBuf, i32>> = RefCell::new(HashMap::new());
}

/// Device pixels to decode a cover drawn `width` logical pixels wide on a
/// screen of `scale`, rounded up to 8 so tiny changes reuse a texture.
pub fn device_pixels(width: i32, scale: f64) -> i32 {
    let pixels = (f64::from(width) * scale).ceil() as i32;
    (pixels + 7) / 8 * 8
}

pub fn cached(key: &Key) -> Option<Option<SmoothTexture>> {
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
pub async fn decode(path: PathBuf, size: i32) -> Option<SmoothTexture> {
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
        Ok(pixbuf) => {
            tracing::debug!("decoded cover {} at {size}px", path.display());
            Some(SmoothTexture::new(texture_from(&pixbuf)))
        }
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
        cache.insert(key.clone(), texture);
    });
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
    fn grid_covers_decode_at_the_device_pixels_they_are_drawn_at() {
        assert_eq!(device_pixels(185, 1.0), 192);
        assert_eq!(device_pixels(185, 2.0), 376);
        // Fractional scaling: 185 × 1.25 = 231.25.
        assert_eq!(device_pixels(185, 1.25), 232);
        assert_eq!(device_pixels(160, 1.0), 160);
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
