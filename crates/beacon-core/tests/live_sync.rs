// SPDX-License-Identifier: GPL-3.0-or-later

//! The M6 scenario against a real nextcloud-gpodder server, with two
//! libraries standing in for two devices. Not run in CI, which never calls
//! live servers:
//!
//! ```sh
//! BEACON_SYNC_SERVER=http://127.0.0.1:8080 BEACON_SYNC_USER=alice \
//! BEACON_SYNC_PASSWORD=… BEACON_SYNC_FEED=http://127.0.0.1:8000/feed.xml \
//! cargo test -p beacon-core --test live_sync -- --ignored
//! ```
//!
//! The feed must have at least two episodes with audio and durations; the
//! account should start with no subscriptions. A server without
//! Nextcloud's app-password endpoint (nc-gpodder-lite) is logged in to with
//! Login Flow v2 instead, approved by posting its grant form as the
//! browser would.

use std::sync::Arc;
use std::time::Duration;

use beacon_core::services::clock::SystemClock;
use beacon_core::services::covers::CoverCache;
use beacon_core::services::downloads::Downloader;
use beacon_core::services::feeds::FeedClient;
use beacon_core::services::{download_client, http_client, library::Library};
use beacon_core::store::{PendingMark, Store};
use beacon_core::sync::nextcloud::{self, Credentials, NextcloudBackend};
use beacon_core::sync::service::SyncService;
use url::Url;

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name}"))
}

struct Device {
    service: SyncService,
    library: Arc<Library>,
    backend: NextcloudBackend,
    _dir: tempfile::TempDir,
}

fn device(credentials: &Credentials) -> Device {
    let dir = tempfile::tempdir().unwrap();
    let http = http_client();
    let library = Arc::new(Library::new(
        Store::open_in_memory().unwrap(),
        FeedClient::new(http.clone()),
        CoverCache::new(dir.path().join("covers"), http.clone()),
        Downloader::new(dir.path().join("downloads"), download_client()),
        Arc::new(SystemClock),
    ));
    library.with_store(|store| {
        store
            .set_sync_account(&credentials.server, &credentials.user)
            .unwrap();
    });
    Device {
        service: SyncService::new(library.clone()),
        library,
        backend: NextcloudBackend::new(http, credentials.clone()),
        _dir: dir,
    }
}

impl Device {
    async fn sync(&self) -> beacon_core::sync::service::SyncSummary {
        let summary = self.service.sync(&self.backend).await.unwrap();
        assert_eq!(
            self.library.with_store(|s| s.pending_mark().unwrap()),
            PendingMark::default(),
            "nothing left pending"
        );
        summary
    }
}

/// Log in through the browser flow, approving it as nc-gpodder-lite's
/// grant page would.
async fn browser_login(
    http: &reqwest::Client,
    server: &Url,
    user: &str,
    password: &str,
) -> Credentials {
    let login = nextcloud::start_browser_login(http, server).await.unwrap();
    let token = login.login.path_segments().unwrap().next_back().unwrap();
    let form = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("flow_token", token)
        .append_pair("username", user)
        .append_pair("password", password)
        .finish();
    let granted = http
        .post(login.login.join("../grant").unwrap())
        .header("content-type", "application/x-www-form-urlencoded")
        .body(form)
        .send()
        .await
        .unwrap();
    assert!(granted.status().is_success(), "{}", granted.status());
    nextcloud::wait_for_browser_login(
        http,
        &login,
        Duration::from_millis(200),
        &std::sync::atomic::AtomicBool::new(false),
    )
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "needs BEACON_SYNC_SERVER and a nextcloud-gpodder account"]
async fn two_devices_sync_through_a_real_server() {
    let http = http_client();
    let server = nextcloud::server_url(&var("BEACON_SYNC_SERVER")).unwrap();
    let (user, password) = (var("BEACON_SYNC_USER"), var("BEACON_SYNC_PASSWORD"));
    let credentials = match nextcloud::log_in_with_password(&http, &server, &user, &password).await
    {
        Ok(credentials) => credentials,
        Err(beacon_core::SyncError::Status(404)) => {
            browser_login(&http, &server, &user, &password).await
        }
        Err(err) => panic!("{err:?}"),
    };
    let feed = Url::parse(&var("BEACON_SYNC_FEED")).unwrap();

    // Device A subscribes and listens part of the way, then syncs.
    let a = device(&credentials);
    let show = a.library.subscribe(&feed).await.unwrap();
    let episodes = a.library.with_store(|s| s.episodes(show).unwrap());
    let (first, second) = (&episodes[0], &episodes[1]);
    a.library.with_store(|s| {
        s.set_position(first.id, Duration::from_secs(95)).unwrap();
    });
    let summary = a.sync().await;
    assert!(summary.sent_subscriptions >= 1 && summary.sent_actions >= 1);

    // Device B, fresh, picks up the show and the position.
    let b = device(&credentials);
    let summary = b.sync().await;
    assert_eq!(summary.subscribed, 1);
    let b_show = b
        .library
        .with_store(|s| s.show_id_for_feed(&feed).unwrap())
        .expect("subscribed from the server");
    let b_episodes = b.library.with_store(|s| s.episodes(b_show).unwrap());
    let find = |guid: &str| b_episodes.iter().find(|e| e.info.guid == guid).unwrap();
    assert_eq!(
        find(&first.info.guid).state.position,
        Duration::from_secs(95)
    );

    // Device B finishes the second episode; device A sees it played.
    let b_second = find(&second.info.guid).id;
    b.library.with_store(|s| {
        s.set_position(b_second, Duration::from_secs(30)).unwrap();
        s.finish(b_second, chrono::Utc::now()).unwrap();
    });
    b.sync().await;
    // The server's `since` is in seconds; let the clock move past it.
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let summary = a.sync().await;
    assert!(summary.episodes >= 1, "{summary:?}");
    let state = a
        .library
        .with_store(|s| s.episode(second.id).unwrap().unwrap().state);
    assert!(state.played);

    // Device B unsubscribes; device A follows.
    b.library.with_store(|s| s.delete_show(b_show).unwrap());
    b.sync().await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let summary = a.sync().await;
    assert_eq!(summary.unsubscribed, 1, "{summary:?}");
    assert!(
        a.library
            .with_store(|s| s.show_id_for_feed(&feed).unwrap())
            .is_none()
    );

    // nc-gpodder-lite has no app-password endpoint; Log Out goes on anyway.
    match nextcloud::revoke(&http, &credentials).await {
        Ok(()) | Err(beacon_core::SyncError::Status(404)) => {}
        Err(err) => panic!("{err:?}"),
    }
}
