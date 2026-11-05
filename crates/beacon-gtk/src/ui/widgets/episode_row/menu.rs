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
