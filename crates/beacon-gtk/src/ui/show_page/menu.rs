// SPDX-License-Identifier: GPL-3.0-or-later

//! A podcast page's More Options menu, a preview's, and what its list says
//! when no episode shows.

use beacon_core::model::{DownloadFilter, EpisodeFilter, ShowPrefs};
use gettextrs::gettext;
use gtk::gio;
use gtk::prelude::*;

/// What the list says in place of episodes: why none show.
pub(super) fn empty_message(prefs: &ShowPrefs, searching: bool) -> String {
    if searching {
        return gettext("No episodes match your search");
    }
    let only_unplayed =
        prefs.played == EpisodeFilter::Unplayed && prefs.downloaded == DownloadFilter::All;
    if only_unplayed {
        gettext("No unplayed episodes: you are all caught up")
    } else if prefs.is_filtered() {
        gettext("No episodes match these filters")
    } else {
        gettext("This podcast has no episodes yet")
    }
}

/// The page's More Options button, in its header bar.
pub(super) fn more_button() -> gtk::MenuButton {
    let more = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(gettext("More Options"))
        .menu_model(&more_menu(false))
        .build();
    more.update_property(&[gtk::accessible::Property::Label(&gettext("More Options"))]);
    more
}

/// More Options, offering to mark every episode played, or unplayed when
/// every one already is.
pub(super) fn more_menu(all_played: bool) -> gio::Menu {
    let menu = gio::Menu::new();
    let mark_all = if all_played {
        gettext("Mark All as _Unplayed")
    } else {
        gettext("Mark All as _Played")
    };
    menu.append(Some(&mark_all), Some("show.mark-all"));
    // Unavailable when every episode is downloaded.
    menu.append(
        Some(&gettext("_Download All Episodes…")),
        Some("show.download-all"),
    );
    menu.append(Some(&gettext("_Copy Feed Address")), Some("show.copy-feed"));
    menu
}

/// A preview's More Options: the show's page on Apple Podcasts, the link
/// to the store its result came from (unavailable when there is none),
/// and its feed's address.
pub(super) fn preview_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some(&gettext("_View on Apple Podcasts")),
        Some("show.view-in-store"),
    );
    menu.append(Some(&gettext("_Copy Feed Address")), Some("show.copy-feed"));
    menu
}
