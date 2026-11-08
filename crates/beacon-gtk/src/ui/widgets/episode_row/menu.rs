// SPDX-License-Identifier: GPL-3.0-or-later

//! The row menu, built when opened.

use gettextrs::gettext;
use relm4::gtk::gio;

use crate::ui::episode_actions;
use crate::ui::models::EpisodeItem;

/// The row menu, worded for the episode's current state.
pub(super) fn menu_for(item: &EpisodeItem) -> gio::Menu {
    let details = gio::Menu::new();
    details.append(Some(&gettext("Show _Details")), Some("row.details"));
    // Starts selection mode with this episode selected, in place of the
    // HIG's check box on hover (asked by the user, 2026-11-04).
    details.append(Some(&gettext("_Select")), Some("row.select"));

    let menu = gio::Menu::new();
    menu.append_section(None, &details);
    menu.append_section(None, &episode_actions::menu(item, "row"));
    menu
}

#[cfg(test)]
mod tests {
    use relm4::gtk::prelude::*;

    use super::*;
    use crate::ui::episode_actions::{item, items};

    #[test]
    fn the_row_menu_offers_select_after_details() {
        let actions: Vec<_> = items(menu_for(&item()).upcast_ref())
            .into_iter()
            .map(|(_, action)| action)
            .collect();
        assert_eq!(&actions[..3], ["row.details", "row.select", "row.queue"]);
    }
}
