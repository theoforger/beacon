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
