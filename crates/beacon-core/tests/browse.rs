// SPDX-License-Identifier: GPL-3.0-or-later

//! M3 scenarios on the data side: subscribe, see the episodes grouped as a
//! podcast page shows them, use the All and Unplayed filter, and the played,
//! seen, favourite and queue rules behind the row menu.

use std::path::Path;
use std::sync::Arc;

use beacon_core::browse::{self, RecencyGroup};
use beacon_core::model::{EpisodeFilter, LibraryCounts, ShowPrefs, SortOrder};
use beacon_core::services::clock::Clock;
use beacon_core::services::covers::CoverCache;
use beacon_core::services::feeds::FeedClient;
use beacon_core::services::{http_client, library::Library};
use beacon_core::store::Store;
use chrono::{DateTime, Duration, TimeZone, Utc};
use httpmock::prelude::*;
use url::Url;

struct FixedClock(DateTime<Utc>);

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 29, 12, 0, 0).unwrap()
}

async fn subscribed(
    server: &MockServer,
    fixture: &str,
    store: Store,
) -> (Library, beacon_core::model::ShowId) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(fixture);
    let body = std::fs::read_to_string(path)
        .unwrap()
        .replace("{{BASE}}", &server.base_url());
    server
        .mock_async(|when, then| {
            when.method(GET).path("/feed.xml");
            then.status(200).body(body);
        })
        .await;
    let cache = std::env::temp_dir().join(format!("beacon-test-covers-{}", std::process::id()));
    let http = http_client();
    let library = Library::new(
        store,
        FeedClient::new(http.clone()),
        CoverCache::new(cache, http),
        Arc::new(FixedClock(now())),
    );
    let id = library
        .subscribe(&Url::parse(&server.url("/feed.xml")).unwrap())
        .await
        .unwrap();
    (library, id)
}

#[tokio::test]
async fn a_podcast_page_groups_seasons_and_filters_unplayed() {
    let server = MockServer::start_async().await;
    let (library, show) =
        subscribed(&server, "seasons.xml", Store::open_in_memory().unwrap()).await;

    library.with_store(|store| {
        let episodes = store.episodes(show).unwrap();
        let groups = browse::group_by_season(episodes.clone(), SortOrder::NewestFirst).unwrap();
        let summary: Vec<_> = groups
            .iter()
            .map(|g| {
                let season = g.season.as_ref().unwrap();
                (season.number, season.name.clone(), g.episodes.len())
            })
            .collect();
        assert_eq!(summary, [(3, Some("Deserts".into()), 2), (1, None, 1)]);

        // Mark the newest played; the Unplayed filter hides it.
        store.set_played(episodes[0].id, true, now()).unwrap();
        let unplayed: Vec<_> = store
            .episodes(show)
            .unwrap()
            .into_iter()
            .filter(|e| EpisodeFilter::Unplayed.accepts(e))
            .map(|e| e.info.title)
            .collect();
        assert_eq!(unplayed, ["Salt flats", "The first echo"]);

        // The page's choices are remembered per show.
        let prefs = ShowPrefs {
            filter: EpisodeFilter::Unplayed,
            order: SortOrder::OldestFirst,
        };
        store.set_show_prefs(show, prefs).unwrap();
        assert_eq!(store.show_prefs(show).unwrap(), prefs);
    });
}

#[tokio::test]
async fn marking_played_never_touches_the_queue() {
    let server = MockServer::start_async().await;
    let (library, show) = subscribed(&server, "plain.xml", Store::open_in_memory().unwrap()).await;
    library.with_store(|store| {
        let episodes = store.episodes(show).unwrap();
        let ids: Vec<_> = episodes.iter().map(|e| e.id).collect();
        store.enqueue(ids[1]).unwrap();
        store.enqueue(ids[0]).unwrap();
        store.enqueue(ids[1]).unwrap(); // already queued: no change

        store.set_played(ids[0], true, now()).unwrap();
        store.set_played(ids[0], false, now()).unwrap();
        assert_eq!(store.mark_all_played(show, now()).unwrap(), 2);
        assert_eq!(store.queue().unwrap(), [ids[1], ids[0]]);

        store.dequeue(ids[1]).unwrap();
        assert_eq!(store.queue().unwrap(), [ids[0]]);
    });
}

#[tokio::test]
async fn the_blue_dot_goes_when_seen_and_stays_gone() {
    let server = MockServer::start_async().await;
    let (library, show) = subscribed(&server, "plain.xml", Store::open_in_memory().unwrap()).await;
    library.with_store(|store| {
        // Subscribing leaves only the newest episode new.
        assert_eq!(
            store.counts().unwrap(),
            LibraryCounts {
                shows_with_new: 1,
                new_episodes: 1,
                queue: 0
            }
        );
        assert_eq!(store.new_counts().unwrap().get(&show), Some(&1));

        let newest = store.episodes(show).unwrap()[0].id;
        store.mark_seen(newest).unwrap();
        store.set_played(newest, true, now()).unwrap();
        store.set_played(newest, false, now()).unwrap();
        let episode = store.episode(newest).unwrap().unwrap();
        assert!(
            !episode.state.is_new(),
            "Mark as Unplayed does not bring the dot back"
        );
        assert!(!episode.state.played);
        assert_eq!(store.counts().unwrap(), LibraryCounts::default());
        assert!(store.new_counts().unwrap().is_empty());
    });
}

#[tokio::test]
async fn favorites_and_unsubscribing() {
    let server = MockServer::start_async().await;
    let (library, show) = subscribed(&server, "plain.xml", Store::open_in_memory().unwrap()).await;
    library.with_store(|store| {
        let id = store.episodes(show).unwrap()[0].id;
        store.set_favorite(id, true).unwrap();
        assert!(store.episode(id).unwrap().unwrap().state.favorite);
        store.enqueue(id).unwrap();

        store.delete_show(show).unwrap();
        assert!(store.shows().unwrap().is_empty());
        assert!(store.episode(id).unwrap().is_none());
        assert!(
            store.queue().unwrap().is_empty(),
            "queue entries go with the show"
        );
    });
}

#[tokio::test]
async fn the_new_view_sees_recent_episodes_in_recency_groups() {
    let server = MockServer::start_async().await;
    let (library, _show) =
        subscribed(&server, "seasons.xml", Store::open_in_memory().unwrap()).await;
    library.with_store(|store| {
        let since = now() - Duration::days(browse::NEW_VIEW_DAYS);
        let today = now().date_naive();
        let groups: Vec<_> = store
            .recent_episodes(since)
            .unwrap()
            .iter()
            .map(|e| browse::recency_group(e.info.published.unwrap().date_naive(), today))
            .collect();
        // 27 October is two days before the fixed clock; 20 October nine.
        assert!(matches!(groups[0], Some(RecencyGroup::Weekday(_))));
        assert_eq!(groups[1], Some(RecencyGroup::LastWeek));
        assert_eq!(groups.len(), 2, "the January episode is too old for New");
    });
}

#[tokio::test]
async fn show_prefs_survive_a_restart() {
    let server = MockServer::start_async().await;
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("beacon.db");
    let (library, show) = subscribed(&server, "plain.xml", Store::open(&db).unwrap()).await;
    let prefs = ShowPrefs {
        filter: EpisodeFilter::Unplayed,
        order: SortOrder::NewestFirst,
    };
    library.with_store(|store| store.set_show_prefs(show, prefs).unwrap());
    drop(library);
    assert_eq!(Store::open(&db).unwrap().show_prefs(show).unwrap(), prefs);
}
