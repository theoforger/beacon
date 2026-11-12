// SPDX-License-Identifier: GPL-3.0-or-later

//! What a section's search finds (`docs/specs/add-and-itunes.md`, Search
//! in the library). Each search field searches only its own page (decided
//! by the user 2026-11-11): Shows its shows, the other sections their own
//! episodes, in the section's own order. Matching uses the store's
//! full-text tables, every word a prefix.

use std::collections::HashSet;

use beacon_core::StoreError;
use beacon_core::store::Store;

use crate::navigation::Location;
use crate::ui::models::{self, EpisodeItem, ShowCard};

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResults {
    /// What was searched for, as typed.
    pub query: String,
    /// The section searched.
    pub section: Location,
    pub shows: Vec<ShowCard>,
    pub episodes: Vec<EpisodeItem>,
}

impl Default for SearchResults {
    fn default() -> Self {
        Self::new("", Location::Shows)
    }
}

impl SearchResults {
    pub fn new(query: &str, section: Location) -> Self {
        Self {
            query: query.to_owned(),
            section,
            shows: Vec::new(),
            episodes: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.shows.is_empty() && self.episodes.is_empty()
    }
}

/// Search `section` for `query`: Shows' cards, or `episodes`, the
/// section's own list as it shows it.
pub fn search(
    store: &Store,
    query: &str,
    section: Location,
    episodes: Vec<EpisodeItem>,
) -> Result<SearchResults, StoreError> {
    let mut results = SearchResults::new(query, section);
    if section == Location::Shows {
        let found: HashSet<_> = store.search_shows(query)?.into_iter().collect();
        results.shows = models::show_cards(store)?
            .into_iter()
            .filter(|card| found.contains(&card.id))
            .collect();
    } else {
        let found: HashSet<_> = store.search_episodes(query)?.into_iter().collect();
        results.episodes = episodes
            .into_iter()
            .filter(|item| found.contains(&item.episode.id))
            .collect();
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use beacon_core::model::{EpisodeInfo, ParsedEpisode, ShowInfo};
    use chrono::{TimeZone, Utc};
    use url::Url;

    use super::*;
    use crate::ui::models::Context;

    #[test]
    fn each_section_finds_only_its_own() {
        let now = Utc.with_ymd_and_hms(2026, 11, 10, 12, 0, 0).unwrap();
        let mut store = Store::open_in_memory().unwrap();
        let show = |store: &mut Store, url: &str, title: &str| {
            let info = ShowInfo {
                title: title.into(),
                ..Default::default()
            };
            store
                .insert_show(&Url::parse(url).unwrap(), &info, now)
                .unwrap()
        };
        let signal = show(&mut store, "https://a.example/feed", "The Long Signal");
        let other = show(&mut store, "https://b.example/feed", "Quiet Hours");
        let episode = |guid: &str, title: &str, description: &str| ParsedEpisode {
            info: EpisodeInfo {
                guid: guid.into(),
                title: title.into(),
                description: Some(description.into()),
                published: Some(now),
                ..Default::default()
            },
            chapters: Vec::new(),
        };
        store
            .sync_episodes(
                other,
                &[
                    episode("a", "Static", "A signal from far away"),
                    episode("b", "Signals", "Nothing else"),
                    episode("c", "Weather", "Rain"),
                ],
                now,
            )
            .unwrap();

        // Shows finds shows, not episodes.
        let shows = search(&store, "signal", Location::Shows, Vec::new()).unwrap();
        let ids: Vec<_> = shows.shows.iter().map(|c| c.id).collect();
        assert_eq!(ids, [signal]);
        assert!(shows.episodes.is_empty());

        // An episode list finds among its own episodes, in its own order.
        let context = Context::load(&store).unwrap();
        let mut list: Vec<_> = store
            .episodes(other)
            .unwrap()
            .into_iter()
            .map(|e| context.item(e))
            .collect();
        list.sort_by(|a, b| a.episode.info.title.cmp(&b.episode.info.title));
        let static_only = list
            .iter()
            .filter(|i| i.episode.info.title != "Signals")
            .cloned()
            .collect();
        let found = search(&store, "signal", Location::Favorites, static_only).unwrap();
        let titles: Vec<_> = found
            .episodes
            .iter()
            .map(|i| i.episode.info.title.as_str())
            .collect();
        assert_eq!(titles, ["Static"]);
        assert!(found.shows.is_empty());
        let found = search(&store, "signal", Location::New, list).unwrap();
        let titles: Vec<_> = found
            .episodes
            .iter()
            .map(|i| i.episode.info.title.as_str())
            .collect();
        assert_eq!(titles, ["Signals", "Static"]);

        assert!(
            search(&store, "thunder", Location::New, Vec::new())
                .unwrap()
                .is_empty()
        );
    }
}
