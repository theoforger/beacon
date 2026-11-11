// SPDX-License-Identifier: GPL-3.0-or-later

//! What a search of the library finds (`docs/specs/add-and-itunes.md`,
//! Search in the library): the shows, as Shows tiles, then the episodes,
//! each best match first. Read from the store's full-text tables.

use beacon_core::StoreError;
use beacon_core::store::Store;

use crate::ui::models::{self, Context, EpisodeItem, ShowCard};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchResults {
    /// What was searched for, as typed.
    pub query: String,
    pub shows: Vec<ShowCard>,
    pub episodes: Vec<EpisodeItem>,
}

impl SearchResults {
    pub fn is_empty(&self) -> bool {
        self.shows.is_empty() && self.episodes.is_empty()
    }
}

pub fn search(store: &Store, query: &str) -> Result<SearchResults, StoreError> {
    let mut cards = models::show_cards(store)?;
    let shows = store
        .search_shows(query)?
        .into_iter()
        .filter_map(|id| {
            let at = cards.iter().position(|card| card.id == id)?;
            Some(cards.swap_remove(at))
        })
        .collect();
    let context = Context::load(store)?;
    let mut episodes = Vec::new();
    for id in store.search_episodes(query)? {
        if let Some(episode) = store.episode(id)? {
            episodes.push(context.item(episode));
        }
    }
    Ok(SearchResults {
        query: query.to_owned(),
        shows,
        episodes,
    })
}

#[cfg(test)]
mod tests {
    use beacon_core::model::{EpisodeInfo, ParsedEpisode, ShowInfo};
    use chrono::{TimeZone, Utc};
    use url::Url;

    use super::*;

    #[test]
    fn finds_shows_and_episodes_best_first() {
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

        let results = search(&store, "signal").unwrap();
        assert_eq!(results.query, "signal");
        let shows: Vec<_> = results.shows.iter().map(|c| c.id).collect();
        assert_eq!(shows, [signal]);
        // A match in the title ranks above one in the description.
        let titles: Vec<_> = results
            .episodes
            .iter()
            .map(|i| i.episode.info.title.as_str())
            .collect();
        assert_eq!(titles, ["Signals", "Static"]);
        assert_eq!(results.episodes[0].show_title, "Quiet Hours");

        assert!(search(&store, "thunder").unwrap().is_empty());
    }
}
