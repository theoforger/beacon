// SPDX-License-Identifier: GPL-3.0-or-later

//! The row menu, built when opened.

use gettextrs::gettext;
use relm4::gtk::gio;

use crate::ui::models::EpisodeItem;

/// The row menu, worded for the episode's current state.
pub(super) fn menu_for(item: &EpisodeItem) -> gio::Menu {
    let state = &item.episode.state;
    let details = gio::Menu::new();
    details.append(Some(&gettext("Show _Details")), Some("row.details"));
    // Starts selection mode with this episode selected, in place of the
    // HIG's check box on hover (asked by the user, 2026-11-04).
    details.append(Some(&gettext("_Select")), Some("row.select"));

    let changes = gio::Menu::new();
    changes.append(
        Some(&if item.in_queue {
            gettext("Remove From _Queue")
        } else {
            gettext("Add to _Queue")
        }),
        Some("row.queue"),
    );
    // Downloads arrive in M5; the item stays disabled until then.
    changes.append(Some(&gettext("_Download")), Some("row.download"));
    changes.append(
        Some(&if state.favorite {
            gettext("Remove From _Favorites")
        } else {
            gettext("Add to _Favorites")
        }),
        Some("row.favorite"),
    );
    changes.append(
        Some(&if state.played {
            gettext("Mark as _Unplayed")
        } else {
            gettext("Mark as _Played")
        }),
        Some("row.played"),
    );

    let menu = gio::Menu::new();
    menu.append_section(None, &details);
    menu.append_section(None, &changes);
    menu
}

#[cfg(test)]
mod tests {
    use beacon_core::model::{Episode, EpisodeId, EpisodeInfo, EpisodeState, ShowId};
    use chrono::Utc;
    use relm4::gtk::prelude::*;

    use super::*;

    /// The menu's items as (label, action).
    fn items(menu: &gio::MenuModel) -> Vec<(String, String)> {
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

    #[test]
    fn the_row_menu_offers_select_after_details() {
        let item = EpisodeItem {
            episode: Episode {
                id: EpisodeId(1),
                show_id: ShowId(1),
                info: EpisodeInfo::default(),
                state: EpisodeState::default(),
                added_at: Utc::now(),
            },
            show_title: String::new(),
            cover: None,
            in_queue: false,
        };
        let actions: Vec<_> = items(menu_for(&item).upcast_ref())
            .into_iter()
            .map(|(_, action)| action)
            .collect();
        assert_eq!(&actions[..2], ["row.details", "row.select"]);
    }
}
