// SPDX-License-Identifier: GPL-3.0-or-later

//! Domain types. `*Info` types hold what a feed says; `Show` and `Episode`
//! add the database identity and the listener's state.

use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShowId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EpisodeId(pub i64);

/// Show metadata as published in the feed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShowInfo {
    pub title: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub link: Option<Url>,
    pub image_url: Option<Url>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Show {
    pub id: ShowId,
    pub feed_url: Url,
    pub info: ShowInfo,
    /// Local copy of `info.image_url`, once cached.
    pub cover_path: Option<PathBuf>,
    pub subscribed_at: DateTime<Utc>,
    pub last_refreshed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Season {
    pub number: u32,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Enclosure {
    pub url: Url,
    pub mime_type: Option<String>,
    pub length: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chapter {
    pub start: Duration,
    pub title: String,
    pub url: Option<Url>,
    pub image_url: Option<Url>,
}

/// Episode metadata as published in the feed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EpisodeInfo {
    /// Stable identity within the show: the feed's GUID, or a fallback that
    /// survives refreshes (see `services::feeds`).
    pub guid: String,
    pub title: String,
    pub description: Option<String>,
    pub published: Option<DateTime<Utc>>,
    pub duration: Option<Duration>,
    pub enclosure: Option<Enclosure>,
    pub season: Option<Season>,
    pub number: Option<f32>,
    pub image_url: Option<Url>,
    /// Podcasting 2.0 `<podcast:chapters>` JSON, fetched on demand.
    pub chapters_url: Option<Url>,
}

/// What the listener has done with an episode. Never touched by a refresh.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EpisodeState {
    /// False shows the blue dot. Viewing the details sets it; marking an
    /// episode unplayed does not clear it.
    pub seen: bool,
    pub played: bool,
    pub played_at: Option<DateTime<Utc>>,
    pub position: Duration,
    pub favorite: bool,
    pub download_path: Option<PathBuf>,
    pub downloaded_at: Option<DateTime<Utc>>,
}

impl EpisodeState {
    /// New: not yet seen and not played. Shows the blue dot and counts
    /// towards the Shows badges and the New count.
    pub fn is_new(&self) -> bool {
        !self.seen && !self.played
    }

    /// Started but not finished: shows a progress bar.
    pub fn is_partly_played(&self) -> bool {
        !self.played && !self.position.is_zero()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Episode {
    pub id: EpisodeId,
    pub show_id: ShowId,
    pub info: EpisodeInfo,
    pub state: EpisodeState,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueItem {
    pub episode_id: EpisodeId,
    pub position: u32,
}

/// One episode as parsed from a feed, with its inline chapters.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParsedEpisode {
    pub info: EpisodeInfo,
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParsedFeed {
    pub show: ShowInfo,
    pub episodes: Vec<ParsedEpisode>,
}

/// Which episodes a list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EpisodeFilter {
    #[default]
    All,
    Unplayed,
}

impl EpisodeFilter {
    pub fn accepts(self, episode: &Episode) -> bool {
        match self {
            EpisodeFilter::All => true,
            EpisodeFilter::Unplayed => !episode.state.played,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    NewestFirst,
    OldestFirst,
}

/// How a podcast page lists its episodes, remembered per show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShowPrefs {
    pub filter: EpisodeFilter,
    pub order: SortOrder,
}

/// The numbers beside the sidebar items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LibraryCounts {
    /// Shows with at least one new episode.
    pub shows_with_new: u32,
    pub new_episodes: u32,
    pub queue: u32,
}
