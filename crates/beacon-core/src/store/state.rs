// SPDX-License-Identifier: GPL-3.0-or-later

//! Changes the listener makes: played, seen, favourites, the queue, page
//! preferences and unsubscribing. Marking played or unplayed never touches
//! the queue.

use std::time::Duration;

use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};

use super::Store;
use super::rows::{EPISODE_COLUMNS, episode_from_row};
use crate::error::StoreError;
use crate::model::{
    DownloadFilter, Episode, EpisodeChange, EpisodeFilter, EpisodeId, ShowId, ShowPrefs, SortOrder,
};

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

    /// Mark every episode of a show unplayed, each starting over as one
    /// marked unplayed does. Seen flags stay, so no blue dots come back.
    /// Returns how many changed.
    pub fn mark_all_unplayed(&self, show: ShowId) -> Result<usize, StoreError> {
        Ok(self.conn.execute(
            "UPDATE episodes SET played = 0, played_at = NULL, position_secs = 0
             WHERE show_id = ?1 AND played = 1",
            [show.0],
        )?)
    }

    /// Where playback of an episode got to, saved while it plays and when
    /// it stops. Saved to the second.
    pub fn set_position(&self, id: EpisodeId, position: Duration) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE episodes SET position_secs = ?2 WHERE id = ?1",
            params![id.0, i64::try_from(position.as_secs()).unwrap_or(i64::MAX)],
        )?;
        Ok(())
    }

    /// An episode played to its end: played, and back to the start for the
    /// next listen. The queue is left alone; moving on is the player's job.
    pub fn finish(&self, id: EpisodeId, now: DateTime<Utc>) -> Result<(), StoreError> {
        self.set_played(id, true, now)?;
        self.set_position(id, Duration::ZERO)
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

    /// Move a queued episode to `index` in the queue (Move Up, Move Down,
    /// a drag). Not queued: no change.
    pub fn move_queued(&self, id: EpisodeId, index: usize) -> Result<(), StoreError> {
        let order = crate::queue::moved(&self.queue()?, id, index);
        let tx = self.conn.unchecked_transaction()?;
        for (position, id) in order.iter().enumerate() {
            tx.execute(
                "UPDATE queue SET position = ?2 WHERE episode_id = ?1",
                params![id.0, i64::try_from(position).unwrap_or(i64::MAX) + 1],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn dequeue(&self, id: EpisodeId) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM queue WHERE episode_id = ?1", [id.0])?;
        Ok(())
    }

    /// Apply `change` to every episode in `ids`, in one transaction.
    /// Queued episodes join the end of the queue in the order given.
    pub fn change_episodes(
        &self,
        ids: &[EpisodeId],
        change: EpisodeChange,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        for &id in ids {
            match change {
                EpisodeChange::Queued(true) => self.enqueue(id)?,
                EpisodeChange::Queued(false) => self.dequeue(id)?,
                EpisodeChange::Favorite(favorite) => self.set_favorite(id, favorite)?,
                EpisodeChange::Played(played) => self.set_played(id, played, now)?,
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn show_prefs(&self, show: ShowId) -> Result<ShowPrefs, StoreError> {
        let (played, order, downloaded): (String, String, String) = self.conn.query_row(
            "SELECT episode_filter, sort_order, download_filter FROM shows WHERE id = ?1",
            [show.0],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        Ok(ShowPrefs {
            order: match order.as_str() {
                "oldest" => SortOrder::OldestFirst,
                _ => SortOrder::NewestFirst,
            },
            played: match played.as_str() {
                "played" => EpisodeFilter::Played,
                "unplayed" => EpisodeFilter::Unplayed,
                _ => EpisodeFilter::All,
            },
            downloaded: match downloaded.as_str() {
                "downloaded" => DownloadFilter::Downloaded,
                "not-downloaded" => DownloadFilter::NotDownloaded,
                _ => DownloadFilter::All,
            },
        })
    }

    pub fn set_show_prefs(&self, show: ShowId, prefs: ShowPrefs) -> Result<(), StoreError> {
        let order = match prefs.order {
            SortOrder::NewestFirst => "newest",
            SortOrder::OldestFirst => "oldest",
        };
        let played = match prefs.played {
            EpisodeFilter::All => "all",
            EpisodeFilter::Played => "played",
            EpisodeFilter::Unplayed => "unplayed",
        };
        let downloaded = match prefs.downloaded {
            DownloadFilter::All => "all",
            DownloadFilter::Downloaded => "downloaded",
            DownloadFilter::NotDownloaded => "not-downloaded",
        };
        self.conn.execute(
            "UPDATE shows SET episode_filter = ?2, sort_order = ?3, download_filter = ?4
             WHERE id = ?1",
            params![show.0, played, order, downloaded],
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
