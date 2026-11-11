// SPDX-License-Identifier: GPL-3.0-or-later

//! M6 scenarios against a mock nextcloud-gpodder: a fresh library picks up
//! a show and a position from the server; a local play and an unsubscribe
//! are pushed and leave nothing pending. Also the login flows, Log Out and
//! how HTTP statuses map to errors.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use beacon_core::SyncError;
use beacon_core::services::clock::Clock;
use beacon_core::services::covers::CoverCache;
use beacon_core::services::downloads::Downloader;
use beacon_core::services::feeds::FeedClient;
use beacon_core::services::{download_client, http_client, library::Library};
use beacon_core::store::{PendingMark, Store};
use beacon_core::sync::nextcloud::{self, Credentials, NextcloudBackend};
use beacon_core::sync::service::SyncService;
use beacon_core::sync::{SubscriptionChanges, SyncBackend};
use chrono::{DateTime, TimeZone, Utc};
use httpmock::prelude::*;
use url::Url;

const API: &str = "/index.php/apps/gpoddersync";
/// Basic auth for `ada` with the app password `secret`.
const AUTH: &str = "Basic YWRhOnNlY3JldA==";

struct FixedClock(DateTime<Utc>);

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 10, 12, 0, 0).unwrap()
}

fn fixture(name: &str, server: &MockServer) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name);
    std::fs::read_to_string(path)
        .unwrap()
        .replace("{{BASE}}", &server.base_url())
}

struct Setup {
    service: SyncService,
    library: Arc<Library>,
    _cache: tempfile::TempDir,
}

fn setup() -> Setup {
    let cache = tempfile::tempdir().unwrap();
    let http = http_client();
    let library = Arc::new(Library::new(
        Store::open_in_memory().unwrap(),
        FeedClient::new(http.clone()),
        CoverCache::new(cache.path().join("covers"), http),
        Downloader::new(cache.path().join("downloads"), download_client()),
        Arc::new(FixedClock(now())),
    ));
    Setup {
        service: SyncService::new(library.clone()),
        library,
        _cache: cache,
    }
}

fn credentials(server: &MockServer) -> Credentials {
    Credentials {
        server: Url::parse(&server.url("/")).unwrap(),
        user: "ada".to_owned(),
        app_password: "secret".to_owned(),
    }
}

fn backend(server: &MockServer) -> NextcloudBackend {
    NextcloudBackend::new(http_client(), credentials(server))
}

fn sign_in(library: &Library, server: &MockServer) {
    library.with_store(|store| {
        store
            .set_sync_account(&credentials(server).server, "ada")
            .unwrap();
    });
}

fn nothing_pending(library: &Library) -> bool {
    library.with_store(|store| store.pending_mark().unwrap()) == PendingMark::default()
}

#[tokio::test]
async fn a_fresh_library_picks_up_a_show_and_a_position() {
    let server = MockServer::start_async().await;
    let feed = server.url("/feed.xml");
    let feed_mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/feed.xml");
            then.status(200).body(fixture("plain.xml", &server));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET)
                .path(format!("{API}/subscriptions"))
                .query_param_missing("since")
                .header("authorization", AUTH);
            then.status(200).body(format!(
                r#"{{"add": ["{feed}"], "remove": [], "timestamp": 1000}}"#
            ));
        })
        .await;
    let audio = server.url("/audio/sh1.mp3");
    let missing = server.url("/audio/sh9.mp3");
    server
        .mock_async(|when, then| {
            when.method(GET)
                .path(format!("{API}/episode_action"))
                .query_param_missing("since")
                .header("authorization", AUTH);
            then.status(200).body(format!(
                r#"{{"actions": [
                    {{"podcast": "{feed}", "episode": "{audio}", "guid": "small-hours-1",
                      "action": "PLAY", "timestamp": "2026-11-09T20:00:00",
                      "started": 0, "position": 600, "total": 2460}},
                    {{"podcast": "{feed}", "episode": "{missing}", "guid": "small-hours-9",
                      "action": "PLAY", "timestamp": "2026-11-09T21:00:00",
                      "started": 0, "position": 60, "total": 2460}}
                ], "timestamp": 2000}}"#
            ));
        })
        .await;
    let pushed = server
        .mock_async(|when, then| {
            when.method(POST).path_includes("/create");
            then.status(200).body(r#"{"timestamp": 3000}"#);
        })
        .await;
    let Setup {
        service, library, ..
    } = setup();
    sign_in(&library, &server);

    let summary = service.sync(&backend(&server)).await.unwrap();

    assert_eq!(summary.subscribed, 1);
    assert_eq!(summary.episodes, 1);
    assert_eq!(summary.dropped, 1, "an episode the feed does not have");
    assert_eq!(
        feed_mock.calls_async().await,
        2,
        "the show was refreshed once for it"
    );
    assert_eq!(
        pushed.calls_async().await,
        0,
        "nothing goes back to the server"
    );
    library.with_store(|store| {
        let show = store
            .show_id_for_feed(&Url::parse(&feed).unwrap())
            .unwrap()
            .expect("subscribed");
        let episode = store
            .episodes(show)
            .unwrap()
            .into_iter()
            .find(|e| e.info.guid == "small-hours-1")
            .unwrap();
        assert_eq!(episode.state.position, Duration::from_secs(600));
        assert!(!episode.state.played);
        let account = store.sync_account().unwrap().unwrap();
        assert_eq!(
            (account.subscriptions_since, account.actions_since),
            (Some(999), Some(1999)),
            "the server's times less a second of overlap, not the push's"
        );
        assert_eq!(account.last_synced, Some(now()));
    });
    assert!(nothing_pending(&library));
}

#[tokio::test]
async fn a_local_play_and_an_unsubscribe_are_pushed_and_leave_nothing_pending() {
    let server = MockServer::start_async().await;
    let feed = server.url("/feed.xml");
    server
        .mock_async(|when, then| {
            when.method(GET).path("/feed.xml");
            then.status(200).body(fixture("plain.xml", &server));
        })
        .await;
    let Setup {
        service, library, ..
    } = setup();
    let show = library
        .subscribe(&Url::parse(&feed).unwrap())
        .await
        .unwrap();
    // Signed in and synced before: the server has the show.
    library.with_store(|store| {
        store
            .set_sync_account(&credentials(&server).server, "ada")
            .unwrap();
        store
            .finish_sync(store.pending_mark().unwrap(), 1000, 2000, now())
            .unwrap();
    });

    let episode = library.with_store(|store| {
        let episode = store.episodes(show).unwrap().pop().unwrap();
        store
            .set_position(episode.id, Duration::from_secs(300))
            .unwrap();
        store.delete_show(show).unwrap();
        episode
    });

    for path in ["subscriptions", "episode_action"] {
        server
            .mock_async(|when, then| {
                when.method(GET).path(format!("{API}/{path}"));
                then.status(200)
                    .body(r#"{"add": [], "remove": [], "actions": [], "timestamp": 4000}"#);
            })
            .await;
    }
    let subscriptions = server
        .mock_async(|when, then| {
            when.method(POST)
                .path(format!("{API}/subscription_change/create"))
                .header("authorization", AUTH)
                .json_body(serde_json::json!({"add": [], "remove": [feed]}));
            then.status(200).body(r#"{"timestamp": 4001}"#);
        })
        .await;
    let audio = episode.info.enclosure.unwrap().url.to_string();
    let actions = server
        .mock_async(|when, then| {
            when.method(POST)
                .path(format!("{API}/episode_action/create"))
                .header("authorization", AUTH)
                .body_includes(r#""action":"PLAY""#)
                .body_includes(r#""guid":"small-hours-1""#)
                .body_includes(format!(r#""episode":"{audio}""#))
                .body_includes(r#""position":300"#)
                .body_includes(r#""total":2460"#);
            then.status(200).body(r#"{"timestamp": 4002}"#);
        })
        .await;

    let summary = service.sync(&backend(&server)).await.unwrap();

    assert_eq!(subscriptions.calls_async().await, 1);
    assert_eq!(actions.calls_async().await, 1);
    assert_eq!((summary.sent_subscriptions, summary.sent_actions), (1, 1));
    assert!(nothing_pending(&library));
    let account = library.with_store(|store| store.sync_account().unwrap().unwrap());
    assert_eq!(account.actions_since, Some(3999));
}

#[tokio::test]
async fn a_sync_needs_an_account_with_the_switch_on() {
    let server = MockServer::start_async().await;
    let Setup {
        service, library, ..
    } = setup();
    assert!(matches!(
        service.sync(&backend(&server)).await,
        Err(SyncError::NotConfigured)
    ));
    sign_in(&library, &server);
    library.with_store(|store| store.set_sync_active(false).unwrap());
    assert!(matches!(
        service.sync(&backend(&server)).await,
        Err(SyncError::NotConfigured)
    ));
}

#[tokio::test]
async fn http_statuses_map_to_their_errors() {
    for (status, body, check) in [
        (
            401,
            "",
            (|e| matches!(e, SyncError::Unauthorized)) as fn(&SyncError) -> bool,
        ),
        (404, "", |e| matches!(e, SyncError::AppMissing)),
        (500, "", |e| matches!(e, SyncError::Status(500))),
        (200, "<html>", |e| matches!(e, SyncError::Parse(_))),
    ] {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.any_request();
                then.status(status).body(body);
            })
            .await;
        let backend = backend(&server);
        let error = backend.subscriptions(None).await.unwrap_err();
        assert!(check(&error), "{status}: {error:?}");
        let error = backend
            .push_subscriptions(&SubscriptionChanges::default())
            .await
            .unwrap_err();
        assert!(check(&error), "{status}: {error:?}");
    }
}

#[tokio::test]
async fn an_unreachable_server_is_a_network_error() {
    let backend = NextcloudBackend::new(
        http_client(),
        Credentials {
            server: Url::parse("http://127.0.0.1:9/").unwrap(),
            user: "ada".to_owned(),
            app_password: "secret".to_owned(),
        },
    );
    assert!(matches!(
        backend.episode_actions(Some(5)).await,
        Err(SyncError::Network(_))
    ));
}

#[tokio::test]
async fn a_browser_login_returns_the_approved_app_password() {
    let server = MockServer::start_async().await;
    let endpoint = server.url("/login/v2/poll");
    let login_page = server.url("/login/v2/flow/abc");
    server
        .mock_async(|when, then| {
            when.method(POST).path("/nextcloud/index.php/login/v2");
            then.status(200).body(format!(
                r#"{{"poll": {{"token": "t0ken", "endpoint": "{endpoint}"}}, "login": "{login_page}"}}"#
            ));
        })
        .await;
    let base = server.base_url();
    let poll = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/login/v2/poll")
                .form_urlencoded_tuple("token", "t0ken");
            then.status(200).body(format!(
                r#"{{"server": "{base}/nextcloud", "loginName": "ada", "appPassword": "secret"}}"#
            ));
        })
        .await;
    let http = http_client();
    let server_url = nextcloud::server_url(&server.url("/nextcloud")).unwrap();

    let login = nextcloud::start_browser_login(&http, &server_url)
        .await
        .unwrap();
    assert_eq!(login.login.as_str(), login_page);
    let credentials = nextcloud::wait_for_browser_login(
        &http,
        &login,
        Duration::from_millis(10),
        &AtomicBool::new(false),
    )
    .await
    .unwrap();

    assert_eq!(poll.calls_async().await, 1);
    assert_eq!(credentials.server, server_url);
    assert_eq!(credentials.user, "ada");
    assert_eq!(credentials.app_password, "secret");
}

#[tokio::test]
async fn a_password_login_keeps_an_app_password_instead() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/ocs/v2.php/core/getapppassword")
                .query_param("format", "json")
                .header("OCS-APIRequest", "true")
                .header("authorization", "Basic YWRhOnJlYWw=");
            then.status(200).body(
                r#"{"ocs": {"meta": {"status": "ok", "statuscode": 200, "message": "OK"},
                            "data": {"apppassword": "secret"}}}"#,
            );
        })
        .await;
    let http = http_client();
    let base = Url::parse(&server.url("/")).unwrap();

    let credentials = nextcloud::log_in_with_password(&http, &base, "ada", "real")
        .await
        .unwrap();
    assert_eq!(credentials, self::credentials(&server));
}

#[tokio::test]
async fn a_password_that_is_an_app_password_already_is_kept() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/ocs/v2.php/core/getapppassword");
            then.status(403);
        })
        .await;
    let base = Url::parse(&server.url("/")).unwrap();
    let credentials = nextcloud::log_in_with_password(&http_client(), &base, "ada", "secret")
        .await
        .unwrap();
    assert_eq!(credentials.app_password, "secret");
}

#[tokio::test]
async fn a_wrong_password_is_unauthorized() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/ocs/v2.php/core/getapppassword");
            then.status(401);
        })
        .await;
    let base = Url::parse(&server.url("/")).unwrap();
    assert!(matches!(
        nextcloud::log_in_with_password(&http_client(), &base, "ada", "wrong").await,
        Err(SyncError::Unauthorized)
    ));
}

#[tokio::test]
async fn log_out_revokes_the_app_password() {
    let server = MockServer::start_async().await;
    let revoke = server
        .mock_async(|when, then| {
            when.method(DELETE)
                .path("/ocs/v2.php/core/apppassword")
                .header("OCS-APIRequest", "true")
                .header("authorization", AUTH);
            then.status(200);
        })
        .await;
    nextcloud::revoke(&http_client(), &credentials(&server))
        .await
        .unwrap();
    assert_eq!(revoke.calls_async().await, 1);
}
