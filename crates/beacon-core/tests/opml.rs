// SPDX-License-Identifier: GPL-3.0-or-later

//! M8 scenarios: import the OPML files other podcast apps export, export
//! the library, and import that again with nothing lost. Feeds come from
//! `tests/fixtures/` through a local HTTP server.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use beacon_core::error::FeedError;
use beacon_core::model::ShowId;
use beacon_core::services::clock::SystemClock;
use beacon_core::services::covers::CoverCache;
use beacon_core::services::downloads::Downloader;
use beacon_core::services::feeds::FeedClient;
use beacon_core::services::opml::OpmlFeed;
use beacon_core::services::{download_client, http_client, library::Library};
use beacon_core::store::Store;
use beacon_core::{LibraryError, OpmlError};
use httpmock::prelude::*;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn fixture(name: &str, server: &MockServer) -> String {
    std::fs::read_to_string(fixtures().join(name))
        .unwrap()
        .replace("{{BASE}}", &server.base_url())
}

/// The three shows every fixture file lists, at the addresses it gives.
async fn serve_feeds(server: &MockServer) {
    for (path, feed) in [
        ("/small-hours.xml", "plain.xml"),
        ("/long-signal.xml", "seasons.xml"),
        ("/paper-trails.xml", "chapters.xml"),
    ] {
        let body = fixture(feed, server);
        server
            .mock_async(|when, then| {
                when.method(GET).path(path);
                then.status(200)
                    .header("content-type", "application/rss+xml")
                    .body(body);
            })
            .await;
    }
    server
        .mock_async(|when, then| {
            when.method(GET).path("/gone-quiet.xml");
            then.status(404);
        })
        .await;
}

struct Setup {
    library: Library,
    dir: tempfile::TempDir,
}

fn library() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let http = http_client();
    let library = Library::new(
        Store::open_in_memory().unwrap(),
        FeedClient::new(http.clone()),
        CoverCache::new(dir.path().join("covers"), http),
        Downloader::new(dir.path().join("downloads"), download_client()),
        Arc::new(SystemClock),
    );
    Setup { library, dir }
}

impl Setup {
    /// Write `text` to a file in the scratch folder, as a file chosen to
    /// import.
    fn file(&self, name: &str, text: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    async fn import(&self, feeds: Vec<OpmlFeed>) -> Vec<(OpmlFeed, Result<ShowId, LibraryError>)> {
        let mut results = Vec::new();
        self.library
            .import(feeds, |feed, result| results.push((feed, result)))
            .await;
        results
    }

    /// Every show's feed address and title, in title order.
    fn shows(&self) -> Vec<(String, String)> {
        self.library.with_store(|store| {
            store
                .shows()
                .unwrap()
                .into_iter()
                .map(|s| (s.feed_url.to_string(), s.info.title))
                .collect()
        })
    }
}

const SHOWS: [&str; 3] = ["Paper Trails", "Small Hours", "The Long Signal"];

#[tokio::test]
async fn files_from_other_podcast_apps_import() {
    let server = MockServer::start_async().await;
    serve_feeds(&server).await;
    for app in [
        "gnome-podcasts.opml",
        "antennapod.opml",
        "gpodder.opml",
        "pocket-casts.opml",
    ] {
        let setup = library();
        let path = setup.file(app, &fixture(&format!("opml/{app}"), &server));
        let plan = setup.library.read_opml(&path).unwrap();
        let mut names: Vec<_> = plan.feeds.iter().map(|f| f.name().to_owned()).collect();
        names.sort();
        assert_eq!(names, SHOWS, "{app}");
        assert_eq!((plan.subscribed, plan.repeated), (0, 0), "{app}");

        let results = setup.import(plan.feeds).await;
        assert!(results.iter().all(|(_, r)| r.is_ok()), "{app}: {results:?}");
        let titles: Vec<_> = setup.shows().into_iter().map(|(_, t)| t).collect();
        assert_eq!(titles, SHOWS, "{app}");
    }
}

#[tokio::test]
async fn an_export_imports_again_with_nothing_lost() {
    let server = MockServer::start_async().await;
    serve_feeds(&server).await;
    let first = library();
    let path = first.file("in.opml", &fixture("opml/antennapod.opml", &server));
    let plan = first.library.read_opml(&path).unwrap();
    first.import(plan.feeds).await;
    let before = first.shows();
    assert_eq!(before.len(), 3);

    let exported = first.dir.path().join("podcasts.opml");
    assert_eq!(first.library.export_opml(&exported).unwrap(), 3);

    // Into an empty library: the same feeds, under the same names.
    let second = library();
    let plan = second.library.read_opml(&exported).unwrap();
    let mut listed: Vec<_> = plan
        .feeds
        .iter()
        .map(|f| (f.url.to_string(), f.title.clone().unwrap()))
        .collect();
    listed.sort_by(|a, b| a.1.cmp(&b.1));
    assert_eq!(listed, before);
    second.import(plan.feeds).await;
    assert_eq!(second.shows(), before);

    // Into the library it came from: nothing new.
    let plan = first.library.read_opml(&exported).unwrap();
    assert!(plan.feeds.is_empty());
    assert_eq!(plan.subscribed, 3);
}

#[tokio::test]
async fn an_import_goes_on_past_a_feed_that_fails() {
    let server = MockServer::start_async().await;
    serve_feeds(&server).await;
    let setup = library();
    let path = setup.file("edge.opml", &fixture("opml/edge-cases.opml", &server));
    let plan = setup.library.read_opml(&path).unwrap();
    let names: Vec<_> = plan.feeds.iter().map(|f| f.name().to_owned()).collect();
    assert_eq!(
        names,
        [
            "Small Hours",
            "The Long Signal",
            "Gone Quiet",
            "Paper Trails & Friends"
        ]
    );
    assert_eq!(plan.repeated, 1);

    let results = setup.import(plan.feeds).await;
    let failed: Vec<_> = results
        .iter()
        .filter_map(|(feed, r)| r.as_ref().err().map(|e| (feed.name(), e)))
        .collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].0, "Gone Quiet");
    assert!(matches!(
        failed[0].1,
        LibraryError::Feed(FeedError::Status(404))
    ));
    let titles: Vec<_> = setup.shows().into_iter().map(|(_, t)| t).collect();
    assert_eq!(titles, SHOWS);
}

#[tokio::test]
async fn a_file_that_is_not_opml_is_an_error() {
    let setup = library();
    let malformed = fixtures().join("opml/malformed.opml");
    assert!(matches!(
        setup.library.read_opml(&malformed),
        Err(OpmlError::Xml(_))
    ));
    let feed = fixtures().join("plain.xml");
    assert!(matches!(
        setup.library.read_opml(&feed),
        Err(OpmlError::NotOpml)
    ));
    let missing = setup.dir.path().join("missing.opml");
    assert!(matches!(
        setup.library.read_opml(&missing),
        Err(OpmlError::Io(_))
    ));
    assert!(setup.shows().is_empty());
}
