// SPDX-License-Identifier: GPL-3.0-or-later

//! The filter menu, as in GNOME Podcasts, on podcast pages and in New: three
//! groups of radio items for the sort order, the played state and the
//! download state, then Reset. Each group is a stateful action holding the
//! chosen value's name; the button owns the `filter.*` actions, as GNOME
//! Podcasts' `FilterMenu` does.

use std::rc::Rc;

use beacon_core::model::{DownloadFilter, EpisodeFilter, ShowPrefs, SortOrder};
use gettextrs::gettext;
use gtk::{self, gio, glib, prelude::*};

pub const SORT: &str = "sort";
pub const PLAYED: &str = "played";
pub const DOWNLOADED: &str = "downloaded";
const RESET: &str = "reset";
const PREFIX: &str = "filter";

/// A choice made in the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// `value` chosen in the group `action`.
    Set(&'static str, String),
    /// Back to Newest First with no filters.
    Reset,
}

impl Choice {
    /// `prefs` with this choice made.
    pub fn apply(&self, prefs: ShowPrefs) -> ShowPrefs {
        match self {
            Choice::Set(action, value) => apply(prefs, action, value),
            Choice::Reset => ShowPrefs::default(),
        }
    }
}

/// The menu, its items naming the `filter.*` actions.
fn menu() -> gio::Menu {
    let menu = gio::Menu::new();
    for (action, title, items) in [
        (
            SORT,
            // Translators: a section title in the Sort and Filter menu.
            gettext("Order"),
            vec![
                (gettext("_Newest First"), "newest"),
                (gettext("_Oldest First"), "oldest"),
            ],
        ),
        (
            PLAYED,
            // Translators: a section title in the Sort and Filter menu, over
            // All, Played and Unplayed.
            gettext("Progress"),
            vec![
                (gettext("_All"), "all"),
                (gettext("_Played"), "played"),
                (gettext("_Unplayed"), "unplayed"),
            ],
        ),
        (
            DOWNLOADED,
            // Translators: a section title in the Sort and Filter menu, over
            // All, Downloaded and Not Downloaded.
            gettext("Downloads"),
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
                Some(&format!("{PREFIX}.{action}")),
                Some(&value.to_variant()),
            );
            section.append_item(&item);
        }
        // Titled, so the two "All" items can be told apart, by eye and by
        // screen reader.
        menu.append_section(Some(&title), &section);
    }
    let reset = gio::Menu::new();
    // Translators: in the Sort and Filter menu; back to Newest First with no
    // filters.
    reset.append(
        Some(&gettext("_Reset Sort and Filters")),
        Some(&format!("{PREFIX}.{RESET}")),
    );
    menu.append_section(None, &reset);
    menu
}

/// The filter button, as GNOME Podcasts builds it: a menu button in an
/// overlay with an accent dot in its corner while the list differs from the
/// default (any filter, or oldest first). Reset is available only then.
#[derive(Debug, Clone)]
pub struct FilterButton {
    root: gtk::Overlay,
    button: gtk::MenuButton,
    dot: gtk::Box,
    choices: [gio::SimpleAction; 3],
    reset: gio::SimpleAction,
}

impl FilterButton {
    /// A button that reports each choice to `on_choice`. Its radio items
    /// follow [`FilterButton::set_prefs`], not the choices themselves.
    pub fn new(on_choice: impl Fn(Choice) + 'static) -> Self {
        let on_choice: Rc<dyn Fn(Choice)> = Rc::new(on_choice);
        let button = gtk::MenuButton::builder()
            .icon_name("funnel-symbolic")
            .menu_model(&menu())
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

        let actions = gio::SimpleActionGroup::new();
        let choices = [SORT, PLAYED, DOWNLOADED].map(|name| {
            let action = gio::SimpleAction::new_stateful(
                name,
                Some(glib::VariantTy::STRING),
                &"".to_variant(),
            );
            let on_choice = on_choice.clone();
            action.connect_activate(move |action, target| {
                if let Some(value) = target.and_then(|t| t.get::<String>()) {
                    action.set_state(&value.to_variant());
                    on_choice(Choice::Set(name, value));
                }
            });
            actions.add_action(&action);
            action
        });
        let reset = gio::SimpleAction::new(RESET, None);
        reset.set_enabled(false);
        reset.connect_activate(move |_, _| on_choice(Choice::Reset));
        actions.add_action(&reset);
        root.insert_action_group(PREFIX, Some(&actions));

        Self {
            root,
            button,
            dot,
            choices,
            reset,
        }
    }

    pub fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    /// Check the items that match `prefs`, and show the dot and offer Reset
    /// when they differ from the default.
    pub fn set_prefs(&self, prefs: &ShowPrefs) {
        for (choice, (_, value)) in self.choices.iter().zip(states(prefs)) {
            choice.set_state(&value.to_variant());
        }
        let active = is_active(prefs);
        self.dot.set_visible(active);
        self.reset.set_enabled(active);
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
    fn reset_returns_to_the_default() {
        let prefs = ShowPrefs {
            order: SortOrder::OldestFirst,
            played: EpisodeFilter::Unplayed,
            downloaded: DownloadFilter::Downloaded,
        };
        assert_eq!(Choice::Reset.apply(prefs), ShowPrefs::default());
        assert_eq!(
            Choice::Set(SORT, "oldest".into()).apply(ShowPrefs::default()),
            apply(ShowPrefs::default(), SORT, "oldest")
        );
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
