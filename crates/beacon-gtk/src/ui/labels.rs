// SPDX-License-Identifier: GPL-3.0-or-later

//! Translated names and icons for each location.

use gettextrs::gettext;

use crate::navigation::Location;

pub fn title(location: Location) -> String {
    match location {
        Location::Shows => gettext("Shows"),
        Location::New => gettext("New"),
        Location::Queue => gettext("Queue"),
        Location::Favorites => gettext("Favorites"),
        Location::Downloaded => gettext("Downloaded"),
        Location::Add => gettext("Add"),
    }
}

pub fn icon(location: Location) -> &'static str {
    match location {
        Location::Shows => "com.theoforger.Beacon-symbolic",
        Location::New => "view-list-bullet-symbolic",
        Location::Queue => "queue-symbolic",
        Location::Favorites => "non-starred-symbolic",
        Location::Downloaded => "folder-download-symbolic",
        Location::Add => "list-add-symbolic",
    }
}
