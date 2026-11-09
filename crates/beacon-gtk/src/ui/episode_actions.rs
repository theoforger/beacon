// SPDX-License-Identifier: GPL-3.0-or-later

//! What can be done to one episode, offered the same way by a row's menu
//! and the details pane's More Options (which took the place of the pane's
//! toggles, asked by the user 2026-11-08): queue, download, favourite,
//! played and Reset Progress, each worded for the episode's state.

use gettextrs::gettext;
use gtk::{gio, prelude::*};

use crate::ui::models::{EpisodeItem, RowAction};

/// Each action's name in its group, and what it asks for.
const ACTIONS: [(&str, RowAction); 4] = [
    ("queue", RowAction::ToggleQueue),
    ("favorite", RowAction::ToggleFavorite),
    ("played", RowAction::TogglePlayed),
    ("reset", RowAction::ResetProgress),
];

/// The episode's actions as a menu section, for the action group `group`
/// (see [`add_to`]).
pub fn menu(item: &EpisodeItem, group: &str) -> gio::Menu {
    let state = &item.episode.state;
    let action = |name: &str| format!("{group}.{name}");
    let menu = gio::Menu::new();
    menu.append(
        Some(&if item.in_queue {
            gettext("Remove From _Queue")
        } else {
            gettext("Add to _Queue")
        }),
        Some(&action("queue")),
    );
    // Downloads arrive in M5: no action yet, so the item is greyed out.
    menu.append(Some(&gettext("_Download")), Some(&action("download")));
    menu.append(
        Some(&if state.favorite {
            gettext("Remove From _Favorites")
        } else {
            gettext("Add to _Favorites")
        }),
        Some(&action("favorite")),
    );
    menu.append(
        Some(&if state.played {
            gettext("Mark as _Unplayed")
        } else {
            gettext("Mark as _Played")
        }),
        Some(&action("played")),
    );
    // Only where there is progress to reset (asked by the user, 2026-11-08).
    if !state.position.is_zero() {
        menu.append(Some(&gettext("_Reset Progress")), Some(&action("reset")));
    }
    menu
}

/// Add the actions [`menu`] names to `actions`, each calling `run`.
pub fn add_to(actions: &gio::SimpleActionGroup, run: impl Fn(RowAction) + Clone + 'static) {
    for (name, action) in ACTIONS {
        let entry = gio::SimpleAction::new(name, None);
        let run = run.clone();
        entry.connect_activate(move |_, _| run(action));
        actions.add_action(&entry);
    }
}

/// A menu's items as (label, action), sections flattened, for tests.
#[cfg(test)]
pub fn items(menu: &gio::MenuModel) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for i in 0..menu.n_items() {
        if let Some(section) = menu.item_link(i, gio::MENU_LINK_SECTION) {
            found.extend(items(&section));
            continue;
        }
        let get = |key| {
            menu.item_attribute_value(i, key, None)
                .and_then(|v| v.get::<String>())
                .unwrap_or_default()
        };
        found.push((
            get(gio::MENU_ATTRIBUTE_LABEL),
            get(gio::MENU_ATTRIBUTE_ACTION),
        ));
    }
    found
}

#[cfg(test)]
pub fn item() -> EpisodeItem {
    use beacon_core::model::{Episode, EpisodeId, EpisodeInfo, EpisodeState, ShowId};
    EpisodeItem {
        episode: Episode {
            id: EpisodeId(1),
            show_id: ShowId(1),
            info: EpisodeInfo::default(),
            state: EpisodeState::default(),
            added_at: chrono::Utc::now(),
        },
        show_title: String::new(),
        cover: None,
        in_queue: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actions(item: &EpisodeItem) -> Vec<String> {
        items(menu(item, "pane").upcast_ref())
            .into_iter()
            .map(|(_, action)| action)
            .collect()
    }

    #[test]
    fn every_action_named_is_added_but_download() {
        let mut item = item();
        item.episode.state.position = std::time::Duration::from_secs(90);
        let group = gio::SimpleActionGroup::new();
        add_to(&group, |_| {});
        for action in actions(&item) {
            let name = action.strip_prefix("pane.").expect("in the group given");
            assert_eq!(group.has_action(name), name != "download", "{name}");
        }
    }

    #[test]
    fn reset_progress_is_offered_only_with_progress() {
        let mut item = item();
        let resets = |item: &EpisodeItem| actions(item).iter().any(|a| a == "pane.reset");
        assert!(!resets(&item), "nothing to reset");
        item.episode.state.position = std::time::Duration::from_secs(90);
        assert!(resets(&item));
        // Played as well: still the position alone.
        item.episode.state.played = true;
        assert!(resets(&item));
    }
}
