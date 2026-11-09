// SPDX-License-Identifier: GPL-3.0-or-later

//! Beacon's core: domain model, storage and services.
//!
//! This crate must never depend on GTK or GLib. See `CLAUDE.md`.

pub mod browse;
pub mod error;
pub mod model;
pub mod playback;
pub mod queue;
pub mod services;
pub mod store;
pub mod text;

pub use error::{CoverError, FeedError, LibraryError, PlaybackError, StoreError};
