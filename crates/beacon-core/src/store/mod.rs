// SPDX-License-Identifier: GPL-3.0-or-later

//! SQLite storage. All writes for one feed sync happen in one transaction.

use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use rusqlite_migration::{M, Migrations};
use url::Url;

mod rows;

use self::rows::{EPISODE_COLUMNS, SHOW_COLUMNS, episode_from_row, episode_params, show_from_row};
use crate::error::StoreError;
use crate::model::{Chapter, Episode, EpisodeId, ParsedEpisode, Show, ShowId, ShowInfo};

static MIGRATIONS: LazyLock<Migrations<'static>> =
    LazyLock::new(|| Migrations::new(vec![M::up(include_str!("migrations/001_initial.sql"))]));

/// What a sync changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncStats {
    pub added: usize,
    pub updated: usize,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self, StoreError> {
        conn.pragma_update(None, "foreign_keys", true)?;
        MIGRATIONS.to_latest(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn show_id_for_feed(&self, feed_url: &Url) -> Result<Option<ShowId>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT id FROM shows WHERE feed_url = ?1",
                [feed_url],
                |row| row.get(0).map(ShowId),
            )
            .optional()?)
    }

    pub fn insert_show(
        &self,
        feed_url: &Url,
        info: &ShowInfo,
        now: DateTime<Utc>,
    ) -> Result<ShowId, StoreError> {
        self.conn.execute(
            "INSERT INTO shows (feed_url, title, author, description, link, image_url, category,
                                subscribed_at, last_refreshed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                feed_url,
                info.title,
                info.author,
                info.description,
                info.link,
                info.image_url,
                info.category,
                now
            ],
        )?;
        Ok(ShowId(self.conn.last_insert_rowid()))
    }

    pub fn update_show(
        &self,
        id: ShowId,
        info: &ShowInfo,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE shows SET title = ?2, author = ?3, description = ?4, link = ?5,
                              image_url = ?6, category = ?7, last_refreshed_at = ?8
             WHERE id = ?1",
            params![
                id.0,
                info.title,
                info.author,
                info.description,
                info.link,
                info.image_url,
                info.category,
                now
            ],
        )?;
        Ok(())
    }

    pub fn set_cover_path(&self, id: ShowId, path: &Path) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE shows SET cover_path = ?2 WHERE id = ?1",
            params![id.0, path.to_string_lossy()],
        )?;
        Ok(())
    }

    pub fn show(&self, id: ShowId) -> Result<Option<Show>, StoreError> {
        Ok(self
            .conn
            .query_row(
                &format!("{SHOW_COLUMNS} WHERE id = ?1"),
                [id.0],
                show_from_row,
            )
            .optional()?)
    }

    pub fn shows(&self) -> Result<Vec<Show>, StoreError> {
        let mut stmt = self
            .conn
            .prepare(&format!("{SHOW_COLUMNS} ORDER BY title COLLATE NOCASE"))?;
        let shows = stmt
            .query_map([], show_from_row)?
            .collect::<Result<_, _>>()?;
        Ok(shows)
    }

    /// Insert episodes that are new to the show and refresh the metadata of
    /// known ones. The listener's state is never touched. New episodes start
    /// unseen.
    pub fn sync_episodes(
        &mut self,
        show_id: ShowId,
        episodes: &[ParsedEpisode],
        now: DateTime<Utc>,
    ) -> Result<SyncStats, StoreError> {
        let tx = self.conn.transaction()?;
        let mut stats = SyncStats::default();
        for episode in episodes {
            let info = &episode.info;
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT id FROM episodes WHERE show_id = ?1 AND guid = ?2",
                    params![show_id.0, info.guid],
                    |row| row.get(0),
                )
                .optional()?;
            let id = match existing {
                Some(id) => {
                    let changed = tx.execute(
                        "UPDATE episodes SET title = ?2, description = ?3, published_at = ?4,
                             duration_secs = ?5, enclosure_url = ?6, enclosure_type = ?7,
                             enclosure_length = ?8, season_number = ?9, season_name = ?10,
                             episode_number = ?11, image_url = ?12, chapters_url = ?13
                         WHERE id = ?1 AND (title, description, published_at, duration_secs,
                             enclosure_url, enclosure_type, enclosure_length, season_number,
                             season_name, episode_number, image_url, chapters_url)
                             IS NOT (?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                        rusqlite::params_from_iter(episode_params(id, info)),
                    )?;
                    stats.updated += changed;
                    id
                }
                None => {
                    let mut values = episode_params(show_id.0, info);
                    values.push(Box::new(info.guid.clone()));
                    values.push(Box::new(now));
                    tx.execute(
                        "INSERT INTO episodes (show_id, title, description, published_at,
                             duration_secs, enclosure_url, enclosure_type, enclosure_length,
                             season_number, season_name, episode_number, image_url, chapters_url,
                             guid, added_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                        rusqlite::params_from_iter(values),
                    )?;
                    stats.added += 1;
                    tx.last_insert_rowid()
                }
            };
            if !episode.chapters.is_empty() {
                tx.execute("DELETE FROM chapters WHERE episode_id = ?1", [id])?;
                for chapter in &episode.chapters {
                    tx.execute(
                        "INSERT OR REPLACE INTO chapters (episode_id, start_ms, title, url, image_url)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            id,
                            chapter.start.as_millis() as i64,
                            chapter.title,
                            chapter.url,
                            chapter.image_url
                        ],
                    )?;
                }
            }
        }
        tx.commit()?;
        Ok(stats)
    }

    /// Mark every episode of a show as seen except the newest. Used right
    /// after subscribing, so a new show brings one blue dot, not hundreds.
    pub fn mark_seen_except_newest(&self, show_id: ShowId) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE episodes SET seen = 1
             WHERE show_id = ?1 AND id IS NOT (
                 SELECT id FROM episodes WHERE show_id = ?1
                 ORDER BY published_at DESC, id DESC LIMIT 1)",
            [show_id.0],
        )?;
        Ok(())
    }

    /// Episodes of a show, newest first.
    pub fn episodes(&self, show_id: ShowId) -> Result<Vec<Episode>, StoreError> {
        let mut stmt = self.conn.prepare(&format!(
            "{EPISODE_COLUMNS} WHERE show_id = ?1 ORDER BY published_at DESC, id DESC"
        ))?;
        let episodes = stmt
            .query_map([show_id.0], episode_from_row)?
            .collect::<Result<_, _>>()?;
        Ok(episodes)
    }

    pub fn chapters(&self, episode_id: EpisodeId) -> Result<Vec<Chapter>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT start_ms, title, url, image_url FROM chapters
             WHERE episode_id = ?1 ORDER BY start_ms",
        )?;
        let chapters = stmt
            .query_map([episode_id.0], |row| {
                Ok(Chapter {
                    start: Duration::from_millis(row.get::<_, i64>(0)? as u64),
                    title: row.get(1)?,
                    url: row.get(2)?,
                    image_url: row.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(chapters)
    }

    /// Episodes whose title or description match a full-text query.
    pub fn search_episodes(&self, query: &str) -> Result<Vec<EpisodeId>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT rowid FROM episodes_fts WHERE episodes_fts MATCH ?1 ORDER BY rank")?;
        let ids = stmt
            .query_map([fts_phrase(query)], |row| row.get(0).map(EpisodeId))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }
}

/// Quote user input as an FTS5 phrase prefix, so punctuation cannot break the
/// query syntax.
fn fts_phrase(query: &str) -> String {
    format!("\"{}\"*", query.replace('"', "\"\""))
}

#[cfg(test)]
mod tests;
