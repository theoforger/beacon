// SPDX-License-Identifier: GPL-3.0-or-later

//! Subscribing and refreshing: fetch a feed, then store what changed.
//!
//! The store sits behind a mutex that is never held across an `await`, so the
//! network work of several feeds can overlap.

use std::sync::{Arc, Mutex, MutexGuard};

use url::Url;

use crate::error::LibraryError;
use crate::model::ShowId;
use crate::services::clock::Clock;
use crate::services::covers::CoverCache;
use crate::services::feeds::FeedClient;
use crate::store::{Store, SyncStats};

pub struct Library {
    store: Mutex<Store>,
    feeds: FeedClient,
    covers: CoverCache,
    clock: Arc<dyn Clock>,
}

impl Library {
    pub fn new(store: Store, feeds: FeedClient, covers: CoverCache, clock: Arc<dyn Clock>) -> Self {
        Self {
            store: Mutex::new(store),
            feeds,
            covers,
            clock,
        }
    }

    /// Run `f` with the store. Keep the closure short and synchronous.
    pub fn with_store<T>(&self, f: impl FnOnce(&mut Store) -> T) -> T {
        f(&mut self.lock())
    }

    /// Subscribe to the feed at `url`. Only the newest episode starts unseen.
    pub async fn subscribe(&self, url: &Url) -> Result<ShowId, LibraryError> {
        if let Some(id) = self.lock().show_id_for_feed(url)? {
            return Err(LibraryError::AlreadySubscribed(id));
        }
        let feed = self.feeds.fetch(url).await?;
        let now = self.clock.now();
        let id = {
            let mut store = self.lock();
            let id = store.insert_show(url, &feed.show, now)?;
            store.sync_episodes(id, &feed.episodes, now)?;
            store.mark_seen_except_newest(id)?;
            id
        };
        self.cache_cover(id).await;
        Ok(id)
    }

    /// Fetch the show's feed again and store new episodes. Episodes already
    /// stored keep their state; only their metadata is updated.
    pub async fn refresh(&self, id: ShowId) -> Result<SyncStats, LibraryError> {
        let show = self
            .lock()
            .show(id)?
            .ok_or(LibraryError::ShowNotFound(id))?;
        let feed = self.feeds.fetch(&show.feed_url).await?;
        let now = self.clock.now();
        let stats = {
            let mut store = self.lock();
            store.update_show(id, &feed.show, now)?;
            store.sync_episodes(id, &feed.episodes, now)?
        };
        if feed.show.image_url != show.info.image_url || show.cover_path.is_none() {
            self.cache_cover(id).await;
        }
        Ok(stats)
    }

    /// Download the show's cover. A missing cover is not an error worth
    /// reporting: the UI shows a placeholder and the next refresh tries again.
    async fn cache_cover(&self, id: ShowId) {
        let Ok(Some(show)) = self.lock().show(id) else {
            return;
        };
        let Some(image_url) = show.info.image_url else {
            return;
        };
        if let Ok(path) = self.covers.fetch(&image_url).await {
            let _ = self.lock().set_cover_path(id, &path);
        }
    }

    fn lock(&self) -> MutexGuard<'_, Store> {
        // A panic while holding the lock cannot leave SQLite inconsistent:
        // every multi-step write is a transaction.
        self.store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
