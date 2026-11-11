// SPDX-License-Identifier: GPL-3.0-or-later

use chrono::{TimeZone, Utc};

use super::nextcloud::server_url;
use super::*;

#[test]
fn reads_subscription_changes() {
    let update: SubscriptionUpdate = serde_json::from_str(
        r#"{"add": ["https://a.example/feed"], "remove": ["https://b.example/rss"],
            "timestamp": 1700000000}"#,
    )
    .unwrap();
    assert_eq!(update.changes.add, ["https://a.example/feed"]);
    assert_eq!(update.changes.remove, ["https://b.example/rss"]);
    assert_eq!(update.timestamp, 1_700_000_000);
}

#[test]
fn reads_actions_in_either_case_with_or_without_a_zone() {
    let update: ActionUpdate = serde_json::from_str(
        r#"{"actions": [
            {"podcast": "http://example.com/feed.rss",
             "episode": "http://example.com/files/s01e20.mp3",
             "guid": "s01e20-example-org", "action": "PLAY",
             "timestamp": "2009-12-12T09:00:00", "started": 15, "position": 120, "total": 500},
            {"podcast": "http://example.com/feed.rss",
             "episode": "http://example.com/files/s01e21.mp3",
             "action": "play", "timestamp": "2009-12-12T09:00:00+01:00",
             "started": -1, "position": 30, "total": -1},
            {"podcast": "http://example.com/feed.rss",
             "episode": "http://example.com/files/s01e21.mp3",
             "action": "download", "timestamp": "2009-12-12T09:00:00.250Z"},
            {"podcast": "http://example.com/feed.rss",
             "episode": "http://example.com/files/s01e21.mp3",
             "action": "flattr", "timestamp": "2009-12-12T09:00:00"}
        ], "timestamp": 12345}"#,
    )
    .unwrap();
    let [first, second, third, fourth] = update.actions.as_slice() else {
        panic!("four actions");
    };
    assert_eq!(first.action, ActionKind::Play);
    assert_eq!(first.guid.as_deref(), Some("s01e20-example-org"));
    assert_eq!(
        first.timestamp,
        Utc.with_ymd_and_hms(2009, 12, 12, 9, 0, 0).unwrap()
    );
    assert_eq!(
        (first.started, first.position, first.total),
        (Some(15), Some(120), Some(500))
    );
    assert_eq!(second.action, ActionKind::Play);
    assert_eq!(second.guid, None);
    assert_eq!(
        second.timestamp,
        Utc.with_ymd_and_hms(2009, 12, 12, 8, 0, 0).unwrap()
    );
    assert_eq!(
        (second.started, second.position, second.total),
        (None, Some(30), None)
    );
    assert_eq!(third.action, ActionKind::Download);
    assert_eq!(third.position, None);
    assert_eq!(fourth.action, ActionKind::Other("flattr".to_owned()));
    assert_eq!(update.timestamp, 12345);
}

#[test]
fn writes_actions_upper_case_utc_without_a_zone() {
    let action = EpisodeAction {
        podcast: "http://example.com/feed.rss".to_owned(),
        episode: "http://example.com/1.mp3".to_owned(),
        guid: None,
        action: ActionKind::Play,
        timestamp: Utc.with_ymd_and_hms(2026, 11, 10, 8, 30, 5).unwrap(),
        started: None,
        position: Some(120),
        total: Some(500),
    };
    let json: serde_json::Value = serde_json::to_value([action]).unwrap();
    assert_eq!(
        json,
        serde_json::json!([{
            "podcast": "http://example.com/feed.rss",
            "episode": "http://example.com/1.mp3",
            "action": "PLAY",
            "timestamp": "2026-11-10T08:30:05",
            "started": -1,
            "position": 120,
            "total": 500
        }])
    );
}

#[test]
fn a_bad_action_time_does_not_parse() {
    let result: Result<EpisodeAction, _> = serde_json::from_str(
        r#"{"podcast": "a", "episode": "b", "action": "PLAY", "timestamp": "yesterday"}"#,
    );
    assert!(result.is_err());
}

fn play(position: Option<u64>, total: Option<u64>) -> EpisodeAction {
    EpisodeAction {
        podcast: String::new(),
        episode: String::new(),
        guid: None,
        action: ActionKind::Play,
        timestamp: Utc::now(),
        started: None,
        position,
        total,
    }
}

#[test]
fn a_play_within_five_seconds_of_the_end_is_finished() {
    assert!(finished(&play(Some(500), Some(500))));
    assert!(finished(&play(Some(495), Some(500))));
    assert!(!finished(&play(Some(494), Some(500))));
    assert!(!finished(&play(Some(10), None)));
    assert!(!finished(&play(None, Some(500))));
    assert!(!finished(&play(Some(0), Some(0))));
}

#[test]
fn server_addresses_get_a_scheme_and_a_trailing_slash() {
    assert_eq!(
        server_url("cloud.example.com").unwrap().as_str(),
        "https://cloud.example.com/"
    );
    assert_eq!(
        server_url(" http://localhost:8080/nextcloud ")
            .unwrap()
            .as_str(),
        "http://localhost:8080/nextcloud/"
    );
    assert_eq!(
        server_url("https://cloud.example.com/?x=1#y")
            .unwrap()
            .as_str(),
        "https://cloud.example.com/"
    );
    assert!(matches!(server_url(""), Err(SyncError::InvalidServer)));
    assert!(matches!(
        server_url("ftp://cloud.example.com"),
        Err(SyncError::InvalidServer)
    ));
    assert!(matches!(
        server_url("https://"),
        Err(SyncError::InvalidServer)
    ));
}
