// SPDX-License-Identifier: GPL-3.0-or-later

//! View models: what the library views show, built from the store. The views
//! render these and send `RowAction`s back; they never read the store.

use std::collections::HashMap;
use std::path::PathBuf;

use beacon_core::StoreError;
use beacon_core::browse::{self, RecencyGroup};
use beacon_core::model::{Episode, EpisodeFilter, EpisodeId, Season, Show, ShowId, ShowPrefs};
use beacon_core::store::Store;
use chrono::{DateTime, Duration, Local, Utc};

/// One episode as a row shows it.
#[derive(Debug, Clone)]
pub struct EpisodeItem {
    pub episode: Episode,
    pub show_title: String,
    pub cover: Option<PathBuf>,
    pub in_queue: bool,
}

/// What a row's menu, or activating a row, asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAction {
    ShowDetails,
    ToggleQueue,
    ToggleFavorite,
    TogglePlayed,
}

/// A tile on the Shows grid.
#[derive(Debug, Clone)]
pub struct ShowCard {
    pub id: ShowId,
    pub title: String,
    pub author: Option<String>,
    pub cover: Option<PathBuf>,
    pub new_count: u32,
}

/// One season on a podcast page.
#[derive(Debug, Clone)]
pub struct SeasonItems {
    pub season: Option<Season>,
    pub episodes: Vec<EpisodeItem>,
}

/// Everything a podcast page shows.
#[derive(Debug, Clone)]
pub struct ShowPageData {
    pub show: Show,
    pub prefs: ShowPrefs,
    /// `Some` when the feed has season tags.
    pub seasons: Option<Vec<SeasonItems>>,
    /// The filtered, sorted episodes when there are no seasons.
    pub episodes: Vec<EpisodeItem>,
}

struct Context {
    shows: HashMap<ShowId, Show>,
    queue: Vec<EpisodeId>,
}

impl Context {
    fn load(store: &Store) -> Result<Self, StoreError> {
        Ok(Self {
            shows: store.shows()?.into_iter().map(|s| (s.id, s)).collect(),
            queue: store.queue()?,
        })
    }

    fn item(&self, episode: Episode) -> EpisodeItem {
        let show = self.shows.get(&episode.show_id);
        EpisodeItem {
            show_title: show.map(|s| s.info.title.clone()).unwrap_or_default(),
            cover: show.and_then(|s| s.cover_path.clone()),
            in_queue: self.queue.contains(&episode.id),
            episode,
        }
    }
}

pub fn show_cards(store: &Store) -> Result<Vec<ShowCard>, StoreError> {
    let counts = store.new_counts()?;
    Ok(store
        .shows()?
        .into_iter()
        .map(|show| ShowCard {
            new_count: counts.get(&show.id).copied().unwrap_or(0),
            id: show.id,
            title: show.info.title,
            author: show.info.author,
            cover: show.cover_path,
        })
        .collect())
}

/// The New view: recent episodes of every show with their recency group,
/// newest first.
pub fn new_items(
    store: &Store,
    now: DateTime<Utc>,
    filter: EpisodeFilter,
) -> Result<Vec<(RecencyGroup, EpisodeItem)>, StoreError> {
    let context = Context::load(store)?;
    let today = now.with_timezone(&Local).date_naive();
    // A day of slack so the oldest local day is complete.
    let since = now - Duration::days(browse::NEW_VIEW_DAYS + 1);
    Ok(store
        .recent_episodes(since)?
        .into_iter()
        .filter(|e| filter.accepts(e))
        .filter_map(|episode| {
            let published = episode.info.published?.with_timezone(&Local).date_naive();
            let group = browse::recency_group(published, today)?;
            Some((group, context.item(episode)))
        })
        .collect())
}

pub fn show_page(store: &Store, id: ShowId) -> Result<Option<ShowPageData>, StoreError> {
    let context = Context::load(store)?;
    let Some(show) = context.shows.get(&id).cloned() else {
        return Ok(None);
    };
    let prefs = store.show_prefs(id)?;
    let mut episodes: Vec<Episode> = store
        .episodes(id)?
        .into_iter()
        .filter(|e| prefs.filter.accepts(e))
        .collect();

    let seasons = browse::group_by_season(episodes.clone(), prefs.order).map(|groups| {
        groups
            .into_iter()
            .map(|group| SeasonItems {
                season: group.season,
                episodes: group
                    .episodes
                    .into_iter()
                    .map(|e| context.item(e))
                    .collect(),
            })
            .collect()
    });
    let episodes = if seasons.is_some() {
        Vec::new()
    } else {
        browse::sort_episodes(&mut episodes, prefs.order);
        episodes.into_iter().map(|e| context.item(e)).collect()
    };
    Ok(Some(ShowPageData {
        show,
        prefs,
        seasons,
        episodes,
    }))
}

/// One episode with its show, for the details pane.
pub fn episode_item(store: &Store, id: EpisodeId) -> Result<Option<EpisodeItem>, StoreError> {
    let context = Context::load(store)?;
    Ok(store.episode(id)?.map(|e| context.item(e)))
}
