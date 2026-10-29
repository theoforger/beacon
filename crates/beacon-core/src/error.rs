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
    #[error("database migration failed: {0}")]
    Migration(#[from] rusqlite_migration::Error),
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
