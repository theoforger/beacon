// SPDX-License-Identifier: GPL-3.0-or-later

//! Translated names and icons for sections and pages.

use gettextrs::gettext;

use crate::navigation::{Location, Subpage, View};

pub fn title(location: Location) -> String {
    match location {
        Location::Shows => gettext("Shows"),
        Location::New => gettext("New"),
        Location::Queue => gettext("Queue"),
        Location::Favorites => gettext("Favorites"),
        Location::Downloaded => gettext("Downloaded"),
    }
}

/// The content header title.
pub fn view_title(view: View) -> String {
    match view {
        View::Section(location) => title(location),
        View::Subpage(Subpage::Add) => gettext("Add Podcast"),
        // Replaced by the podcast's own title in M3.
        View::Subpage(Subpage::Show(_)) => gettext("Podcast"),
    }
}

pub fn icon(location: Location) -> &'static str {
    match location {
        Location::Shows => "com.theoforger.Beacon-symbolic",
        Location::New => "view-list-bullet-symbolic",
        Location::Queue => "queue-symbolic",
        Location::Favorites => "non-starred-symbolic",
        Location::Downloaded => "folder-download-symbolic",
    }
}
