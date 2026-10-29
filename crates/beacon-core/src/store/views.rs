// SPDX-License-Identifier: GPL-3.0-or-later

//! Queries behind the library views and the sidebar counts.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::Store;
use super::rows::{EPISODE_COLUMNS, episode_from_row};
use crate::error::StoreError;
use crate::model::{Episode, LibraryCounts, ShowId};

/// The "new" rule from `EpisodeState::is_new`, in SQL.
const IS_NEW: &str = "seen = 0 AND played = 0";

impl Store {
    pub fn counts(&self) -> Result<LibraryCounts, StoreError> {
        let (shows_with_new, new_episodes): (u32, u32) = self.conn.query_row(
            &format!("SELECT count(DISTINCT show_id), count(*) FROM episodes WHERE {IS_NEW}"),
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let queue: u32 = self
            .conn
            .query_row("SELECT count(*) FROM queue", [], |row| row.get(0))?;
        Ok(LibraryCounts {
            shows_with_new,
            new_episodes,
            queue,
        })
    }

    /// New episodes per show, for the badges on the Shows grid. Shows
    /// without new episodes are absent.
    pub fn new_counts(&self) -> Result<HashMap<ShowId, u32>, StoreError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT show_id, count(*) FROM episodes WHERE {IS_NEW} GROUP BY show_id"
        ))?;
        let counts = stmt
            .query_map([], |row| Ok((ShowId(row.get(0)?), row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        Ok(counts)
    }

    /// Episodes of every show published since `since`, newest first.
    pub fn recent_episodes(&self, since: DateTime<Utc>) -> Result<Vec<Episode>, StoreError> {
        let mut stmt = self.conn.prepare(&format!(
            "{EPISODE_COLUMNS} WHERE published_at >= ?1 ORDER BY published_at DESC, id DESC"
        ))?;
        let episodes = stmt
            .query_map([since], episode_from_row)?
            .collect::<Result<_, _>>()?;
        Ok(episodes)
    }
}
