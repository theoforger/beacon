// SPDX-License-Identifier: GPL-3.0-or-later

//! The downloads under way, as the views show them: the app sets them as
//! they progress, and rows and menus read them, since what a row binds
//! (`EpisodeItem`) comes from the library, which knows only finished
//! downloads.

use std::cell::RefCell;
use std::collections::HashMap;

use beacon_core::model::EpisodeId;
use gettextrs::gettext;
use gtk::glib;

/// How far a download got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Shown {
    pub received: u64,
    /// The whole size, once the server has said it.
    pub total: Option<u64>,
}

impl Shown {
    /// The share received, once the size is known.
    pub fn fraction(self) -> Option<f64> {
        let total = self.total.filter(|total| *total > 0)?;
        // Bytes as floats: exact enough for a progress bar.
        #[allow(clippy::cast_precision_loss)]
        let share = self.received as f64 / total as f64;
        Some(share.min(1.0))
    }

    /// The progress bar's text, as the HIG's Progress Bars page asks:
    /// "12.1 of 30 MB downloaded".
    pub fn text(self) -> String {
        let received = glib::format_size(self.received);
        match self.total {
            // Translators: download progress, such as "12.1 MB of 30.0 MB
            // downloaded".
            Some(total) => gettext("{received} of {total} downloaded")
                .replace("{received}", &received)
                .replace("{total}", &glib::format_size(total)),
            // Translators: download progress while the size is unknown,
            // such as "12.1 MB downloaded".
            None => gettext("{received} downloaded").replace("{received}", &received),
        }
    }
}

thread_local! {
    /// Each download under way.
    static ACTIVE: RefCell<HashMap<EpisodeId, Shown>> = RefCell::default();
}

/// The downloads under way now; the rows follow.
pub fn set(active: HashMap<EpisodeId, Shown>) {
    ACTIVE.replace(active);
    crate::ui::widgets::episode_row::show_downloads();
}

/// How far `id`'s download got, if it is downloading.
pub fn progress(id: EpisodeId) -> Option<Shown> {
    ACTIVE.with_borrow(|active| active.get(&id).copied())
}

pub fn is_downloading(id: EpisodeId) -> bool {
    progress(id).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_reads_as_the_hig_has_it() {
        let shown = Shown {
            received: 12_100_000,
            total: Some(30_000_000),
        };
        assert_eq!(shown.text(), "12.1\u{a0}MB of 30.0\u{a0}MB downloaded");
        assert!((shown.fraction().unwrap() - 0.403).abs() < 0.001);
        let unknown = Shown {
            received: 12_100_000,
            total: None,
        };
        assert_eq!(unknown.text(), "12.1\u{a0}MB downloaded");
        assert_eq!(unknown.fraction(), None);
    }
}
