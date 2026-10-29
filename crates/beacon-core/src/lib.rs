// SPDX-License-Identifier: GPL-3.0-or-later

//! Beacon's core: domain model, storage and services.
//!
//! This crate must never depend on GTK, GLib or relm4. See `CLAUDE.md`.

pub mod browse;
pub mod error;
pub mod model;
pub mod services;
pub mod store;

pub use error::{CoverError, FeedError, LibraryError, StoreError};
