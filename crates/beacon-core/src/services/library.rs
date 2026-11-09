// SPDX-License-Identifier: GPL-3.0-or-later

//! Subscribing and refreshing: fetch a feed, then store what changed.
//!
//! The store sits behind a mutex that is never held across an `await`, so the
//! network work of several feeds can overlap.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use futures_util::StreamExt;
use url::Url;

use crate::error::{LibraryError, StoreError};
use crate::model::{Chapter, EpisodeId, ShowId};
use crate::services::clock::Clock;
use crate::services::clock::SystemClock;
use crate::services::covers::CoverCache;
use crate::services::feeds::FeedClient;
use crate::services::http_client;
use crate::store::{Store, SyncStats};

/// What a refresh of every show did.
#[derive(Debug, Default)]
pub struct RefreshSummary {
    pub added: usize,
    /// Shows whose feed could not be refreshed, with the reason.
    pub failed: Vec<(ShowId, LibraryError)>,
}

/// Feeds fetched at once by a refresh of everything: enough to overlap the
/// waiting, few enough to be polite to the network and to podcast hosts.
const REFRESH_AT_ONCE: usize = 4;

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

    /// The library in `data_dir` (`beacon.db`), caching covers in
    /// `cache_dir/covers`. Creates the folders if needed.
    pub fn open(data_dir: &Path, cache_dir: &Path) -> Result<Self, StoreError> {
        std::fs::create_dir_all(data_dir)?;
        let store = Store::open(&data_dir.join("beacon.db"))?;
        let http = http_client();
        Ok(Self::new(
            store,
            FeedClient::new(http.clone()),
            CoverCache::new(cache_dir.join("covers"), http),
            Arc::new(SystemClock),
        ))
    }

    pub fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.clock.now()
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

    /// Refresh every show, a few at a time, so a library of many feeds is
    /// not as slow as all of them added up. A feed that fails does not stop
    /// the others.
    pub async fn refresh_all(&self) -> RefreshSummary {
        let shows = match self.lock().shows() {
            Ok(shows) => shows,
            Err(err) => {
                return RefreshSummary {
                    added: 0,
                    failed: vec![(ShowId(0), err.into())],
                };
            }
        };
        let mut results = futures_util::stream::iter(shows)
            .map(|show| async move { (show.id, self.refresh(show.id).await) })
            .buffer_unordered(REFRESH_AT_ONCE);
        let mut summary = RefreshSummary::default();
        while let Some((id, result)) = results.next().await {
            match result {
                Ok(stats) => summary.added += stats.added,
                Err(err) => summary.failed.push((id, err)),
            }
        }
        summary
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

    /// The episode's own cover, downloaded once and then kept with the
    /// shows' covers. `None` when it has no image of its own, or the same
    /// image as its show (GNOME Podcasts' rule), or the image cannot be
    /// fetched: the show's cover stands in.
    /// Delete the downloaded files of `ids` and forget them; a file
    /// already gone is forgotten too. Returns how many were downloaded.
    pub fn delete_downloads(&self, ids: &[EpisodeId]) -> Result<usize, StoreError> {
        let store = self.lock();
        let now = self.clock.now();
        let mut deleted = 0;
        for id in ids {
            let Some(path) = store.episode(*id)?.and_then(|e| e.state.download_path) else {
                continue;
            };
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err.into()),
            }
            store.set_download(*id, None, now)?;
            deleted += 1;
        }
        Ok(deleted)
    }

    pub async fn episode_cover(&self, id: EpisodeId) -> Result<Option<PathBuf>, StoreError> {
        let (episode_image, show_image) = {
            let store = self.lock();
            let Some(episode) = store.episode(id)? else {
                return Ok(None);
            };
            let show_image = store.show(episode.show_id)?.and_then(|s| s.info.image_url);
            (episode.info.image_url, show_image)
        };
        let Some(url) = episode_image.filter(|url| Some(url) != show_image.as_ref()) else {
            return Ok(None);
        };
        Ok(self.covers.fetch(&url).await.ok())
    }

    /// The episode's chapters: those the feed carries, or else those of its
    /// chapters file, fetched the first time and then kept.
    pub async fn chapters(&self, id: EpisodeId) -> Result<Vec<Chapter>, LibraryError> {
        let (stored, url) = {
            let store = self.lock();
            let stored = store.chapters(id)?;
            let url = store.episode(id)?.and_then(|e| e.info.chapters_url);
            (stored, url)
        };
        let Some(url) = url.filter(|_| stored.is_empty()) else {
            return Ok(stored);
        };
        let chapters = self.feeds.fetch_chapters(&url).await?;
        self.lock().replace_chapters(id, &chapters)?;
        Ok(chapters)
    }

    fn lock(&self) -> MutexGuard<'_, Store> {
        // A panic while holding the lock cannot leave SQLite inconsistent:
        // every multi-step write is a transaction.
        self.store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
