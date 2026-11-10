// SPDX-License-Identifier: GPL-3.0-or-later

//! A row's download under way: a progress bar under the subtitle, as GNOME
//! Podcasts shows one (`download_progress_bar.rs`), in place of the
//! playback progress bar while it shows. Cancel Download is in the row
//! menu, where Download was, rather than a third button on the row.

use gettextrs::gettext;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::EpisodeRow;
use crate::ui::downloads;

impl EpisodeRow {
    /// Show how far the row's episode has downloaded, if it is downloading.
    pub(super) fn show_download(&self) {
        let Some(parts) = self.imp().parts.get() else {
            return;
        };
        let progress = self.episode_id().and_then(downloads::progress);
        match progress {
            Some(fraction) => {
                let bar = parts.download.get_or_init(|| {
                    let bar = gtk::ProgressBar::new();
                    bar.add_css_class("episode-progress");
                    bar.update_property(&[gtk::accessible::Property::Label(&gettext(
                        "Download Progress",
                    ))]);
                    parts.text.append(&bar);
                    bar
                });
                match fraction {
                    Some(fraction) => bar.set_fraction(fraction),
                    None => bar.pulse(),
                }
                bar.set_visible(true);
            }
            None => {
                if let Some(bar) = parts.download.get() {
                    bar.set_visible(false);
                }
            }
        }
        if let Some(played) = parts.progress.get() {
            let partly = parts
                .episode
                .borrow()
                .as_ref()
                .is_some_and(|item| item.episode.state.is_partly_played());
            played.set_visible(partly && progress.is_none());
        }
        self.fit_title();
    }
}
