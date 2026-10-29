// SPDX-License-Identifier: GPL-3.0-or-later

//! Mapping between table rows and model types.

use std::path::PathBuf;
use std::time::Duration;

use rusqlite::Row;
use url::Url;

use crate::model::{
    Enclosure, Episode, EpisodeId, EpisodeInfo, EpisodeState, Season, Show, ShowId, ShowInfo,
};

pub(super) const SHOW_COLUMNS: &str =
    "SELECT id, feed_url, title, author, description, link, image_url,
    category, cover_path, subscribed_at, last_refreshed_at FROM shows";

pub(super) fn show_from_row(row: &Row) -> rusqlite::Result<Show> {
    Ok(Show {
        id: ShowId(row.get(0)?),
        feed_url: row.get(1)?,
        info: ShowInfo {
            title: row.get(2)?,
            author: row.get(3)?,
            description: row.get(4)?,
            link: row.get(5)?,
            image_url: row.get(6)?,
            category: row.get(7)?,
        },
        cover_path: row.get::<_, Option<String>>(8)?.map(PathBuf::from),
        subscribed_at: row.get(9)?,
        last_refreshed_at: row.get(10)?,
    })
}

/// Parameters ?1 to ?13 shared by the episode insert and update.
pub(super) fn episode_params(first: i64, info: &EpisodeInfo) -> Vec<Box<dyn rusqlite::ToSql>> {
    let enclosure = info.enclosure.as_ref();
    vec![
        Box::new(first),
        Box::new(info.title.clone()),
        Box::new(info.description.clone()),
        Box::new(info.published),
        Box::new(info.duration.map(|d| d.as_secs() as i64)),
        Box::new(enclosure.map(|e| e.url.clone())),
        Box::new(enclosure.and_then(|e| e.mime_type.clone())),
        Box::new(enclosure.and_then(|e| e.length.map(|l| l as i64))),
        Box::new(info.season.as_ref().map(|s| s.number)),
        Box::new(info.season.as_ref().and_then(|s| s.name.clone())),
        Box::new(info.number.map(f64::from)),
        Box::new(info.image_url.clone()),
        Box::new(info.chapters_url.clone()),
    ]
}

pub(super) const EPISODE_COLUMNS: &str =
    "SELECT id, show_id, guid, title, description, published_at,
    duration_secs, enclosure_url, enclosure_type, enclosure_length, season_number, season_name,
    episode_number, image_url, chapters_url, seen, played, played_at, position_secs, favorite,
    download_path, downloaded_at, added_at FROM episodes";

pub(super) fn episode_from_row(row: &Row) -> rusqlite::Result<Episode> {
    let enclosure_url: Option<Url> = row.get(7)?;
    let season_number: Option<u32> = row.get(10)?;
    Ok(Episode {
        id: EpisodeId(row.get(0)?),
        show_id: ShowId(row.get(1)?),
        info: EpisodeInfo {
            guid: row.get(2)?,
            title: row.get(3)?,
            description: row.get(4)?,
            published: row.get(5)?,
            duration: row
                .get::<_, Option<i64>>(6)?
                .map(|s| Duration::from_secs(s as u64)),
            enclosure: match enclosure_url {
                Some(url) => Some(Enclosure {
                    url,
                    mime_type: row.get(8)?,
                    length: row.get::<_, Option<i64>>(9)?.map(|l| l as u64),
                }),
                None => None,
            },
            season: match season_number {
                Some(number) => Some(Season {
                    number,
                    name: row.get(11)?,
                }),
                None => None,
            },
            number: row.get::<_, Option<f64>>(12)?.map(|n| n as f32),
            image_url: row.get(13)?,
            chapters_url: row.get(14)?,
        },
        state: EpisodeState {
            seen: row.get(15)?,
            played: row.get(16)?,
            played_at: row.get(17)?,
            position: Duration::from_secs(row.get::<_, i64>(18)? as u64),
            favorite: row.get(19)?,
            download_path: row.get::<_, Option<String>>(20)?.map(PathBuf::from),
            downloaded_at: row.get(21)?,
        },
        added_at: row.get(22)?,
    })
}
