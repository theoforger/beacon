// SPDX-License-Identifier: GPL-3.0-or-later

//! M1 scenarios: subscribe from an RSS URL, refresh, and the ways that fails.
//! Feeds come from `tests/fixtures/` through a local HTTP server.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use beacon_core::LibraryError;
use beacon_core::error::FeedError;
use beacon_core::services::clock::Clock;
use beacon_core::services::covers::CoverCache;
use beacon_core::services::feeds::FeedClient;
use beacon_core::services::{http_client, library::Library};
use beacon_core::store::{Store, SyncStats};
use chrono::{DateTime, TimeZone, Utc};
use httpmock::prelude::*;
use url::Url;

struct FixedClock(DateTime<Utc>);

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

fn fixture(name: &str, server: &MockServer) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name);
    std::fs::read_to_string(path)
        .unwrap()
        .replace("{{BASE}}", &server.base_url())
}

async fn serve<'a>(server: &'a MockServer, path: &str, body: String) -> httpmock::Mock<'a> {
    server
        .mock_async(|when, then| {
            when.method(GET).path(path);
            then.status(200)
                .header("content-type", "application/rss+xml")
                .body(body);
        })
        .await
}

struct Setup {
    library: Library,
    _cache: tempfile::TempDir,
}

fn library_with(store: Store) -> Setup {
    let cache = tempfile::tempdir().unwrap();
    let http = http_client();
    let clock = FixedClock(Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0).unwrap());
    let library = Library::new(
        store,
        FeedClient::new(http.clone()),
        CoverCache::new(cache.path().to_owned(), http),
        Arc::new(clock),
    );
    Setup {
        library,
        _cache: cache,
    }
}

fn library() -> Setup {
    library_with(Store::open_in_memory().unwrap())
}

fn feed_url(server: &MockServer, path: &str) -> Url {
    Url::parse(&server.url(path)).unwrap()
}

#[tokio::test]
async fn subscribe_stores_show_episodes_and_cover() {
    let server = MockServer::start_async().await;
    serve(&server, "/feed.xml", fixture("seasons.xml", &server)).await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/covers/long-signal.png");
            then.status(200).body(b"\x89PNG fake".as_slice());
        })
        .await;
    let setup = library();

    let id = setup
        .library
        .subscribe(&feed_url(&server, "/feed.xml"))
        .await
        .unwrap();

    setup.library.with_store(|store| {
        let show = store.show(id).unwrap().unwrap();
        assert_eq!(show.info.title, "The Long Signal");
        assert_eq!(show.info.author.as_deref(), Some("Dr. Imani Okafor"));
        assert_eq!(show.info.category.as_deref(), Some("Science"));
        let cover: PathBuf = show.cover_path.expect("cover cached");
        assert_eq!(std::fs::read(cover).unwrap(), b"\x89PNG fake");

        let episodes = store.episodes(id).unwrap();
        let titles: Vec<_> = episodes.iter().map(|e| e.info.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "A dish in the desert: listening to the quietest place on Earth",
                "Salt flats",
                "The first echo"
            ]
        );
        let newest = &episodes[0];
        assert_eq!(newest.info.duration, Some(Duration::from_secs(52 * 60)));
        let season = newest.info.season.as_ref().unwrap();
        assert_eq!(
            (season.number, season.name.as_deref()),
            (3, Some("Deserts"))
        );
        assert_eq!(newest.info.number, Some(2.0));
        assert_eq!(episodes[2].info.season.as_ref().unwrap().name, None);

        let unseen: Vec<_> = episodes.iter().filter(|e| !e.state.seen).collect();
        assert_eq!(unseen.len(), 1, "only the newest episode starts unseen");
        assert_eq!(unseen[0].id, newest.id);
    });
}

#[tokio::test]
async fn show_without_seasons_has_none() {
    let server = MockServer::start_async().await;
    serve(&server, "/feed.xml", fixture("plain.xml", &server)).await;
    let setup = library();
    let id = setup
        .library
        .subscribe(&feed_url(&server, "/feed.xml"))
        .await
        .unwrap();
    setup.library.with_store(|store| {
        let episodes = store.episodes(id).unwrap();
        assert_eq!(episodes.len(), 2);
        assert!(episodes.iter().all(|e| e.info.season.is_none()));
    });
}

#[tokio::test]
async fn refresh_adds_only_new_episodes() {
    let server = MockServer::start_async().await;
    let old = serve(&server, "/feed.xml", fixture("plain.xml", &server)).await;
    let setup = library();
    let id = setup
        .library
        .subscribe(&feed_url(&server, "/feed.xml"))
        .await
        .unwrap();

    // The same feed again changes nothing.
    assert_eq!(
        setup.library.refresh(id).await.unwrap(),
        SyncStats::default()
    );

    old.delete_async().await;
    serve(&server, "/feed.xml", fixture("plain-updated.xml", &server)).await;
    let stats = setup.library.refresh(id).await.unwrap();
    assert_eq!(
        stats,
        SyncStats {
            added: 1,
            updated: 1
        }
    );

    setup.library.with_store(|store| {
        let episodes = store.episodes(id).unwrap();
        let summary: Vec<_> = episodes
            .iter()
            .map(|e| (e.info.title.as_str(), e.state.seen))
            .collect();
        assert_eq!(
            summary,
            [
                ("Dawn chorus", false),
                ("The night shift (remastered)", false),
                ("Insomnia, briefly", true),
            ]
        );
    });
}

#[tokio::test]
async fn chapters_are_stored() {
    let server = MockServer::start_async().await;
    serve(&server, "/feed.xml", fixture("chapters.xml", &server)).await;
    let setup = library();
    let id = setup
        .library
        .subscribe(&feed_url(&server, "/feed.xml"))
        .await
        .unwrap();
    setup.library.with_store(|store| {
        let episodes = store.episodes(id).unwrap();
        let chapters = store.chapters(episodes[0].id).unwrap();
        let summary: Vec<_> = chapters
            .iter()
            .map(|c| (c.start.as_millis(), c.title.as_str()))
            .collect();
        assert_eq!(
            summary,
            [
                (0, "Cold open"),
                (192_000, "Why quiet is hard to protect"),
                (1_300_500, "The ledger")
            ]
        );
        assert_eq!(
            episodes[1].info.chapters_url.as_ref().unwrap().as_str(),
            server.url("/chapters/pt1.json")
        );
        assert!(store.chapters(episodes[1].id).unwrap().is_empty());
    });
}

#[tokio::test]
async fn items_without_guid_keep_their_identity() {
    let server = MockServer::start_async().await;
    serve(&server, "/feed.xml", fixture("no-guid.xml", &server)).await;
    let setup = library();
    let id = setup
        .library
        .subscribe(&feed_url(&server, "/feed.xml"))
        .await
        .unwrap();
    assert_eq!(
        setup.library.refresh(id).await.unwrap(),
        SyncStats::default()
    );
    setup.library.with_store(|store| {
        let guids: Vec<_> = store
            .episodes(id)
            .unwrap()
            .into_iter()
            .map(|e| e.info.guid)
            .collect();
        assert_eq!(
            guids,
            [
                server.url("/audio/sr2.mp3"),
                "https://example.com/slow-roads/trailer".into()
            ]
        );
    });
}

#[tokio::test]
async fn bad_feeds_are_typed_errors_and_store_nothing() {
    let server = MockServer::start_async().await;
    serve(&server, "/malformed.xml", fixture("malformed.xml", &server)).await;
    serve(&server, "/page.html", fixture("not-a-feed.html", &server)).await;
    let setup = library();

    for path in ["/malformed.xml", "/page.html"] {
        let result = setup.library.subscribe(&feed_url(&server, path)).await;
        assert!(
            matches!(result, Err(LibraryError::Feed(FeedError::Parse(_)))),
            "{path}: {result:?}"
        );
    }
    let missing = setup
        .library
        .subscribe(&feed_url(&server, "/missing.xml"))
        .await;
    assert!(matches!(
        missing,
        Err(LibraryError::Feed(FeedError::Status(404)))
    ));

    setup
        .library
        .with_store(|store| assert!(store.shows().unwrap().is_empty()));
}

#[tokio::test]
async fn subscribing_twice_is_refused() {
    let server = MockServer::start_async().await;
    serve(&server, "/feed.xml", fixture("plain.xml", &server)).await;
    let setup = library();
    let url = feed_url(&server, "/feed.xml");
    let id = setup.library.subscribe(&url).await.unwrap();
    assert!(matches!(
        setup.library.subscribe(&url).await,
        Err(LibraryError::AlreadySubscribed(existing)) if existing == id
    ));
}

#[tokio::test]
async fn missing_cover_does_not_fail_the_subscription() {
    let server = MockServer::start_async().await;
    serve(&server, "/feed.xml", fixture("plain.xml", &server)).await;
    let setup = library();
    let id = setup
        .library
        .subscribe(&feed_url(&server, "/feed.xml"))
        .await
        .unwrap();
    setup.library.with_store(|store| {
        assert_eq!(store.show(id).unwrap().unwrap().cover_path, None);
    });
}

#[tokio::test]
async fn library_on_disk_survives_a_restart() {
    let server = MockServer::start_async().await;
    serve(&server, "/feed.xml", fixture("plain.xml", &server)).await;
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("beacon.db");

    let setup = library_with(Store::open(&db).unwrap());
    setup
        .library
        .subscribe(&feed_url(&server, "/feed.xml"))
        .await
        .unwrap();
    drop(setup);

    // Reopening runs the migrations against an existing database.
    let store = Store::open(&db).unwrap();
    let shows = store.shows().unwrap();
    assert_eq!(shows.len(), 1);
    assert_eq!(store.episodes(shows[0].id).unwrap().len(), 2);
}
