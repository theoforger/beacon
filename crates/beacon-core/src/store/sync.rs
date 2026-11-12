// SPDX-License-Identifier: GPL-3.0-or-later

//! The sync account and the changes waiting to be sent
//! (`docs/specs/sync.md`). Triggers (migration 004) record local changes;
//! changes from the server are written through [`Store::quietly`], so they
//! are not recorded and sent back.

use std::time::Duration;

use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};
use url::Url;

use super::Store;
use crate::error::StoreError;
use crate::model::{EpisodeId, ShowId};
use crate::sync::{EpisodeAction, SubscriptionChanges, finished};

/// The account Beacon syncs with. Its app password is in the keyring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncAccount {
    pub server: Url,
    pub user: String,
    /// The Sync switch: off records nothing and never syncs.
    pub active: bool,
    /// The server's timestamps of the last changes fetched, for `since`.
    pub subscriptions_since: Option<i64>,
    pub actions_since: Option<i64>,
    pub last_synced: Option<DateTime<Utc>>,
}

impl SyncAccount {
    /// No sync has finished yet: the next one merges both sides.
    pub fn is_first_sync(&self) -> bool {
        self.subscriptions_since.is_none() || self.actions_since.is_none()
    }
}

/// How far the pending changes went when a sync started, so it clears only
/// those it sent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PendingMark {
    shows: i64,
    episodes: i64,
}

/// A local episode change waiting to be sent, as a PLAY.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingPlay {
    pub feed_url: String,
    pub guid: String,
    pub episode_url: String,
    pub started: i64,
    pub position: i64,
    pub total: i64,
    /// Unix seconds.
    pub changed_at: i64,
}

impl PendingPlay {
    /// The action to send. The GUID goes along only when the feed gave one:
    /// Beacon's own fallback is the enclosure URL, which `episode` carries.
    ///
    /// Servers hand out actions newer than a device's `since`, judged by
    /// the action's own time, so an action dated before another device last
    /// fetched would never reach it. The time is therefore never earlier
    /// than `not_before`, the server's time when this sync fetched.
    pub fn action(&self, not_before: i64) -> EpisodeAction {
        EpisodeAction {
            podcast: self.feed_url.clone(),
            episode: self.episode_url.clone(),
            guid: (self.guid != self.episode_url).then(|| self.guid.clone()),
            action: crate::sync::ActionKind::Play,
            timestamp: DateTime::from_timestamp(self.changed_at.max(not_before), 0)
                .unwrap_or_default(),
            started: u64::try_from(self.started).ok(),
            position: u64::try_from(self.position).ok(),
            total: u64::try_from(self.total).ok().filter(|t| *t > 0),
        }
    }
}

/// What applying one action from the server did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// The episode changed.
    Changed(EpisodeId),
    /// The episode was already as the server says (an echo, say).
    Same(EpisodeId),
    /// A newer local change wins; it is sent instead.
    LocalNewer,
    /// No stored episode matches; `Some` when its show is known.
    Unmatched(Option<ShowId>),
}

impl Store {
    /// Run `f` with the sync triggers quiet: what it writes came from the
    /// server and is not sent back. The connection is the store's only
    /// one, so nothing else writes meanwhile.
    pub fn quietly<T>(
        &mut self,
        f: impl FnOnce(&mut Store) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        self.set_guard(true)?;
        let result = f(self);
        self.set_guard(false)?;
        result
    }

    pub(super) fn set_guard(&self, applying: bool) -> Result<(), StoreError> {
        self.conn
            .execute("UPDATE sync_guard SET applying = ?1", [applying])?;
        Ok(())
    }

    pub fn sync_account(&self) -> Result<Option<SyncAccount>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT server, user, active, subscriptions_since, actions_since, last_synced_at
                 FROM sync_account",
                [],
                |row| {
                    Ok(SyncAccount {
                        server: row.get(0)?,
                        user: row.get(1)?,
                        active: row.get(2)?,
                        subscriptions_since: row.get(3)?,
                        actions_since: row.get(4)?,
                        last_synced: row.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    /// Sign in: the account replaces any other, starts active and has
    /// never synced. Pending changes from before are dropped; the first
    /// sync sends the whole library.
    pub fn set_sync_account(&self, server: &Url, user: &str) -> Result<(), StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM sync_account", [])?;
        tx.execute("DELETE FROM sync_shows", [])?;
        tx.execute("DELETE FROM sync_episodes", [])?;
        tx.execute(
            "INSERT INTO sync_account (id, server, user) VALUES (1, ?1, ?2)",
            params![server, user],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Log Out: the account and every pending change go.
    pub fn remove_sync_account(&self) -> Result<(), StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM sync_account", [])?;
        tx.execute("DELETE FROM sync_shows", [])?;
        tx.execute("DELETE FROM sync_episodes", [])?;
        tx.commit()?;
        Ok(())
    }

    /// The Sync switch.
    pub fn set_sync_active(&self, active: bool) -> Result<(), StoreError> {
        self.conn
            .execute("UPDATE sync_account SET active = ?1", [active])?;
        Ok(())
    }

    /// Record the whole library as pending, for the first sync: every show
    /// and every episode with a position or marked played. Changes already
    /// pending stay as they are. A played episode's change dates from when
    /// it was played; a position has no date, so the server's wins.
    pub fn record_library(&self) -> Result<(), StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO sync_shows (feed_url, action, seq)
             SELECT feed_url, 'add',
                 (SELECT coalesce(max(seq), 0) FROM sync_shows) + row_number() OVER ()
             FROM shows",
            [],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO sync_episodes
                 (feed_url, guid, episode_url, started, position, total, changed_at, seq)
             SELECT shows.feed_url, e.guid, e.enclosure_url, -1,
                 CASE WHEN e.played THEN coalesce(e.duration_secs, e.position_secs)
                      ELSE e.position_secs END,
                 coalesce(e.duration_secs, e.position_secs),
                 CASE WHEN e.played
                      THEN coalesce(CAST(strftime('%s', e.played_at) AS INTEGER), 0)
                      ELSE 0 END,
                 (SELECT coalesce(max(seq), 0) FROM sync_episodes) + row_number() OVER ()
             FROM episodes e JOIN shows ON shows.id = e.show_id
             WHERE e.enclosure_url IS NOT NULL AND (e.played OR e.position_secs > 0)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Whether any change waits to be sent: what Log Out would drop.
    pub fn has_pending_changes(&self) -> Result<bool, StoreError> {
        Ok(self.pending_mark()? != PendingMark::default())
    }

    /// How far the pending changes go now.
    pub fn pending_mark(&self) -> Result<PendingMark, StoreError> {
        Ok(self.conn.query_row(
            "SELECT (SELECT coalesce(max(seq), 0) FROM sync_shows),
                    (SELECT coalesce(max(seq), 0) FROM sync_episodes)",
            [],
            |row| {
                Ok(PendingMark {
                    shows: row.get(0)?,
                    episodes: row.get(1)?,
                })
            },
        )?)
    }

    /// The subscription changes waiting to be sent, up to `mark`.
    pub fn pending_subscriptions(
        &self,
        mark: PendingMark,
    ) -> Result<SubscriptionChanges, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT feed_url, action FROM sync_shows WHERE seq <= ?1 ORDER BY seq")?;
        let mut changes = SubscriptionChanges::default();
        for row in stmt.query_map([mark.shows], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })? {
            let (url, action) = row?;
            if action == "add" {
                changes.add.push(url);
            } else {
                changes.remove.push(url);
            }
        }
        Ok(changes)
    }

    /// The pending subscription change to `feed_url`: `Some(true)` for an
    /// add, `Some(false)` for a remove.
    pub fn pending_subscription(&self, feed_url: &str) -> Result<Option<bool>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT action = 'add' FROM sync_shows WHERE feed_url = ?1",
                [feed_url],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// The episode changes waiting to be sent, up to `mark`, oldest first.
    pub fn pending_plays(&self, mark: PendingMark) -> Result<Vec<PendingPlay>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT feed_url, guid, episode_url, started, position, total, changed_at
             FROM sync_episodes WHERE seq <= ?1 ORDER BY seq",
        )?;
        let plays = stmt
            .query_map([mark.episodes], |row| {
                Ok(PendingPlay {
                    feed_url: row.get(0)?,
                    guid: row.get(1)?,
                    episode_url: row.get(2)?,
                    started: row.get(3)?,
                    position: row.get(4)?,
                    total: row.get(5)?,
                    changed_at: row.get(6)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(plays)
    }

    /// A sync went through: clear the changes it sent (those up to `mark`)
    /// and keep the server's timestamps for the next one.
    pub fn finish_sync(
        &self,
        mark: PendingMark,
        subscriptions_since: i64,
        actions_since: i64,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM sync_shows WHERE seq <= ?1", [mark.shows])?;
        tx.execute("DELETE FROM sync_episodes WHERE seq <= ?1", [mark.episodes])?;
        tx.execute(
            "UPDATE sync_account SET subscriptions_since = ?1, actions_since = ?2,
                 last_synced_at = ?3",
            params![subscriptions_since, actions_since, now],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The stored episode an action is about: by feed and GUID, then by
    /// enclosure URL within the feed.
    /// `Err` holds the show when it is known.
    fn match_action(
        &self,
        action: &EpisodeAction,
    ) -> Result<Result<EpisodeId, Option<ShowId>>, StoreError> {
        let Some(show) = self
            .conn
            .query_row(
                "SELECT id FROM shows WHERE feed_url = ?1",
                [&action.podcast],
                |row| row.get(0).map(ShowId),
            )
            .optional()?
        else {
            return Ok(Err(None));
        };
        let by_guid = match &action.guid {
            Some(guid) => self
                .conn
                .query_row(
                    "SELECT id FROM episodes WHERE show_id = ?1 AND guid = ?2",
                    params![show.0, guid],
                    |row| row.get(0).map(EpisodeId),
                )
                .optional()?,
            None => None,
        };
        let found = match by_guid {
            Some(id) => Some(id),
            None => self
                .conn
                .query_row(
                    "SELECT id FROM episodes WHERE show_id = ?1
                         AND (enclosure_url = ?2 OR guid = ?2)
                     ORDER BY id LIMIT 1",
                    params![show.0, action.episode],
                    |row| row.get(0).map(EpisodeId),
                )
                .optional()?,
        };
        Ok(found.ok_or(Some(show)))
    }

    /// Apply a PLAY from the server (`docs/specs/sync.md`, A sync): a
    /// newer local change wins; a play that reached the end marks the
    /// episode played and starts it over; a partial play moves the
    /// position and never un-plays. The server's change replaces the
    /// pending local one.
    pub fn apply_play(&mut self, action: &EpisodeAction) -> Result<Applied, StoreError> {
        let id = match self.match_action(action)? {
            Ok(id) => id,
            Err(show) => return Ok(Applied::Unmatched(show)),
        };
        let (feed_url, guid): (String, String) = self.conn.query_row(
            "SELECT shows.feed_url, e.guid FROM episodes e JOIN shows ON shows.id = e.show_id
             WHERE e.id = ?1",
            [id.0],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let local: Option<i64> = self
            .conn
            .query_row(
                "SELECT changed_at FROM sync_episodes WHERE feed_url = ?1 AND guid = ?2",
                params![feed_url, guid],
                |row| row.get(0),
            )
            .optional()?;
        if local.is_some_and(|at| at > action.timestamp.timestamp()) {
            return Ok(Applied::LocalNewer);
        }
        let state = |store: &Store| -> Result<(bool, i64), StoreError> {
            Ok(store.conn.query_row(
                "SELECT played, position_secs FROM episodes WHERE id = ?1",
                [id.0],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?)
        };
        let before = state(self)?;
        self.quietly(|store| {
            let tx = store.conn.unchecked_transaction()?;
            if finished(action) {
                store.set_played(id, true, action.timestamp)?;
                store.set_position(id, Duration::ZERO)?;
            } else if let Some(position) = action.position {
                store.set_position(id, Duration::from_secs(position))?;
            }
            tx.execute(
                "DELETE FROM sync_episodes WHERE feed_url = ?1 AND guid = ?2",
                params![feed_url, guid],
            )?;
            tx.commit()?;
            Ok(if state(store)? == before {
                Applied::Same(id)
            } else {
                Applied::Changed(id)
            })
        })
    }
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
