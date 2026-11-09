// SPDX-License-Identifier: GPL-3.0-or-later

use chrono::TimeZone;

use super::*;
use crate::model::{EpisodeChange, EpisodeInfo};

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

fn version(conn: &Connection) -> u32 {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

#[test]
fn a_library_gets_the_changes_it_has_not_had() {
    let mut conn = Connection::open_in_memory().unwrap();
    // As a library made before the second change.
    conn.execute_batch(MIGRATIONS[0]).unwrap();
    conn.pragma_update(None, "user_version", 1).unwrap();
    migrate(&mut conn).unwrap();
    assert_eq!(version(&conn), 3);
    // Applying again changes nothing.
    migrate(&mut conn).unwrap();
    assert_eq!(version(&conn), 3);
}

#[test]
fn a_library_from_a_newer_beacon_is_left_alone() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "user_version", 9).unwrap();
    assert!(matches!(
        migrate(&mut conn),
        Err(StoreError::TooNew {
            applied: 9,
            known: 3
        })
    ));
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

#[test]
fn positions_are_saved_and_a_finished_episode_starts_over() {
    let (mut store, show) = store_with_show();
    store
        .sync_episodes(show, &[episode("a", "A", 1)], now())
        .unwrap();
    let id = store.episodes(show).unwrap()[0].id;
    store.enqueue(id).unwrap();

    store
        .set_position(id, std::time::Duration::from_millis(754_900))
        .unwrap();
    let state = store.episode(id).unwrap().unwrap().state;
    assert_eq!(state.position, std::time::Duration::from_secs(754));
    assert!(state.is_partly_played());

    store.finish(id, now()).unwrap();
    let state = store.episode(id).unwrap().unwrap().state;
    assert!(state.played);
    assert_eq!(state.position, std::time::Duration::ZERO);
    // Finishing is not dequeuing: moving on is the player's job.
    assert_eq!(store.queue().unwrap(), [id]);
}

#[test]
fn marking_a_show_unplayed_starts_it_over_and_keeps_the_queue() {
    let (mut store, show) = store_with_show();
    store
        .sync_episodes(show, &[episode("a", "A", 1), episode("b", "B", 2)], now())
        .unwrap();
    let ids: Vec<_> = store.episodes(show).unwrap().iter().map(|e| e.id).collect();
    store.mark_seen(ids[0]).unwrap();
    store.enqueue(ids[1]).unwrap();
    store.mark_all_played(show, now()).unwrap();
    store
        .set_position(ids[0], std::time::Duration::from_secs(90))
        .unwrap();

    assert_eq!(store.mark_all_unplayed(show).unwrap(), 2);
    for id in &ids {
        let state = store.episode(*id).unwrap().unwrap().state;
        assert!(!state.played && state.played_at.is_none());
        assert_eq!(state.position, std::time::Duration::ZERO);
    }
    // The seen episode keeps no dot; the queue is untouched.
    assert!(!store.episode(ids[0]).unwrap().unwrap().state.is_new());
    assert_eq!(store.queue().unwrap(), [ids[1]]);
}

#[test]
fn a_change_applies_to_every_episode_chosen() {
    let (mut store, show) = store_with_show();
    store
        .sync_episodes(
            show,
            &[
                episode("a", "A", 1),
                episode("b", "B", 2),
                episode("c", "C", 3),
            ],
            now(),
        )
        .unwrap();
    let ids: Vec<_> = store.episodes(show).unwrap().iter().map(|e| e.id).collect();
    store.enqueue(ids[2]).unwrap();

    // Queued in the order chosen, after what was queued; one already
    // queued keeps its place.
    let chosen = [ids[1], ids[2], ids[0]];
    store
        .change_episodes(&chosen, EpisodeChange::Queued(true), now())
        .unwrap();
    assert_eq!(store.queue().unwrap(), [ids[2], ids[1], ids[0]]);

    store
        .change_episodes(&ids[..2], EpisodeChange::Played(true), now())
        .unwrap();
    store
        .change_episodes(&ids[1..], EpisodeChange::Favorite(true), now())
        .unwrap();
    let states: Vec<_> = ids
        .iter()
        .map(|id| store.episode(*id).unwrap().unwrap().state)
        .collect();
    assert_eq!(
        states
            .iter()
            .map(|s| (s.played, s.favorite))
            .collect::<Vec<_>>(),
        [(true, false), (true, true), (false, true)]
    );
    // Marking played never touches the queue.
    assert_eq!(store.queue().unwrap().len(), 3);

    store
        .change_episodes(&ids, EpisodeChange::Queued(false), now())
        .unwrap();
    assert!(store.queue().unwrap().is_empty());
}
