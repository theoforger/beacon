// SPDX-License-Identifier: GPL-3.0-or-later

//! Subscribing and refreshing: fetch a feed, then store what changed.
//!
//! The store sits behind a mutex that is never held across an `await`, so the
//! network work of several feeds can overlap.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use futures_util::StreamExt;
use url::Url;

use crate::error::{DownloadError, LibraryError, OpmlError, StoreError};
use crate::model::{Chapter, EpisodeId, ParsedFeed, ShowId};
use crate::services::cleanup::CleanupPolicy;
use crate::services::clock::Clock;
use crate::services::clock::SystemClock;
use crate::services::covers::CoverCache;
use crate::services::downloads::{Downloader, Progress};
use crate::services::feeds::FeedClient;
use crate::services::opml::{self, ImportPlan, OpmlFeed};
use crate::services::{download_client, http_client};
use crate::store::{Store, SyncStats};

/// A feed fetched to be looked at before subscribing. Nothing of it is
/// stored but its cover, cached with the shows' covers so that subscribing
/// finds it there.
#[derive(Debug, Clone)]
pub struct FeedPreview {
    pub url: Url,
    pub feed: ParsedFeed,
    pub cover: Option<PathBuf>,
    /// The show, when the feed is already subscribed to.
    pub subscribed: Option<ShowId>,
}

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

/// Feeds subscribed to at once by an OPML import, as a refresh fetches.
const IMPORT_AT_ONCE: usize = REFRESH_AT_ONCE;

/// Episodes downloaded at once; the others wait their turn, in the order
/// asked, so Download All Episodes does not open a connection per episode.
const DOWNLOADS_AT_ONCE: usize = 3;

pub struct Library {
    store: Mutex<Store>,
    feeds: FeedClient,
    covers: CoverCache,
    downloads: Downloader,
    download_slots: tokio::sync::Semaphore,
    clock: Arc<dyn Clock>,
}

impl Library {
    pub fn new(
        store: Store,
        feeds: FeedClient,
        covers: CoverCache,
        downloads: Downloader,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            store: Mutex::new(store),
            feeds,
            covers,
            downloads,
            download_slots: tokio::sync::Semaphore::new(DOWNLOADS_AT_ONCE),
            clock,
        }
    }

    /// The library in `data_dir` (`beacon.db`, and downloaded episodes in
    /// `downloads`), caching covers in `cache_dir/covers`. Creates the
    /// folders if needed.
    pub fn open(data_dir: &Path, cache_dir: &Path) -> Result<Self, StoreError> {
        std::fs::create_dir_all(data_dir)?;
        let store = Store::open(&data_dir.join("beacon.db"))?;
        let http = http_client();
        Ok(Self::new(
            store,
            FeedClient::new(http.clone()),
            CoverCache::new(cache_dir.join("covers"), http),
            Downloader::new(data_dir.join("downloads"), download_client()),
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
        self.subscribe_with(url, false).await
    }

    /// Subscribe to a feed the sync server lists, without recording it to
    /// be sent back.
    pub async fn subscribe_from_server(&self, url: &Url) -> Result<ShowId, LibraryError> {
        self.subscribe_with(url, true).await
    }

    async fn subscribe_with(&self, url: &Url, from_server: bool) -> Result<ShowId, LibraryError> {
        if let Some(id) = self.lock().show_id_for_feed(url)? {
            return Err(LibraryError::AlreadySubscribed(id));
        }
        let feed = self.feeds.fetch(url).await?;
        self.store_feed(url, &feed, from_server).await
    }

    /// Fetch the feed at `url` to show it before subscribing.
    pub async fn preview(&self, url: &Url) -> Result<FeedPreview, LibraryError> {
        let feed = self.feeds.fetch(url).await?;
        let cover = match &feed.show.image_url {
            Some(image) => self.covers.fetch(image).await.ok(),
            None => None,
        };
        Ok(FeedPreview {
            url: url.clone(),
            subscribed: self.lock().show_id_for_feed(url)?,
            feed,
            cover,
        })
    }

    /// Subscribe to a feed already fetched for its preview, without
    /// fetching it again.
    pub async fn subscribe_preview(&self, preview: &FeedPreview) -> Result<ShowId, LibraryError> {
        if let Some(id) = self.lock().show_id_for_feed(&preview.url)? {
            return Err(LibraryError::AlreadySubscribed(id));
        }
        self.store_feed(&preview.url, &preview.feed, false).await
    }

    async fn store_feed(
        &self,
        url: &Url,
        feed: &ParsedFeed,
        from_server: bool,
    ) -> Result<ShowId, LibraryError> {
        let now = self.clock.now();
        let id = {
            let insert = |store: &mut Store| {
                let id = store.insert_show(url, &feed.show, now)?;
                store.sync_episodes(id, &feed.episodes, now)?;
                store.mark_seen_except_newest(id)?;
                Ok(id)
            };
            let mut store = self.lock();
            if from_server {
                store.quietly(insert)?
            } else {
                insert(&mut store)?
            }
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

    /// The OPML file at `path`: its feeds, less those already subscribed
    /// to and those it repeats.
    pub fn read_opml(&self, path: &Path) -> Result<ImportPlan, OpmlError> {
        let bytes = std::fs::read(path)?;
        let feeds = opml::parse(&String::from_utf8_lossy(&bytes))?;
        let subscribed: Vec<Url> = self
            .lock()
            .shows()?
            .into_iter()
            .map(|s| s.feed_url)
            .collect();
        Ok(opml::plan(feeds, &subscribed))
    }

    /// Subscribe to `feeds`, a few at a time, telling `done` about each as
    /// it finishes, in the order they finish. One that fails does not stop
    /// the others.
    pub async fn import(
        &self,
        feeds: Vec<OpmlFeed>,
        mut done: impl FnMut(OpmlFeed, Result<ShowId, LibraryError>),
    ) {
        let mut results = futures_util::stream::iter(feeds)
            .map(|feed| async move {
                let result = self.subscribe(&feed.url).await;
                (feed, result)
            })
            .buffer_unordered(IMPORT_AT_ONCE);
        while let Some((feed, result)) = results.next().await {
            done(feed, result);
        }
    }

    /// Write every subscription to `path` as OPML. Returns how many.
    pub fn export_opml(&self, path: &Path) -> Result<usize, OpmlError> {
        let shows = self.lock().shows()?;
        std::fs::write(path, opml::write(&shows, self.clock.now()))?;
        Ok(shows.len())
    }

    /// Whether any subscription's address may hold a secret, which an
    /// exported file would give away (`opml::may_hold_secret`).
    pub fn has_secret_feeds(&self) -> Result<bool, StoreError> {
        Ok(self
            .lock()
            .shows()?
            .iter()
            .any(|show| opml::may_hold_secret(&show.feed_url)))
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

    /// Download episode `id`, reporting to `progress`, once one of the
    /// download slots is free. Resumes a download that stopped part way;
    /// an episode already downloaded is left as it is. Returns its file.
    pub async fn download(
        &self,
        id: EpisodeId,
        progress: &Progress,
    ) -> Result<PathBuf, DownloadError> {
        let url = {
            let store = self.lock();
            let episode = store.episode(id)?.ok_or(DownloadError::NoAudio)?;
            if let Some(path) = episode.state.download_path.filter(|p| p.exists()) {
                return Ok(path);
            }
            episode
                .info
                .enclosure
                .map(|e| e.url)
                .ok_or(DownloadError::NoAudio)?
        };
        let _slot = self
            .download_slots
            .acquire()
            .await
            .map_err(|_| DownloadError::Cancelled)?;
        if progress.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }
        let path = self.downloads.fetch(id, &url, progress).await?;
        self.lock()
            .set_download(id, Some(&path), self.clock.now())?;
        Ok(path)
    }

    /// Delete the downloads `policy` finds old, leaving queued episodes
    /// and those in `keep` (the one playing). Returns how many it deleted.
    pub fn clean_downloads(
        &self,
        policy: CleanupPolicy,
        keep: &[EpisodeId],
    ) -> Result<usize, StoreError> {
        if !policy.enabled {
            return Ok(0);
        }
        let now = self.clock.now();
        let old: Vec<EpisodeId> = {
            let store = self.lock();
            let queue = store.queue()?;
            store
                .downloaded_episodes()?
                .into_iter()
                .filter(|e| policy.expired(e, now))
                .map(|e| e.id)
                .filter(|id| !queue.contains(id) && !keep.contains(id))
                .collect()
        };
        self.delete_downloads(&old)
    }

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

    /// The episode's own cover, downloaded once and then kept with the
    /// shows' covers. `None` when it has no image of its own, or the same
    /// image as its show (GNOME Podcasts' rule), or the image cannot be
    /// fetched: the show's cover stands in.
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
