// SPDX-License-Identifier: GPL-3.0-or-later

use chrono::TimeZone;

use super::*;
use crate::model::EpisodeInfo;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0).unwrap()
}

fn episode(guid: &str, title: &str, day: u32) -> ParsedEpisode {
    ParsedEpisode {
        info: EpisodeInfo {
            guid: guid.into(),
            title: title.into(),
            published: Some(Utc.with_ymd_and_hms(2026, 10, day, 8, 0, 0).unwrap()),
            ..Default::default()
        },
        chapters: Vec::new(),
    }
}

fn store_with_show() -> (Store, ShowId) {
    let store = Store::open_in_memory().unwrap();
    let url = Url::parse("https://example.com/feed.xml").unwrap();
    let info = ShowInfo {
        title: "Rooted".into(),
        ..Default::default()
    };
    let id = store.insert_show(&url, &info, now()).unwrap();
    (store, id)
}

#[test]
fn migrations_are_valid() {
    MIGRATIONS.validate().unwrap();
}

#[test]
fn sync_adds_then_updates_without_touching_state() {
    let (mut store, show) = store_with_show();
    let first = [episode("a", "A", 1), episode("b", "B", 2)];
    assert_eq!(
        store.sync_episodes(show, &first, now()).unwrap(),
        SyncStats {
            added: 2,
            updated: 0
        }
    );

    store
        .conn
        .execute(
            "UPDATE episodes SET played = 1, favorite = 1 WHERE guid = 'a'",
            [],
        )
        .unwrap();

    let second = [
        episode("a", "A, renamed", 1),
        episode("b", "B", 2),
        episode("c", "C", 3),
    ];
    assert_eq!(
        store.sync_episodes(show, &second, now()).unwrap(),
        SyncStats {
            added: 1,
            updated: 1
        }
    );

    let episodes = store.episodes(show).unwrap();
    let titles: Vec<_> = episodes.iter().map(|e| e.info.title.as_str()).collect();
    assert_eq!(titles, ["C", "B", "A, renamed"]);
    let a = &episodes[2];
    assert!(a.state.played && a.state.favorite);
}

#[test]
fn mark_seen_except_newest_leaves_one_dot() {
    let (mut store, show) = store_with_show();
    let eps = [
        episode("a", "A", 1),
        episode("c", "C", 3),
        episode("b", "B", 2),
    ];
    store.sync_episodes(show, &eps, now()).unwrap();
    store.mark_seen_except_newest(show).unwrap();
    let unseen: Vec<_> = store
        .episodes(show)
        .unwrap()
        .into_iter()
        .filter(|e| !e.state.seen)
        .map(|e| e.info.guid)
        .collect();
    assert_eq!(unseen, ["c"]);
}

#[test]
fn chapters_are_replaced_on_sync() {
    let (mut store, show) = store_with_show();
    let mut ep = episode("a", "A", 1);
    ep.chapters = vec![Chapter {
        start: Duration::from_secs(0),
        title: "Cold open".into(),
        url: None,
        image_url: None,
    }];
    store
        .sync_episodes(show, std::slice::from_ref(&ep), now())
        .unwrap();
    ep.chapters.push(Chapter {
        start: Duration::from_secs(192),
        title: "Why quiet is hard to protect".into(),
        url: None,
        image_url: None,
    });
    store.sync_episodes(show, &[ep], now()).unwrap();
    let id = store.episodes(show).unwrap()[0].id;
    let titles: Vec<_> = store
        .chapters(id)
        .unwrap()
        .into_iter()
        .map(|c| c.title)
        .collect();
    assert_eq!(titles, ["Cold open", "Why quiet is hard to protect"]);
}

#[test]
fn full_text_search_follows_updates() {
    let (mut store, show) = store_with_show();
    store
        .sync_episodes(show, &[episode("a", "A dish in the desert", 1)], now())
        .unwrap();
    assert_eq!(store.search_episodes("desert").unwrap().len(), 1);
    store
        .sync_episodes(show, &[episode("a", "Listening to silence", 1)], now())
        .unwrap();
    assert!(store.search_episodes("desert").unwrap().is_empty());
    assert_eq!(store.search_episodes("silen").unwrap().len(), 1);
    assert!(store.search_episodes("\"unbalanced").unwrap().is_empty());
}

#[test]
fn deleting_a_show_removes_its_episodes() {
    let (mut store, show) = store_with_show();
    store
        .sync_episodes(show, &[episode("a", "A", 1)], now())
        .unwrap();
    store.conn.execute("DELETE FROM shows", []).unwrap();
    let count: i64 = store
        .conn
        .query_row("SELECT count(*) FROM episodes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}
