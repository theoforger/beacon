// SPDX-License-Identifier: GPL-3.0-or-later

//! View models: what the library views show, built from the store. The views
//! render these and send `RowAction`s back; they never read the store.

use std::collections::HashMap;
use std::path::PathBuf;

use beacon_core::StoreError;
use beacon_core::browse::{self, RecencyGroup};
use beacon_core::model::{Episode, EpisodeId, Season, Show, ShowId, ShowPrefs, SortOrder};
use beacon_core::store::Store;
use chrono::{DateTime, Duration, Local, Utc};

/// One episode as a row shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct EpisodeItem {
    pub episode: Episode,
    pub show_title: String,
    pub cover: Option<PathBuf>,
    pub in_queue: bool,
}

/// What a row's menu, or activating a row, asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAction {
    /// Play the episode from where it was left, or pause or resume it.
    Play,
    ShowDetails,
    ToggleQueue,
    /// Download, Cancel Download or Delete Download, as the episode is.
    ToggleDownload,
    ToggleFavorite,
    TogglePlayed,
    /// Start the episode from the beginning next time, leaving whether it
    /// is played alone.
    ResetProgress,
    /// Move Up and Move Down, on a queued row of the Queue view.
    MoveUp,
    MoveDown,
    /// The queued episode given was dragged onto this row: it takes this
    /// row's place.
    MoveHere(EpisodeId),
}

/// A tile on the Shows grid.
#[derive(Debug, Clone, PartialEq)]
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
    /// Every episode is played, whatever the filter shows: the page offers
    /// Mark All as Unplayed instead of Mark All as Played.
    pub all_played: bool,
    /// What Download All Episodes downloads: every episode with audio not
    /// yet downloaded, whatever the filter shows, newest first.
    pub to_download: Vec<EpisodeId>,
    /// The search in This Show scope the episodes are narrowed to.
    pub query: Option<String>,
}

/// What turning episodes into items needs: their shows and the queue.
pub(crate) struct Context {
    shows: HashMap<ShowId, Show>,
    queue: Vec<EpisodeId>,
}

impl Context {
    pub(crate) fn load(store: &Store) -> Result<Self, StoreError> {
        Ok(Self {
            shows: store.shows()?.into_iter().map(|s| (s.id, s)).collect(),
            queue: store.queue()?,
        })
    }

    pub(crate) fn queue(&self) -> &[EpisodeId] {
        &self.queue
    }

    pub(crate) fn show_title(&self, id: ShowId) -> Option<&str> {
        self.shows.get(&id).map(|s| s.info.title.as_str())
    }

    pub(crate) fn item(&self, episode: Episode) -> EpisodeItem {
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

/// The New view: recent episodes of every show that pass `prefs`' filters,
/// with their recency group. Newest first puts Today first; oldest first
/// reverses the groups and the episodes in them.
pub fn new_items(
    store: &Store,
    now: DateTime<Utc>,
    prefs: ShowPrefs,
) -> Result<Vec<(RecencyGroup, EpisodeItem)>, StoreError> {
    let context = Context::load(store)?;
    let today = now.with_timezone(&Local).date_naive();
    // A day of slack so the oldest local day is complete.
    let since = now - Duration::days(browse::NEW_VIEW_DAYS + 1);
    let mut items: Vec<_> = store
        .recent_episodes(since)?
        .into_iter()
        .filter(|e| prefs.accepts(e))
        .filter_map(|episode| {
            let published = episode.info.published?.with_timezone(&Local).date_naive();
            let group = browse::recency_group(published, today)?;
            Some((group, context.item(episode)))
        })
        .collect();
    if prefs.order == SortOrder::OldestFirst {
        items.reverse();
    }
    Ok(items)
}

/// The podcast page, its episodes narrowed to those matching `query` (a
/// search in This Show scope) when there is one.
pub fn show_page(
    store: &Store,
    id: ShowId,
    query: Option<&str>,
) -> Result<Option<ShowPageData>, StoreError> {
    let context = Context::load(store)?;
    let Some(show) = context.shows.get(&id).cloned() else {
        return Ok(None);
    };
    let prefs = store.show_prefs(id)?;
    let all = store.episodes(id)?;
    let all_played = !all.is_empty() && all.iter().all(|e| e.state.played);
    let to_download = all
        .iter()
        .filter(|e| e.info.enclosure.is_some() && e.state.download_path.is_none())
        .map(|e| e.id)
        .collect();
    let matches: Option<std::collections::HashSet<EpisodeId>> = match query {
        Some(query) => Some(store.search_show_episodes(id, query)?.into_iter().collect()),
        None => None,
    };
    let episodes: Vec<Episode> = all
        .into_iter()
        .filter(|e| prefs.accepts(e))
        .filter(|e| matches.as_ref().is_none_or(|m| m.contains(&e.id)))
        .collect();
    let (seasons, episodes) = arrange(episodes, prefs, |e| context.item(e));
    Ok(Some(ShowPageData {
        show,
        prefs,
        seasons,
        episodes,
        all_played,
        to_download,
        query: query.map(str::to_owned),
    }))
}

/// A page's episodes in seasons when the feed tags them, or else sorted.
pub(crate) fn arrange(
    mut episodes: Vec<Episode>,
    prefs: ShowPrefs,
    item: impl Fn(Episode) -> EpisodeItem,
) -> (Option<Vec<SeasonItems>>, Vec<EpisodeItem>) {
    let seasons = browse::group_by_season(episodes.clone(), prefs.order).map(|groups| {
        groups
            .into_iter()
            .map(|group| SeasonItems {
                season: group.season,
                episodes: group.episodes.into_iter().map(&item).collect(),
            })
            .collect()
    });
    if seasons.is_some() {
        return (seasons, Vec::new());
    }
    browse::sort_episodes(&mut episodes, prefs.order);
    (None, episodes.into_iter().map(item).collect())
}

/// The Favorites section, newest first.
pub fn favorite_items(store: &Store) -> Result<Vec<EpisodeItem>, StoreError> {
    let context = Context::load(store)?;
    Ok(store
        .favorite_episodes()?
        .into_iter()
        .map(|e| context.item(e))
        .collect())
}

/// The Downloaded section, newest first.
pub fn downloaded_items(store: &Store) -> Result<Vec<EpisodeItem>, StoreError> {
    let context = Context::load(store)?;
    Ok(store
        .downloaded_episodes()?
        .into_iter()
        .map(|e| context.item(e))
        .collect())
}

/// One episode with its show, for the details pane.
pub fn episode_item(store: &Store, id: EpisodeId) -> Result<Option<EpisodeItem>, StoreError> {
    let context = Context::load(store)?;
    Ok(store.episode(id)?.map(|e| context.item(e)))
}

#[cfg(test)]
mod tests {
    use beacon_core::model::{EpisodeFilter, EpisodeInfo, ShowInfo};
    use beacon_core::store::Store;
    use chrono::TimeZone;
    use url::Url;

    use super::*;

    fn store_with_recent_episodes(now: DateTime<Utc>) -> Store {
        let mut store = Store::open_in_memory().unwrap();
        let url = Url::parse("https://example.com/feed.xml").unwrap();
        let show = store.insert_show(&url, &ShowInfo::default(), now).unwrap();
        let episodes: Vec<_> = [(1, "today"), (2, "two days ago"), (10, "ten days ago")]
            .into_iter()
            .map(|(days, title)| beacon_core::model::ParsedEpisode {
                info: EpisodeInfo {
                    guid: title.into(),
                    title: title.into(),
                    published: Some(now - Duration::days(days - 1)),
                    ..Default::default()
                },
                chapters: Vec::new(),
            })
            .collect();
        store.sync_episodes(show, &episodes, now).unwrap();
        store
    }

    fn titles(items: &[(RecencyGroup, EpisodeItem)]) -> Vec<&str> {
        items
            .iter()
            .map(|(_, i)| i.episode.info.title.as_str())
            .collect()
    }

    #[test]
    fn new_sorts_both_ways_and_filters() {
        let now = Utc.with_ymd_and_hms(2026, 11, 2, 12, 0, 0).unwrap();
        let store = store_with_recent_episodes(now);

        let newest = new_items(&store, now, ShowPrefs::default()).unwrap();
        assert_eq!(titles(&newest), ["today", "two days ago", "ten days ago"]);
        assert_eq!(newest[0].0, RecencyGroup::Today);

        let oldest = ShowPrefs {
            order: SortOrder::OldestFirst,
            ..Default::default()
        };
        let oldest = new_items(&store, now, oldest).unwrap();
        assert_eq!(titles(&oldest), ["ten days ago", "two days ago", "today"]);
        assert_eq!(oldest[0].0, RecencyGroup::LastWeek);

        let played = ShowPrefs {
            played: EpisodeFilter::Played,
            ..Default::default()
        };
        assert!(new_items(&store, now, played).unwrap().is_empty());
    }
}
