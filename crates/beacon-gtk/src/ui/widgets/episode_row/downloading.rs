// SPDX-License-Identifier: GPL-3.0-or-later

//! A row's download under way: a progress bar under the subtitle, as GNOME
//! Podcasts shows one (`download_progress_bar.rs`), in place of the
//! playback progress bar while it shows, and how much has downloaded in
//! place of the subtitle. Cancel Download is in the row
//! menu, where Download was, rather than a third button on the row.

use gettextrs::gettext;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::EpisodeRow;
use crate::ui::{downloads, format};

impl EpisodeRow {
    /// Show how far the row's episode has downloaded, if it is downloading.
    pub(super) fn show_download(&self) {
        let Some(parts) = self.imp().parts.get() else {
            return;
        };
        let progress = self.episode_id().and_then(downloads::progress);
        match progress {
            Some(shown) => {
                let bar = parts.download.get_or_init(|| {
                    let bar = gtk::ProgressBar::new();
                    bar.add_css_class("episode-progress");
                    bar.update_property(&[gtk::accessible::Property::Label(&gettext(
                        "Download Progress",
                    ))]);
                    parts.text.append(&bar);
                    bar
                });
                // Activity mode only while the server has not said the
                // size, which the HIG asks to keep short.
                match shown.fraction() {
                    Some(fraction) => bar.set_fraction(fraction),
                    None => bar.pulse(),
                }
                bar.set_visible(true);
                // The bar's text, as the HIG asks ("12.1 of 30 MB
                // downloaded"), where the subtitle was, as GNOME Podcasts
                // puts it beside the subtitle.
                let text = shown.text();
                parts.subtitle.set_label(&text);
                bar.update_property(&[gtk::accessible::Property::Description(&text)]);
            }
            None => {
                if let Some(bar) = parts.download.get() {
                    bar.set_visible(false);
                }
                let subtitle = parts
                    .episode
                    .borrow()
                    .as_ref()
                    .map(|item| format::subtitle(&item.episode));
                if let Some(subtitle) = subtitle {
                    parts.subtitle.set_label(&subtitle);
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
