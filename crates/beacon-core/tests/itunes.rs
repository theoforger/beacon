// SPDX-License-Identifier: GPL-3.0-or-later

//! M7 scenarios against canned iTunes Search API answers: search Apple's
//! directory, find a show from an Apple Podcasts link, preview a result's
//! feed and subscribe to it, then find it in the library. Also the cache,
//! the rate limit and how Apple's statuses map to errors.

use std::path::Path;
use std::sync::Arc;

use beacon_core::ItunesError;
use beacon_core::services::clock::Clock;
use beacon_core::services::covers::CoverCache;
use beacon_core::services::downloads::Downloader;
use beacon_core::services::feeds::FeedClient;
use beacon_core::services::itunes::{CALLS_PER_MINUTE, ItunesClient, apple_podcasts_id};
use beacon_core::services::{download_client, http_client, library::Library};
use beacon_core::store::Store;
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

fn client(server: &MockServer) -> ItunesClient {
    ItunesClient::with_endpoints(
        http_client(),
        Url::parse(&server.url("/search")).unwrap(),
        Url::parse(&server.url("/lookup")).unwrap(),
    )
}

fn library(cache: &Path) -> Library {
    let http = http_client();
    Library::new(
        Store::open_in_memory().unwrap(),
        FeedClient::new(http.clone()),
        CoverCache::new(cache.join("covers"), http),
        Downloader::new(cache.join("downloads"), download_client()),
        Arc::new(FixedClock(
            Utc.with_ymd_and_hms(2026, 11, 10, 12, 0, 0).unwrap(),
        )),
    )
}

#[tokio::test]
async fn find_a_show_in_apples_directory_and_subscribe() {
    let server = MockServer::start_async().await;
    let search = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/search")
                .query_param("term", "riddle")
                .query_param("country", "GB")
                .query_param("media", "podcast")
                .query_param("entity", "podcast");
            then.status(200)
                .header("content-type", "text/javascript; charset=utf-8")
                .body(fixture("itunes/search-mixed.json", &server));
        })
        .await;
    let feed = server
        .mock_async(|when, then| {
            when.method(GET).path("/hey-riddle-riddle.xml");
            then.status(200)
                .body(fixture("hey-riddle-riddle.xml", &server));
        })
        .await;
    let itunes = client(&server);

    let shows = itunes.search(" riddle ", "GB").await.unwrap();
    let titles: Vec<_> = shows.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["Hey Riddle Riddle", "Hello From The Magic Tavern"]);
    // The same words again come from the cache.
    assert_eq!(itunes.search("Riddle", "GB").await.unwrap(), shows);
    search.assert_calls_async(1).await;

    // The preview fetches the feed and stores nothing.
    let cache = tempfile::tempdir().unwrap();
    let library = library(cache.path());
    let preview = library.preview(&shows[0].feed_url).await.unwrap();
    assert_eq!(preview.feed.show.title, "Hey Riddle Riddle");
    assert!(!preview.feed.episodes.is_empty());
    assert_eq!(preview.subscribed, None);
    assert!(library.with_store(|s| s.shows().unwrap().is_empty()));

    // Subscribing uses the feed already fetched.
    let id = library.subscribe_preview(&preview).await.unwrap();
    feed.assert_calls_async(1).await;
    let found = library.with_store(|s| s.search_shows("riddle").unwrap());
    assert_eq!(found, [id]);
    let again = library.preview(&shows[0].feed_url).await.unwrap();
    assert_eq!(again.subscribed, Some(id));
    assert!(matches!(
        library.subscribe_preview(&again).await,
        Err(beacon_core::LibraryError::AlreadySubscribed(same)) if same == id
    ));
}

#[tokio::test]
async fn an_apple_podcasts_link_is_looked_up_by_its_id() {
    let server = MockServer::start_async().await;
    let lookup = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/lookup")
                .query_param_exists("id")
                .query_param("country", "US");
            then.status(200)
                .body(fixture("itunes/search-mixed.json", &server));
        })
        .await;
    let itunes = client(&server);
    let link =
        Url::parse("https://podcasts.apple.com/us/podcast/hello-from-the-magic-tavern/id958230465")
            .unwrap();
    let id = apple_podcasts_id(&link).unwrap();
    let show = itunes.lookup(id, "US").await.unwrap();
    assert_eq!(show.title, "Hello From The Magic Tavern");
    assert_eq!(show.feed_url.path(), "/magic-tavern.xml");
    lookup.assert_calls_async(1).await;

    // A show Apple lists without a feed cannot be subscribed to.
    assert!(matches!(
        itunes.lookup(2000000001, "US").await,
        Err(ItunesError::NoFeed)
    ));
}

#[tokio::test]
async fn no_results_is_an_empty_list() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/search");
            then.status(200)
                .body(fixture("itunes/search-empty.json", &server));
        })
        .await;
    let itunes = client(&server);
    assert!(itunes.search("zzzz", "US").await.unwrap().is_empty());
    // Nothing to search for asks Apple nothing.
    assert!(itunes.search("   ", "US").await.unwrap().is_empty());
}

#[tokio::test]
async fn searches_past_the_limit_are_refused_before_calling_apple() {
    let server = MockServer::start_async().await;
    let search = server
        .mock_async(|when, then| {
            when.method(GET).path("/search");
            then.status(200)
                .body(fixture("itunes/search-empty.json", &server));
        })
        .await;
    let itunes = client(&server);
    for i in 0..CALLS_PER_MINUTE {
        itunes.search(&format!("show {i}"), "US").await.unwrap();
    }
    let refused = itunes.search("one more", "US").await;
    assert!(matches!(refused, Err(ItunesError::RateLimited(wait)) if wait.as_secs() <= 60));
    search.assert_calls_async(CALLS_PER_MINUTE).await;
    // A search already made still comes from the cache.
    assert!(itunes.search("show 0", "US").await.is_ok());
}

#[tokio::test]
async fn apples_statuses_have_their_own_errors() {
    let server = MockServer::start_async().await;
    for (term, status) in [
        ("busy", 403),
        ("slow", 429),
        ("country", 400),
        ("down", 503),
    ] {
        server
            .mock_async(|when, then| {
                when.method(GET).path("/search").query_param("term", term);
                then.status(status);
            })
            .await;
    }
    server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/search")
                .query_param("term", "garbled");
            then.status(200).body("<html>not json</html>");
        })
        .await;
    let itunes = client(&server);
    assert!(matches!(
        itunes.search("busy", "US").await,
        Err(ItunesError::RateLimited(_))
    ));
    assert!(matches!(
        itunes.search("slow", "US").await,
        Err(ItunesError::RateLimited(_))
    ));
    assert!(matches!(
        itunes.search("country", "KP").await,
        Err(ItunesError::NoStore(code)) if code == "KP"
    ));
    assert!(matches!(
        itunes.search("down", "US").await,
        Err(ItunesError::Status(503))
    ));
    assert!(matches!(
        itunes.search("garbled", "US").await,
        Err(ItunesError::Parse(_))
    ));
}
