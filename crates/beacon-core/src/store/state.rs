// SPDX-License-Identifier: GPL-3.0-or-later

//! Changes the listener makes: played, seen, favourites, the queue, page
//! preferences and unsubscribing. Marking played or unplayed never touches
//! the queue.

use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};

use super::Store;
use super::rows::{EPISODE_COLUMNS, episode_from_row};
use crate::error::StoreError;
use crate::model::{Episode, EpisodeFilter, EpisodeId, ShowId, ShowPrefs, SortOrder};

impl Store {
    pub fn episode(&self, id: EpisodeId) -> Result<Option<Episode>, StoreError> {
        Ok(self
            .conn
            .query_row(
                &format!("{EPISODE_COLUMNS} WHERE id = ?1"),
                [id.0],
                episode_from_row,
            )
            .optional()?)
    }

    /// Mark one episode played or unplayed. Unplayed starts it from the
    /// beginning again. The seen flag stays as it is, so the blue dot does
    /// not come back.
    pub fn set_played(
        &self,
        id: EpisodeId,
        played: bool,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        if played {
            self.conn.execute(
                "UPDATE episodes SET played = 1, played_at = ?2 WHERE id = ?1 AND played = 0",
                params![id.0, now],
            )?;
        } else {
            self.conn.execute(
                "UPDATE episodes SET played = 0, played_at = NULL, position_secs = 0 WHERE id = ?1",
                [id.0],
            )?;
        }
        Ok(())
    }

    /// Mark every episode of a show played. Returns how many changed.
    pub fn mark_all_played(&self, show: ShowId, now: DateTime<Utc>) -> Result<usize, StoreError> {
        Ok(self.conn.execute(
            "UPDATE episodes SET played = 1, played_at = ?2 WHERE show_id = ?1 AND played = 0",
            params![show.0, now],
        )?)
    }

    /// Viewing an episode's details clears its blue dot for good.
    pub fn mark_seen(&self, id: EpisodeId) -> Result<(), StoreError> {
        self.conn
            .execute("UPDATE episodes SET seen = 1 WHERE id = ?1", [id.0])?;
        Ok(())
    }

    pub fn set_favorite(&self, id: EpisodeId, favorite: bool) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE episodes SET favorite = ?2 WHERE id = ?1",
            params![id.0, favorite],
        )?;
        Ok(())
    }

    /// The manual queue, in play order.
    pub fn queue(&self) -> Result<Vec<EpisodeId>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT episode_id FROM queue ORDER BY position")?;
        let ids = stmt
            .query_map([], |row| row.get(0).map(EpisodeId))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    /// Add an episode to the end of the queue. Already queued: no change.
    pub fn enqueue(&self, id: EpisodeId) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO queue (episode_id, position)
             VALUES (?1, (SELECT coalesce(max(position), 0) + 1 FROM queue))",
            [id.0],
        )?;
        Ok(())
    }

    pub fn dequeue(&self, id: EpisodeId) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM queue WHERE episode_id = ?1", [id.0])?;
        Ok(())
    }

    pub fn show_prefs(&self, show: ShowId) -> Result<ShowPrefs, StoreError> {
        let (filter, order): (String, String) = self.conn.query_row(
            "SELECT episode_filter, sort_order FROM shows WHERE id = ?1",
            [show.0],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(ShowPrefs {
            filter: match filter.as_str() {
                "unplayed" => EpisodeFilter::Unplayed,
                _ => EpisodeFilter::All,
            },
            order: match order.as_str() {
                "oldest" => SortOrder::OldestFirst,
                _ => SortOrder::NewestFirst,
            },
        })
    }

    pub fn set_show_prefs(&self, show: ShowId, prefs: ShowPrefs) -> Result<(), StoreError> {
        let filter = match prefs.filter {
            EpisodeFilter::All => "all",
            EpisodeFilter::Unplayed => "unplayed",
        };
        let order = match prefs.order {
            SortOrder::NewestFirst => "newest",
            SortOrder::OldestFirst => "oldest",
        };
        self.conn.execute(
            "UPDATE shows SET episode_filter = ?2, sort_order = ?3 WHERE id = ?1",
            params![show.0, filter, order],
        )?;
        Ok(())
    }

    /// Unsubscribe: the show, its episodes, chapters and queue entries go.
    pub fn delete_show(&self, show: ShowId) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM shows WHERE id = ?1", [show.0])?;
        Ok(())
    }
}
