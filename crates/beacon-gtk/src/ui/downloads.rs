// SPDX-License-Identifier: GPL-3.0-or-later

//! The downloads under way, as the views show them: the app sets them as
//! they progress, and rows and menus read them, since what a row binds
//! (`EpisodeItem`) comes from the library, which knows only finished
//! downloads.

use std::cell::RefCell;
use std::collections::HashMap;

use beacon_core::model::EpisodeId;

thread_local! {
    /// Each download under way, with the share received once its size is
    /// known.
    static ACTIVE: RefCell<HashMap<EpisodeId, Option<f64>>> = RefCell::default();
}

/// The downloads under way now; the rows follow.
pub fn set(active: HashMap<EpisodeId, Option<f64>>) {
    ACTIVE.replace(active);
    crate::ui::widgets::episode_row::show_downloads();
}

/// How far `id`'s download got: `None` when it is not downloading,
/// `Some(None)` while its size is unknown.
pub fn progress(id: EpisodeId) -> Option<Option<f64>> {
    ACTIVE.with_borrow(|active| active.get(&id).copied())
}

pub fn is_downloading(id: EpisodeId) -> bool {
    progress(id).is_some()
}
