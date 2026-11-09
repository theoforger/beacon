// SPDX-License-Identifier: GPL-3.0-or-later

//! The row menu, built when opened.

use gettextrs::gettext;
use gtk::gio;

use crate::ui::episode_actions;
use crate::ui::models::EpisodeItem;

/// The row menu, worded for the episode's current state.
/// Select only where the page has selection mode; Move Up and Move Down
/// only on a queued row of the Queue view (`movable`).
pub(super) fn menu_for(item: &EpisodeItem, selectable: bool, movable: bool) -> gio::Menu {
    let details = gio::Menu::new();
    details.append(Some(&gettext("Show _Details")), Some("row.details"));
    // Starts selection mode with this episode selected, in place of the
    // HIG's check box on hover (asked by the user, 2026-11-04).
    if selectable {
        details.append(Some(&gettext("_Select")), Some("row.select"));
    }

    let menu = gio::Menu::new();
    menu.append_section(None, &details);
    if movable {
        let moves = gio::Menu::new();
        moves.append(Some(&gettext("_Move Up")), Some("row.move-up"));
        moves.append(Some(&gettext("Move Do_wn")), Some("row.move-down"));
        menu.append_section(None, &moves);
    }
    menu.append_section(None, &episode_actions::menu(item, "row"));
    menu
}

#[cfg(test)]
mod tests {
    use gtk::prelude::*;

    use super::*;
    use crate::ui::episode_actions::{item, items};

    #[test]
    fn the_row_menu_offers_select_after_details() {
        let actions: Vec<_> = items(menu_for(&item(), true, false).upcast_ref())
            .into_iter()
            .map(|(_, action)| action)
            .collect();
        assert_eq!(&actions[..3], ["row.details", "row.select", "row.queue"]);
        let unselectable: Vec<_> = items(menu_for(&item(), false, false).upcast_ref())
            .into_iter()
            .map(|(_, action)| action)
            .collect();
        assert_eq!(&unselectable[..2], ["row.details", "row.queue"]);
    }

    #[test]
    fn queued_rows_offer_moves_after_details() {
        let actions: Vec<_> = items(menu_for(&item(), true, true).upcast_ref())
            .into_iter()
            .map(|(_, action)| action)
            .collect();
        assert_eq!(
            &actions[..5],
            [
                "row.details",
                "row.select",
                "row.move-up",
                "row.move-down",
                "row.queue"
            ]
        );
    }
}
