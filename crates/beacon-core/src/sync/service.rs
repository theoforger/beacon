// SPDX-License-Identifier: GPL-3.0-or-later

//! One sync (`docs/specs/sync.md`, A sync): pull subscriptions, then
//! episode actions, then push what changed here, then clear what was sent.
//! The server's timestamps (less a second of overlap) become the next
//! `since`, never the local clock.

use std::collections::HashSet;
use std::sync::Arc;

use url::Url;

use super::{ActionKind, EpisodeAction, SyncBackend};
use crate::error::{LibraryError, SyncError};
use crate::model::ShowId;
use crate::services::library::Library;
use crate::store::Applied;

/// Actions sent in one request, so a first sync of a large library does
/// not make one huge body.
const ACTIONS_PER_REQUEST: usize = 500;

/// How far each `since` reaches back before the server's time.
const SINCE_OVERLAP_SECS: i64 = 1;

/// What a sync did.
#[derive(Debug, Default)]
pub struct SyncSummary {
    /// Shows subscribed to and unsubscribed from, as the server asked.
    pub subscribed: usize,
    pub unsubscribed: usize,
    /// Episodes whose position or played state the server changed.
    pub episodes: usize,
    /// Changes sent to the server.
    pub sent_subscriptions: usize,
    pub sent_actions: usize,
    /// Actions for episodes not in the library, even after a refresh.
    pub dropped: usize,
    /// Feeds the server lists that could not be subscribed to.
    pub failed: Vec<(String, LibraryError)>,
}

impl SyncSummary {
    /// Whether the library changed, so the views need reloading.
    pub fn changed_library(&self) -> bool {
        self.subscribed + self.unsubscribed + self.episodes > 0
    }
}

/// Syncs the library with a server, one sync at a time: one asked for
/// while another runs waits, then sends what changed meanwhile.
pub struct SyncService {
    library: Arc<Library>,
    running: tokio::sync::Mutex<()>,
}

impl SyncService {
    pub fn new(library: Arc<Library>) -> Self {
        Self {
            library,
            running: tokio::sync::Mutex::new(()),
        }
    }

    pub fn library(&self) -> &Arc<Library> {
        &self.library
    }

    /// Sync with `backend`. Fails with `NotConfigured` while signed out or
    /// with the Sync switch off.
    pub async fn sync(&self, backend: &impl SyncBackend) -> Result<SyncSummary, SyncError> {
        let _one_at_a_time = self.running.lock().await;
        let library = &self.library;
        let account = library
            .with_store(|store| store.sync_account())?
            .filter(|account| account.active)
            .ok_or(SyncError::NotConfigured)?;
        let first = account.is_first_sync();
        if first {
            library.with_store(|store| store.record_library())?;
        }
        let mark = library.with_store(|store| store.pending_mark())?;
        let mut summary = SyncSummary::default();

        // 1. Subscriptions.
        let since = account.subscriptions_since.filter(|_| !first);
        let subscriptions = backend.subscriptions(since).await?;
        for feed in &subscriptions.changes.add {
            self.subscribe(feed, &mut summary).await?;
        }
        // The first sync is a union: nothing local is removed.
        if !first {
            for feed in &subscriptions.changes.remove {
                self.unsubscribe(feed, &mut summary)?;
            }
        }

        // 2. Episode actions, oldest first, so the latest wins.
        let since = account.actions_since.filter(|_| !first);
        let update = backend.episode_actions(since).await?;
        let mut plays: Vec<&EpisodeAction> = update
            .actions
            .iter()
            .filter(|a| a.action == ActionKind::Play)
            .collect();
        plays.sort_by_key(|a| a.timestamp);
        let mut unmatched = Vec::new();
        for action in plays {
            match library.with_store(|store| store.apply_play(action))? {
                Applied::Changed(_) => summary.episodes += 1,
                Applied::Same(_) | Applied::LocalNewer => {}
                Applied::Unmatched(Some(show)) => unmatched.push((show, action)),
                Applied::Unmatched(None) => summary.dropped += 1,
            }
        }
        // An episode the feed has gained since the last refresh: refresh
        // its show once, then try again.
        let stale: HashSet<ShowId> = unmatched.iter().map(|(show, _)| *show).collect();
        for show in stale {
            // A feed that cannot be fetched leaves its actions unmatched.
            let _ = library.refresh(show).await;
        }
        for (_, action) in unmatched {
            match library.with_store(|store| store.apply_play(action))? {
                Applied::Changed(_) => summary.episodes += 1,
                Applied::Same(_) | Applied::LocalNewer => {}
                Applied::Unmatched(_) => summary.dropped += 1,
            }
        }

        // 3. Push, leaving out what the server already has.
        let mut changes = library.with_store(|store| store.pending_subscriptions(mark))?;
        let server = &subscriptions.changes;
        changes.add.retain(|feed| !server.add.contains(feed));
        changes.remove.retain(|feed| !server.remove.contains(feed));
        if !changes.is_empty() {
            backend.push_subscriptions(&changes).await?;
            summary.sent_subscriptions = changes.add.len() + changes.remove.len();
        }
        let actions: Vec<EpisodeAction> = library
            .with_store(|store| store.pending_plays(mark))?
            .iter()
            .map(|play| play.action(update.timestamp))
            .collect();
        for chunk in actions.chunks(ACTIONS_PER_REQUEST) {
            backend.push_episode_actions(chunk).await?;
            summary.sent_actions += chunk.len();
        }

        // 4. Clear what was sent; the server's times, less a second, are
        // the next `since`: a change another device sent in the same second
        // as this fetch is not missed, and one seen twice changes nothing.
        let now = library.now();
        library.with_store(|store| {
            store.finish_sync(
                mark,
                subscriptions.timestamp - SINCE_OVERLAP_SECS,
                update.timestamp - SINCE_OVERLAP_SECS,
                now,
            )
        })?;
        Ok(summary)
    }

    /// A feed the server added. A local remove still pending wins.
    async fn subscribe(&self, feed: &str, summary: &mut SyncSummary) -> Result<(), SyncError> {
        if self
            .library
            .with_store(|store| store.pending_subscription(feed))?
            == Some(false)
        {
            return Ok(());
        }
        let Ok(url) = Url::parse(feed) else {
            summary
                .failed
                .push((feed.to_owned(), LibraryError::Feed(bad_url(feed))));
            return Ok(());
        };
        match self.library.subscribe_from_server(&url).await {
            Ok(_) => summary.subscribed += 1,
            Err(LibraryError::AlreadySubscribed(_)) => {}
            Err(LibraryError::Store(err)) => return Err(err.into()),
            Err(err) => summary.failed.push((feed.to_owned(), err)),
        }
        Ok(())
    }

    /// A feed the server removed. A local add still pending wins.
    fn unsubscribe(&self, feed: &str, summary: &mut SyncSummary) -> Result<(), SyncError> {
        let library = &self.library;
        if library.with_store(|store| store.pending_subscription(feed))? == Some(true) {
            return Ok(());
        }
        let Ok(url) = Url::parse(feed) else {
            return Ok(());
        };
        if let Some(show) = library.with_store(|store| store.show_id_for_feed(&url))? {
            library.with_store(|store| store.quietly(|store| store.delete_show(show)))?;
            summary.unsubscribed += 1;
        }
        Ok(())
    }
}

fn bad_url(feed: &str) -> crate::error::FeedError {
    crate::error::FeedError::Parse(format!("not a feed address: {feed}"))
}
