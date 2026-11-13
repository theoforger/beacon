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
use beacon_core::services::library::{Library, ToImport};
use beacon_core::services::opml::OpmlFeed;
use beacon_core::services::{download_client, http_client};
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

    /// Import `feeds` without previewing them first.
    async fn import(&self, feeds: Vec<OpmlFeed>) -> Vec<Result<ShowId, LibraryError>> {
        let feeds = feeds.into_iter().map(|f| ToImport::Feed(f.url)).collect();
        let mut results = Vec::new();
        self.library
            .import(feeds, |result| results.push(result))
            .await;
        results
    }

    fn ids(&self) -> Vec<ShowId> {
        self.library
            .shows()
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect()
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
        assert!(results.iter().all(Result::is_ok), "{app}: {results:?}");
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
    assert_eq!(
        first.library.export_opml(&exported, &first.ids()).unwrap(),
        3
    );

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
async fn a_preview_shows_each_feed_and_the_import_uses_it() {
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

    // Each feed's own title, by its place in the file; the one that is gone
    // fails without stopping the others.
    let urls = plan.feeds.iter().map(|f| f.url.clone()).collect();
    let mut previews: Vec<_> = (0..plan.feeds.len()).map(|_| None).collect();
    setup
        .library
        .preview_feeds(urls, |i, result| previews[i] = Some(result))
        .await;
    let titles: Vec<_> = previews
        .iter()
        .map(|p| match p.as_ref().unwrap() {
            Ok(preview) => preview.feed.show.title.clone(),
            Err(err) => format!("{err:?}"),
        })
        .collect();
    assert_eq!(titles[0], "Small Hours");
    assert_eq!(titles[3], "Paper Trails");
    assert!(matches!(
        previews[2],
        Some(Err(LibraryError::Feed(FeedError::Status(404))))
    ));
    assert!(setup.shows().is_empty(), "a preview stores nothing");

    // Import the previews, and the failed one by its address: only that
    // one fails again.
    let feeds = previews
        .into_iter()
        .zip(&plan.feeds)
        .map(|(preview, feed)| match preview.unwrap() {
            Ok(preview) => ToImport::Previewed(Box::new(preview)),
            Err(_) => ToImport::Feed(feed.url.clone()),
        })
        .collect();
    let mut results = Vec::new();
    setup.library.import(feeds, |r| results.push(r)).await;
    let failed: Vec<_> = results.iter().filter_map(|r| r.as_ref().err()).collect();
    assert_eq!(failed.len(), 1);
    assert!(matches!(
        failed[0],
        LibraryError::Feed(FeedError::Status(404))
    ));
    let titles: Vec<_> = setup.shows().into_iter().map(|(_, t)| t).collect();
    assert_eq!(titles, SHOWS);
}

#[tokio::test]
async fn an_export_holds_only_the_shows_chosen() {
    let server = MockServer::start_async().await;
    serve_feeds(&server).await;
    let setup = library();
    let path = setup.file("in.opml", &fixture("opml/gpodder.opml", &server));
    let plan = setup.library.read_opml(&path).unwrap();
    setup.import(plan.feeds).await;
    let shows = setup.library.shows().unwrap();
    let chosen: Vec<_> = shows
        .iter()
        .filter(|s| s.info.title != "Small Hours")
        .map(|s| s.id)
        .collect();
    let exported = setup.dir.path().join("some.opml");
    assert_eq!(setup.library.export_opml(&exported, &chosen).unwrap(), 2);
    let mut names: Vec<_> =
        beacon_core::services::opml::parse(&std::fs::read_to_string(&exported).unwrap())
            .unwrap()
            .into_iter()
            .map(|f| f.title.unwrap())
            .collect();
    names.sort();
    assert_eq!(names, ["Paper Trails", "The Long Signal"]);
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
