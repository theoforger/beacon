// SPDX-License-Identifier: GPL-3.0-or-later

//! M5 scenarios: download an episode, resume one that stopped part way,
//! cancel, and delete old downloads while keeping queued ones. Audio comes
//! from a local HTTP server.

use std::path::Path;
use std::sync::{Arc, Mutex};

use beacon_core::DownloadError;
use beacon_core::model::EpisodeId;
use beacon_core::services::cleanup::CleanupPolicy;
use beacon_core::services::clock::Clock;
use beacon_core::services::covers::CoverCache;
use beacon_core::services::downloads::{Downloader, Progress};
use beacon_core::services::feeds::FeedClient;
use beacon_core::services::{download_client, http_client, library::Library};
use beacon_core::store::Store;
use chrono::{DateTime, Duration, TimeZone, Utc};
use httpmock::prelude::*;
use url::Url;

/// A clock the test moves on.
struct TestClock(Mutex<DateTime<Utc>>);

impl Clock for TestClock {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}

impl TestClock {
    fn advance(&self, by: Duration) {
        *self.0.lock().unwrap() += by;
    }
}

struct Setup {
    library: Library,
    clock: Arc<TestClock>,
    dir: tempfile::TempDir,
    /// The plain fixture's episodes, newest first.
    episodes: Vec<EpisodeId>,
}

impl Setup {
    fn downloads(&self) -> std::path::PathBuf {
        self.dir.path().join("downloads")
    }
}

async fn subscribed(server: &MockServer) -> Setup {
    let feed = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/plain.xml"),
    )
    .unwrap()
    .replace("{{BASE}}", &server.base_url());
    server
        .mock_async(|when, then| {
            when.method(GET).path("/feed.xml");
            then.status(200).body(feed);
        })
        .await;
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(TestClock(Mutex::new(
        Utc.with_ymd_and_hms(2026, 11, 10, 12, 0, 0).unwrap(),
    )));
    let http = http_client();
    let library = Library::new(
        Store::open_in_memory().unwrap(),
        FeedClient::new(http.clone()),
        CoverCache::new(dir.path().join("covers"), http),
        Downloader::new(dir.path().join("downloads"), download_client()),
        clock.clone(),
    );
    let show = library
        .subscribe(&Url::parse(&server.url("/feed.xml")).unwrap())
        .await
        .unwrap();
    let episodes =
        library.with_store(|store| store.episodes(show).unwrap().iter().map(|e| e.id).collect());
    Setup {
        library,
        clock,
        dir,
        episodes,
    }
}

/// 1000 bytes that differ from place to place, so a wrong join shows.
fn audio() -> Vec<u8> {
    (0..1000u32).map(|i| (i * 7 % 251) as u8).collect()
}

#[tokio::test]
async fn an_episode_downloads_and_is_recorded() {
    let server = MockServer::start_async().await;
    let setup = subscribed(&server).await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/audio/sh2.mp3");
            then.status(200)
                .header("content-type", "audio/mpeg")
                .body(audio());
        })
        .await;
    let id = setup.episodes[0];
    let progress = Progress::default();
    let path = setup.library.download(id, &progress).await.unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), audio());
    assert_eq!(path, setup.downloads().join(format!("{}.mp3", id.0)));
    assert_eq!(progress.fraction(), Some(1.0));
    let state = setup
        .library
        .with_store(|store| store.episode(id).unwrap().unwrap().state);
    assert_eq!(state.download_path.as_deref(), Some(path.as_path()));
    assert_eq!(state.downloaded_at, Some(setup.clock.now()));
    assert!(!setup.downloads().join(format!("{}.part", id.0)).exists());
}

#[tokio::test]
async fn a_download_that_stopped_part_way_resumes() {
    let server = MockServer::start_async().await;
    let setup = subscribed(&server).await;
    let id = setup.episodes[0];
    // What an interrupted download left.
    std::fs::create_dir_all(setup.downloads()).unwrap();
    std::fs::write(
        setup.downloads().join(format!("{}.part", id.0)),
        &audio()[..400],
    )
    .unwrap();
    let rest = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/audio/sh2.mp3")
                .header("range", "bytes=400-");
            then.status(206)
                .header("content-type", "audio/mpeg")
                .header("content-range", "bytes 400-999/1000")
                .body(&audio()[400..]);
        })
        .await;
    let progress = Progress::default();
    let path = setup.library.download(id, &progress).await.unwrap();
    rest.assert_async().await;
    assert_eq!(std::fs::read(path).unwrap(), audio());
}

#[tokio::test]
async fn a_server_that_ignores_the_range_starts_it_over() {
    let server = MockServer::start_async().await;
    let setup = subscribed(&server).await;
    let id = setup.episodes[0];
    std::fs::create_dir_all(setup.downloads()).unwrap();
    std::fs::write(setup.downloads().join(format!("{}.part", id.0)), b"stale").unwrap();
    server
        .mock_async(|when, then| {
            when.method(GET).path("/audio/sh2.mp3");
            then.status(200).body(audio());
        })
        .await;
    let path = setup
        .library
        .download(id, &Progress::default())
        .await
        .unwrap();
    assert_eq!(std::fs::read(path).unwrap(), audio());
}

#[tokio::test]
async fn failures_and_cancelling_are_typed_and_record_nothing() {
    let server = MockServer::start_async().await;
    let setup = subscribed(&server).await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/audio/sh2.mp3");
            then.status(404);
        })
        .await;
    let id = setup.episodes[0];
    let result = setup.library.download(id, &Progress::default()).await;
    assert!(
        matches!(result, Err(DownloadError::Status(404))),
        "{result:?}"
    );

    let cancelled = Progress::default();
    cancelled.cancel();
    let result = setup.library.download(setup.episodes[1], &cancelled).await;
    assert!(
        matches!(result, Err(DownloadError::Cancelled)),
        "{result:?}"
    );
    setup.library.with_store(|store| {
        assert!(store.downloaded_episodes().unwrap().is_empty());
    });
}

#[tokio::test]
async fn old_downloads_are_deleted_but_queued_ones_kept() {
    let server = MockServer::start_async().await;
    let setup = subscribed(&server).await;
    let [newer, older] = setup.episodes[..] else {
        panic!("the plain fixture has two episodes");
    };
    std::fs::create_dir_all(setup.downloads()).unwrap();
    let now = setup.clock.now();
    let files = setup.library.with_store(|store| {
        [newer, older].map(|id| {
            let path = setup.downloads().join(format!("{}.mp3", id.0));
            std::fs::write(&path, audio()).unwrap();
            store.set_download(id, Some(&path), now).unwrap();
            store.set_played(id, true, now).unwrap();
            path
        })
    });
    setup
        .library
        .with_store(|store| store.enqueue(older).unwrap());
    let policy = CleanupPolicy {
        enabled: true,
        after_days: 7,
        only_played: true,
    };

    // Six days on, nothing is old enough.
    setup.clock.advance(Duration::days(6));
    assert_eq!(setup.library.clean_downloads(policy, &[]).unwrap(), 0);
    // A week on, the played download goes; the queued one stays, and so
    // does one kept as playing.
    setup.clock.advance(Duration::days(1));
    assert_eq!(setup.library.clean_downloads(policy, &[newer]).unwrap(), 0);
    assert_eq!(setup.library.clean_downloads(policy, &[]).unwrap(), 1);
    assert!(!files[0].exists());
    assert!(files[1].exists(), "queued");
    let downloaded: Vec<_> = setup.library.with_store(|store| {
        store
            .downloaded_episodes()
            .unwrap()
            .iter()
            .map(|e| e.id)
            .collect()
    });
    assert_eq!(downloaded, [older]);
    // Turned off, nothing goes.
    setup
        .library
        .with_store(|store| store.dequeue(older).unwrap());
    let off = CleanupPolicy {
        enabled: false,
        ..policy
    };
    assert_eq!(setup.library.clean_downloads(off, &[]).unwrap(), 0);
}
