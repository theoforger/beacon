// SPDX-License-Identifier: GPL-3.0-or-later

//! The filter menu, as in GNOME Podcasts, on podcast pages (`show.*`
//! actions) and in New (`new.*`): three groups of radio items for the sort
//! order, the played state and the download state. Each group is a stateful
//! action holding the chosen value's name.

use beacon_core::model::{DownloadFilter, EpisodeFilter, ShowPrefs, SortOrder};
use gettextrs::gettext;
use relm4::gtk::{gio, prelude::*};

pub const SORT: &str = "sort";
pub const PLAYED: &str = "played";
pub const DOWNLOADED: &str = "downloaded";

/// The menu, its items naming actions in the group `prefix`.
pub fn menu(prefix: &str) -> gio::Menu {
    let menu = gio::Menu::new();
    for (action, items) in [
        (
            SORT,
            vec![
                (gettext("_Newest First"), "newest"),
                (gettext("_Oldest First"), "oldest"),
            ],
        ),
        (
            PLAYED,
            vec![
                (gettext("_All"), "all"),
                (gettext("_Played"), "played"),
                (gettext("_Unplayed"), "unplayed"),
            ],
        ),
        (
            DOWNLOADED,
            vec![
                // Translators: all episodes, whether downloaded or not.
                (gettext("A_ll"), "all"),
                (gettext("_Downloaded"), "downloaded"),
                (gettext("No_t Downloaded"), "not-downloaded"),
            ],
        ),
    ] {
        let section = gio::Menu::new();
        for (label, value) in items {
            let item = gio::MenuItem::new(Some(&label), None);
            item.set_action_and_target_value(
                Some(&format!("{prefix}.{action}")),
                Some(&value.to_variant()),
            );
            section.append_item(&item);
        }
        menu.append_section(None, &section);
    }
    menu
}

/// The action states that reflect `prefs`.
pub fn states(prefs: &ShowPrefs) -> [(&'static str, &'static str); 3] {
    [
        (
            SORT,
            match prefs.order {
                SortOrder::NewestFirst => "newest",
                SortOrder::OldestFirst => "oldest",
            },
        ),
        (
            PLAYED,
            match prefs.played {
                EpisodeFilter::All => "all",
                EpisodeFilter::Played => "played",
                EpisodeFilter::Unplayed => "unplayed",
            },
        ),
        (
            DOWNLOADED,
            match prefs.downloaded {
                DownloadFilter::All => "all",
                DownloadFilter::Downloaded => "downloaded",
                DownloadFilter::NotDownloaded => "not-downloaded",
            },
        ),
    ]
}

/// `prefs` with the choice `value` made in the group `action`.
pub fn apply(mut prefs: ShowPrefs, action: &str, value: &str) -> ShowPrefs {
    match action {
        SORT => {
            prefs.order = match value {
                "oldest" => SortOrder::OldestFirst,
                _ => SortOrder::NewestFirst,
            }
        }
        PLAYED => {
            prefs.played = match value {
                "played" => EpisodeFilter::Played,
                "unplayed" => EpisodeFilter::Unplayed,
                _ => EpisodeFilter::All,
            }
        }
        DOWNLOADED => {
            prefs.downloaded = match value {
                "downloaded" => DownloadFilter::Downloaded,
                "not-downloaded" => DownloadFilter::NotDownloaded,
                _ => DownloadFilter::All,
            }
        }
        _ => {}
    }
    prefs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_round_trip_through_the_action_states() {
        let prefs = ShowPrefs {
            order: SortOrder::OldestFirst,
            played: EpisodeFilter::Played,
            downloaded: DownloadFilter::NotDownloaded,
        };
        let rebuilt = states(&prefs)
            .into_iter()
            .fold(ShowPrefs::default(), |p, (action, value)| {
                apply(p, action, value)
            });
        assert_eq!(rebuilt, prefs);
    }

    #[test]
    fn one_choice_changes_one_setting() {
        let prefs = apply(ShowPrefs::default(), PLAYED, "unplayed");
        assert_eq!(prefs.played, EpisodeFilter::Unplayed);
        assert_eq!(prefs.order, SortOrder::NewestFirst);
        assert_eq!(prefs.downloaded, DownloadFilter::All);
    }
}
