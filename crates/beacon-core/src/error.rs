// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed errors. The UI turns these into toasts or status pages.

use crate::model::ShowId;

#[derive(Debug, thiserror::Error)]
pub enum FeedError {
    #[error("could not reach the feed: {0}")]
    Network(#[from] reqwest::Error),
    #[error("the server answered with HTTP status {0}")]
    Status(u16),
    #[error("not a podcast feed: {0}")]
    Parse(String),
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("the library has {applied} schema changes; this version knows {known}")]
    TooNew { applied: u32, known: u32 },
    #[error("could not create the library folder: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum CoverError {
    #[error("could not download the cover: {0}")]
    Network(#[from] reqwest::Error),
    #[error("the server answered with HTTP status {0}")]
    Status(u16),
    #[error("could not save the cover: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error(transparent)]
    Feed(#[from] FeedError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("already subscribed to this feed")]
    AlreadySubscribed(ShowId),
    #[error("no such show")]
    ShowNotFound(ShowId),
}

/// Why an episode could not be downloaded.
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("could not download the episode: {0}")]
    Network(#[from] reqwest::Error),
    #[error("the server answered with HTTP status {0}")]
    Status(u16),
    #[error("could not save the episode: {0}")]
    Io(#[from] std::io::Error),
    #[error("the episode has no audio file")]
    NoAudio,
    #[error("the download was cancelled")]
    Cancelled,
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Why an episode cannot be played. The player reports these; the details
/// are for logs.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PlaybackError {
    #[error("the audio system could not start: {0}")]
    Unavailable(String),
    #[error("the episode has no audio file")]
    NoAudio,
    #[error("could not play the episode: {0}")]
    Stream(String),
}

/// Why a sync, a login or a logout did not go through
/// (`docs/specs/sync.md`, Errors). The UI has a message for each.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("no sync account, or sync is off")]
    NotConfigured,
    #[error("not a server address")]
    InvalidServer,
    #[error("could not reach the sync server: {0}")]
    Network(#[from] reqwest::Error),
    #[error("the server did not accept the login")]
    Unauthorized,
    #[error("the server has no nextcloud-gpodder app")]
    AppMissing,
    #[error("the sync server answered with HTTP status {0}")]
    Status(u16),
    #[error("could not read the sync server's answer: {0}")]
    Parse(String),
    #[error("the browser login was not approved within 20 minutes")]
    LoginExpired,
    #[error("the browser login was cancelled")]
    Cancelled,
    #[error("the keyring is unavailable: {0}")]
    Keyring(String),
    #[error(transparent)]
    Library(#[from] LibraryError),
}

impl From<StoreError> for SyncError {
    fn from(err: StoreError) -> Self {
        Self::Library(LibraryError::Store(err))
    }
}

/// Why a search of Apple's podcast directory, or a lookup, did not go
/// through (`docs/specs/add-and-itunes.md`). The UI has a message for each.
#[derive(Debug, thiserror::Error)]
pub enum ItunesError {
    #[error("could not reach the iTunes Search API: {0}")]
    Network(#[from] reqwest::Error),
    #[error("too many searches; the next is allowed in {0:?}")]
    RateLimited(std::time::Duration),
    #[error("Apple has no podcast directory for the country {0}")]
    NoStore(String),
    #[error("the iTunes Search API answered with HTTP status {0}")]
    Status(u16),
    #[error("could not read the iTunes Search API's answer: {0}")]
    Parse(String),
    #[error("Apple's directory has no feed for this show")]
    NoFeed,
}

/// Why an OPML file could not be imported or exported
/// (`docs/specs/opml.md`). The UI has a message for each.
#[derive(Debug, thiserror::Error)]
pub enum OpmlError {
    #[error("could not read or write the file: {0}")]
    Io(#[from] std::io::Error),
    #[error("the file is not well-formed XML: {0}")]
    Xml(String),
    #[error("the file is XML, but not OPML")]
    NotOpml,
    #[error(transparent)]
    Store(#[from] StoreError),
}
