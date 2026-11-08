// SPDX-License-Identifier: GPL-3.0-or-later

//! What plays next, read from the store: the Queue view's three groups, and
//! the episode Next and auto-continue go to (`beacon_core::queue`). The
//! source list is read again in the order its view shows it, filters and
//! sort included, so what follows is what the user saw follow.

use beacon_core::StoreError;
use beacon_core::model::{EpisodeId, ShowPrefs};
use beacon_core::queue::{self, Source, UpNext};
use beacon_core::store::Store;
use chrono::{DateTime, Utc};

use crate::ui::models::{self, Context, EpisodeItem};

/// "Next from" shows this many of the source list, as the mockup does.
pub const FROM_SOURCE_SHOWN: usize = 10;

/// Everything the Queue view shows.
#[derive(Debug, Clone, Default)]
pub struct QueueData {
    pub now_playing: Option<EpisodeItem>,
    pub queued: Vec<EpisodeItem>,
    /// The first of the source list's rest, at most `FROM_SOURCE_SHOWN`.
    pub from_source: Vec<EpisodeItem>,
    /// The source list's name, for "Next from …".
    pub source_name: Option<String>,
    /// What plays after the playing episode (Next Episode, auto-continue),
    /// which the player preloads.
    pub next: Option<EpisodeItem>,
}

/// Where a list's order comes from when it is read again.
#[derive(Debug, Clone, Copy)]
pub struct Lists {
    pub now: DateTime<Utc>,
    pub new_prefs: ShowPrefs,
}

/// The ids of `source`'s list, in the order its view shows them.
fn source_ids(store: &Store, source: Source, lists: Lists) -> Result<Vec<EpisodeId>, StoreError> {
    Ok(match source {
        Source::New => models::new_items(store, lists.now, lists.new_prefs)?
            .into_iter()
            .map(|(_, item)| item.episode.id)
            .collect(),
        Source::Show(id) => match models::show_page(store, id)? {
            Some(page) => match page.seasons {
                Some(seasons) => seasons
                    .into_iter()
                    .flat_map(|s| s.episodes)
                    .map(|item| item.episode.id)
                    .collect(),
                None => page.episodes.into_iter().map(|i| i.episode.id).collect(),
            },
            None => Vec::new(),
        },
        // Their views arrive later in M5.
        Source::Favorites | Source::Downloaded => Vec::new(),
    })
}

fn up_next(
    store: &Store,
    context: &Context,
    current: Option<EpisodeId>,
    source: Option<Source>,
    lists: Lists,
) -> Result<UpNext, StoreError> {
    let source = match source {
        Some(source) => source_ids(store, source, lists)?,
        None => Vec::new(),
    };
    Ok(queue::up_next(context.queue(), &source, current))
}

/// The episode after `current`: the queue's first, else the next in its
/// source list.
pub fn next_item(
    store: &Store,
    current: Option<EpisodeId>,
    source: Option<Source>,
    lists: Lists,
) -> Result<Option<EpisodeItem>, StoreError> {
    let context = Context::load(store)?;
    let next = up_next(store, &context, current, source, lists)?;
    match next.first() {
        Some(id) => Ok(store.episode(id)?.map(|e| context.item(e))),
        None => Ok(None),
    }
}

/// The Queue view's groups.
pub fn queue_data(
    store: &Store,
    current: Option<EpisodeId>,
    source: Option<Source>,
    lists: Lists,
) -> Result<QueueData, StoreError> {
    let context = Context::load(store)?;
    let next = up_next(store, &context, current, source, lists)?;
    let items = |ids: &[EpisodeId]| -> Result<Vec<EpisodeItem>, StoreError> {
        let mut items = Vec::new();
        for id in ids {
            if let Some(episode) = store.episode(*id)? {
                items.push(context.item(episode));
            }
        }
        Ok(items)
    };
    let now_playing = match current {
        Some(id) => store.episode(id)?.map(|e| context.item(e)),
        None => None,
    };
    let shown = &next.from_source[..next.from_source.len().min(FROM_SOURCE_SHOWN)];
    let following = match next.first() {
        Some(id) => store.episode(id)?.map(|e| context.item(e)),
        None => None,
    };
    Ok(QueueData {
        now_playing,
        queued: items(&next.queued)?,
        from_source: items(shown)?,
        source_name: source.map(|source| source_name(&context, source)),
        next: following,
    })
}

fn source_name(context: &Context, source: Source) -> String {
    use crate::navigation::Location;
    use crate::ui::labels;
    match source {
        Source::New => labels::title(Location::New),
        Source::Favorites => labels::title(Location::Favorites),
        Source::Downloaded => labels::title(Location::Downloaded),
        Source::Show(id) => context.show_title(id).unwrap_or_default().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use beacon_core::model::{EpisodeInfo, ParsedEpisode, ShowInfo};
    use chrono::{Duration, TimeZone};
    use url::Url;

    use super::*;

    /// A show of three episodes a day apart, the newest first on its page.
    fn library() -> (Store, beacon_core::model::ShowId, Vec<EpisodeId>, Lists) {
        let now = Utc.with_ymd_and_hms(2026, 11, 2, 12, 0, 0).unwrap();
        let mut store = Store::open_in_memory().unwrap();
        let url = Url::parse("https://example.com/feed.xml").unwrap();
        let info = ShowInfo {
            title: "The Long Signal".into(),
            ..Default::default()
        };
        let show = store.insert_show(&url, &info, now).unwrap();
        let episodes: Vec<_> = (1..=3)
            .map(|day| ParsedEpisode {
                info: EpisodeInfo {
                    guid: format!("e{day}"),
                    title: format!("Day {day}"),
                    published: Some(now - Duration::days(3 - day)),
                    ..Default::default()
                },
                chapters: Vec::new(),
            })
            .collect();
        store.sync_episodes(show, &episodes, now).unwrap();
        // Newest first, as the page shows them.
        let ids = store.episodes(show).unwrap().iter().map(|e| e.id).collect();
        let lists = Lists {
            now,
            new_prefs: ShowPrefs::default(),
        };
        (store, show, ids, lists)
    }

    fn title(item: Option<EpisodeItem>) -> Option<String> {
        item.map(|i| i.episode.info.title)
    }

    #[test]
    fn next_plays_the_queue_then_the_rest_of_the_page() {
        let (store, show, ids, lists) = library();
        let source = Some(Source::Show(show));
        let page: Vec<_> = ids.clone();
        // The newest playing: the next on its page follows.
        assert_eq!(
            title(next_item(&store, Some(page[0]), source, lists).unwrap()),
            Some("Day 2".into())
        );
        // A queued episode comes first.
        store.enqueue(page[2]).unwrap();
        assert_eq!(
            title(next_item(&store, Some(page[0]), source, lists).unwrap()),
            Some("Day 1".into())
        );
        let data = queue_data(&store, Some(page[0]), source, lists).unwrap();
        assert_eq!(data.queued.len(), 1);
        assert_eq!(
            title(data.from_source.first().cloned()),
            Some("Day 2".into())
        );
        assert_eq!(data.source_name.as_deref(), Some("The Long Signal"));
        assert_eq!(title(data.next), Some("Day 1".into()));

        // The last on the page, nothing queued: nothing next.
        store.dequeue(page[2]).unwrap();
        let last = queue_data(&store, Some(page[2]), source, lists).unwrap();
        assert!(last.next.is_none());
        assert_eq!(
            next_item(&store, Some(page[2]), source, lists).unwrap(),
            None
        );
    }
}
