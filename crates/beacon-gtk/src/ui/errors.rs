// SPDX-License-Identifier: GPL-3.0-or-later

//! What to tell people when something fails. Each error variant has its own
//! translated message; the core's `Display` text is for logs only.

use beacon_core::error::{DownloadError, FeedError, LibraryError, PlaybackError, StoreError};
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
        FeedError::Network(_) => gettext("Could not reach the feed"),
        // Translators: the number is an HTTP status code, such as 404.
        FeedError::Status(code) => gettext("The feed's server answered with error {code}")
            .replace("{code}", &code.to_string()),
        FeedError::Parse(_) => gettext("This address is not a podcast feed"),
    }
}

pub fn store(error: &StoreError) -> String {
    match error {
        StoreError::Sqlite(_) => gettext("Could not read or save your library"),
        StoreError::TooNew { .. } => gettext("Your library was saved by a newer version of Beacon"),
        StoreError::Io(_) => gettext("Could not open your library folder"),
    }
}

pub fn playback(error: &PlaybackError) -> String {
    match error {
        PlaybackError::Unavailable(_) => gettext("Audio playback is not available"),
        PlaybackError::NoAudio => gettext("This episode has no audio to play"),
        PlaybackError::Stream(_) => gettext("Could not play this episode"),
    }
}

/// Why "title" did not download; `None` when it was cancelled, which needs
/// no message.
pub fn download(error: &DownloadError, title: &str) -> Option<String> {
    let message = match error {
        DownloadError::Cancelled => return None,
        // Translators: {title} is an episode's title.
        DownloadError::Network(_) => gettext("Could not download “{title}”: the connection failed"),
        // Translators: {title} is an episode's title; {code} an HTTP status
        // code, such as 404.
        DownloadError::Status(code) => {
            gettext("Could not download “{title}”: the server answered with error {code}")
                .replace("{code}", &code.to_string())
        }
        // Translators: {title} is an episode's title.
        DownloadError::Io(_) => gettext("Could not save “{title}” on this device"),
        // Translators: {title} is an episode's title.
        DownloadError::NoAudio => gettext("“{title}” has no audio to download"),
        DownloadError::Store(store) => return Some(self::store(store)),
    };
    Some(message.replace("{title}", title))
}
