// SPDX-License-Identifier: GPL-3.0-or-later

//! The filter menu, as in GNOME Podcasts, on podcast pages (`show.*`
//! actions) and in New (`new.*`): three groups of radio items for the sort
//! order, the played state and the download state. Each group is a stateful
//! action holding the chosen value's name.

use beacon_core::model::{DownloadFilter, EpisodeFilter, ShowPrefs, SortOrder};
use gettextrs::gettext;
use relm4::gtk::{self, gio, prelude::*};

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

/// The filter button, as GNOME Podcasts builds it: a menu button in an
/// overlay with an accent dot in its corner while the list differs from the
/// default (any filter, or oldest first).
#[derive(Debug, Clone)]
pub struct FilterButton {
    root: gtk::Overlay,
    button: gtk::MenuButton,
    dot: gtk::Box,
}

impl FilterButton {
    /// A button for the menu whose actions are in the group `prefix`.
    pub fn new(prefix: &str) -> Self {
        let button = gtk::MenuButton::builder()
            .icon_name("funnel-symbolic")
            .menu_model(&menu(prefix))
            .tooltip_text(gettext("Sort and Filter"))
            .build();
        button.update_property(&[gtk::accessible::Property::Label(&gettext(
            "Sort and Filter",
        ))]);
        let dot = gtk::Box::builder()
            .halign(gtk::Align::End)
            .valign(gtk::Align::Start)
            .margin_end(5)
            .margin_top(5)
            .can_focus(false)
            .can_target(false)
            .visible(false)
            .css_classes(["filter-dot"])
            .build();
        let root = gtk::Overlay::builder().child(&button).build();
        root.add_overlay(&dot);
        Self { root, button, dot }
    }

    pub fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    /// Show the dot when `prefs` differ from the default.
    pub fn set_prefs(&self, prefs: &ShowPrefs) {
        let active = is_active(prefs);
        self.dot.set_visible(active);
        // The dot is only seen; say it too.
        if active {
            self.button
                .update_property(&[gtk::accessible::Property::Description(&gettext(
                    "Filters are on",
                ))]);
        } else {
            self.button
                .reset_property(gtk::AccessibleProperty::Description);
        }
    }
}

/// Whether the list differs from the default, as GNOME Podcasts marks it.
pub fn is_active(prefs: &ShowPrefs) -> bool {
    *prefs != ShowPrefs::default()
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

    #[test]
    fn any_change_from_the_default_marks_the_button() {
        assert!(!is_active(&ShowPrefs::default()));
        assert!(is_active(&apply(ShowPrefs::default(), SORT, "oldest")));
        assert!(is_active(&apply(
            ShowPrefs::default(),
            DOWNLOADED,
            "downloaded"
        )));
    }
}
