// SPDX-License-Identifier: GPL-3.0-or-later

//! What to tell people when something fails. Each error variant has its own
//! translated message; the core's `Display` text is for logs only.

use beacon_core::error::{FeedError, LibraryError, StoreError};
use gettextrs::gettext;

pub fn library(error: &LibraryError) -> String {
    match error {
        LibraryError::Feed(feed) => self::feed(feed),
        LibraryError::Store(store) => self::store(store),
        LibraryError::AlreadySubscribed(_) => gettext("You are already subscribed to this podcast"),
        LibraryError::ShowNotFound(_) => gettext("The podcast is no longer in your library"),
    }
}

pub fn feed(error: &FeedError) -> String {
    match error {
        FeedError::Network(_) => gettext("Could not reach the feed. Check your connection."),
        // Translators: the number is an HTTP status code, such as 404.
        FeedError::Status(code) => gettext("The feed's server answered with error {code}")
            .replace("{code}", &code.to_string()),
        FeedError::Parse(_) => gettext("This address is not a podcast feed"),
    }
}

pub fn store(error: &StoreError) -> String {
    match error {
        StoreError::Sqlite(_) | StoreError::Migration(_) => {
            gettext("Could not read or save your library")
        }
        StoreError::Io(_) => gettext("Could not open your library folder"),
    }
}
