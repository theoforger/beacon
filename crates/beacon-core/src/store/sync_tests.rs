// SPDX-License-Identifier: GPL-3.0-or-later

//! The sync triggers, the echo guard, the merge rules and the first-sync
//! snapshot.

use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use url::Url;

use super::*;
use crate::model::{Enclosure, EpisodeInfo, ParsedEpisode, ShowInfo};
use crate::sync::ActionKind;

const FEED: &str = "https://example.com/feed.xml";

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 10, 12, 0, 0).unwrap()
}

fn audio(guid: &str) -> String {
    format!("https://example.com/{guid}.mp3")
}

/// An episode with audio; `guid` `None` stands for a feed without GUIDs,
/// where Beacon falls back to the enclosure URL.
fn episode(guid: Option<&str>, name: &str, duration: Option<u64>) -> ParsedEpisode {
    ParsedEpisode {
        info: EpisodeInfo {
            guid: guid.map_or_else(|| audio(name), str::to_owned),
            title: name.into(),
            duration: duration.map(Duration::from_secs),
            enclosure: Some(Enclosure {
                url: Url::parse(&audio(name)).unwrap(),
                mime_type: None,
                length: None,
            }),
            ..Default::default()
        },
        chapters: Vec::new(),
    }
}

fn add_show(store: &mut Store, feed: &str, episodes: &[ParsedEpisode]) -> ShowId {
    let info = ShowInfo {
        title: "Rooted".into(),
        ..Default::default()
    };
    let id = store
        .insert_show(&Url::parse(feed).unwrap(), &info, now())
        .unwrap();
    store.sync_episodes(id, episodes, now()).unwrap();
    id
}

fn signed_in() -> Store {
    let store = Store::open_in_memory().unwrap();
    store
        .set_sync_account(&Url::parse("https://cloud.example.com/").unwrap(), "ada")
        .unwrap();
    store
}

fn episode_id(store: &Store, show: ShowId, name: &str) -> EpisodeId {
    store
        .episodes(show)
        .unwrap()
        .into_iter()
        .find(|e| e.info.title == name)
        .unwrap()
        .id
}

fn everything(store: &Store) -> PendingMark {
    store.pending_mark().unwrap()
}

fn plays(store: &Store) -> Vec<PendingPlay> {
    store.pending_plays(everything(store)).unwrap()
}

fn action(guid: Option<&str>, episode: &str, position: u64, total: Option<u64>) -> EpisodeAction {
    EpisodeAction {
        podcast: FEED.to_owned(),
        episode: audio(episode),
        guid: guid.map(str::to_owned),
        action: ActionKind::Play,
        timestamp: Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap(),
        started: None,
        position: Some(position),
        total,
    }
}

#[test]
fn nothing_is_recorded_while_signed_out_or_switched_off() {
    let mut store = Store::open_in_memory().unwrap();
    let show = add_show(&mut store, FEED, &[episode(Some("g1"), "one", Some(600))]);
    let id = episode_id(&store, show, "one");
    store.set_position(id, Duration::from_secs(30)).unwrap();
    assert!(store.pending_mark().unwrap() == PendingMark::default());

    store
        .set_sync_account(&Url::parse("https://cloud.example.com/").unwrap(), "ada")
        .unwrap();
    store.set_sync_active(false).unwrap();
    store.set_position(id, Duration::from_secs(60)).unwrap();
    store.delete_show(show).unwrap();
    assert!(store.pending_mark().unwrap() == PendingMark::default());
}

#[test]
fn subscribing_and_unsubscribing_are_recorded_the_latest_winning() {
    let mut store = signed_in();
    let show = add_show(&mut store, FEED, &[]);
    let pending = store.pending_subscriptions(everything(&store)).unwrap();
    assert_eq!(pending.add, [FEED]);
    assert_eq!(store.pending_subscription(FEED).unwrap(), Some(true));

    store.delete_show(show).unwrap();
    let pending = store.pending_subscriptions(everything(&store)).unwrap();
    assert!(pending.add.is_empty());
    assert_eq!(pending.remove, [FEED]);
    assert_eq!(store.pending_subscription(FEED).unwrap(), Some(false));
}

#[test]
fn a_position_change_is_one_pending_play_keeping_where_it_started() {
    let mut store = signed_in();
    let show = add_show(&mut store, FEED, &[episode(Some("g1"), "one", Some(600))]);
    let id = episode_id(&store, show, "one");
    store.set_position(id, Duration::from_secs(15)).unwrap();
    store.set_position(id, Duration::from_secs(120)).unwrap();
    store.set_position(id, Duration::from_secs(120)).unwrap();

    let [play] = plays(&store).try_into().unwrap();
    assert_eq!(
        (play.started, play.position, play.total),
        (0, 120, 600),
        "the first start stays"
    );
    assert_eq!(play.feed_url, FEED);
    assert_eq!(play.guid, "g1");
    assert_eq!(play.episode_url, audio("one"));
    let sent = play.action(0);
    assert_eq!(sent.guid.as_deref(), Some("g1"));
    assert_eq!(sent.position, Some(120));
    assert_eq!(sent.timestamp.timestamp(), play.changed_at);
    // Never dated before the server's time of this sync, which other
    // devices may already have as their `since`.
    let later = play.changed_at + 600;
    assert_eq!(play.action(later).timestamp.timestamp(), later);
}

#[test]
fn played_sends_the_whole_length_even_after_finishing_resets_the_position() {
    let mut store = signed_in();
    let show = add_show(
        &mut store,
        FEED,
        &[
            episode(Some("g1"), "known", Some(600)),
            episode(Some("g2"), "unknown", None),
        ],
    );
    let known = episode_id(&store, show, "known");
    let unknown = episode_id(&store, show, "unknown");
    for id in [known, unknown] {
        store.set_position(id, Duration::from_secs(400)).unwrap();
        store.finish(id, now()).unwrap();
    }
    let plays = plays(&store);
    let totals: Vec<_> = plays.iter().map(|p| (p.position, p.total)).collect();
    assert_eq!(totals, [(600, 600), (400, 400)]);
    assert!(plays.iter().all(|p| crate::sync::finished(&p.action(0))));
}

#[test]
fn marking_unplayed_is_not_sent_and_drops_a_pending_play() {
    let mut store = signed_in();
    let show = add_show(&mut store, FEED, &[episode(Some("g1"), "one", Some(600))]);
    let id = episode_id(&store, show, "one");
    store.set_played(id, true, now()).unwrap();
    assert_eq!(plays(&store).len(), 1);
    store.set_played(id, false, now()).unwrap();
    assert!(plays(&store).is_empty());
}

#[test]
fn reset_progress_is_sent_as_a_play_at_the_start() {
    let mut store = signed_in();
    let show = add_show(&mut store, FEED, &[episode(Some("g1"), "one", Some(600))]);
    let id = episode_id(&store, show, "one");
    store.set_position(id, Duration::from_secs(90)).unwrap();
    store.set_position(id, Duration::ZERO).unwrap();
    let [play] = plays(&store).try_into().unwrap();
    assert_eq!(play.position, 0);
}

#[test]
fn episodes_without_audio_are_not_recorded() {
    let mut store = signed_in();
    let mut silent = episode(Some("g1"), "silent", None);
    silent.info.enclosure = None;
    let show = add_show(&mut store, FEED, &[silent]);
    let id = episode_id(&store, show, "silent");
    store.set_played(id, true, now()).unwrap();
    assert!(plays(&store).is_empty());
}

#[test]
fn a_pending_play_survives_unsubscribing() {
    let mut store = signed_in();
    let show = add_show(&mut store, FEED, &[episode(Some("g1"), "one", Some(600))]);
    let id = episode_id(&store, show, "one");
    store.set_position(id, Duration::from_secs(42)).unwrap();
    store.delete_show(show).unwrap();
    let [play] = plays(&store).try_into().unwrap();
    assert_eq!((play.feed_url.as_str(), play.position), (FEED, 42));
}

#[test]
fn a_made_up_guid_is_not_sent() {
    let mut store = signed_in();
    let show = add_show(&mut store, FEED, &[episode(None, "one", Some(600))]);
    let id = episode_id(&store, show, "one");
    store.set_position(id, Duration::from_secs(10)).unwrap();
    let [play] = plays(&store).try_into().unwrap();
    let sent = play.action(0);
    assert_eq!(sent.guid, None);
    assert_eq!(sent.episode, audio("one"));
}

#[test]
fn changes_written_quietly_are_not_recorded_and_the_guard_lifts() {
    let mut store = signed_in();
    let show = store
        .quietly(|store| {
            Ok(add_show(
                store,
                FEED,
                &[episode(Some("g1"), "one", Some(600))],
            ))
        })
        .unwrap();
    let id = episode_id(&store, show, "one");
    store
        .quietly(|store| store.set_position(id, Duration::from_secs(5)))
        .unwrap();
    assert_eq!(store.pending_mark().unwrap(), PendingMark::default());

    // The guard lifts even when the work fails.
    let failed: Result<(), StoreError> = store.quietly(|_| {
        Err(StoreError::TooNew {
            applied: 0,
            known: 0,
        })
    });
    assert!(failed.is_err());
    store.set_position(id, Duration::from_secs(6)).unwrap();
    assert_eq!(plays(&store).len(), 1);
}

#[test]
fn a_guard_left_by_a_crash_is_lifted_when_the_library_opens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("beacon.db");
    {
        let store = Store::open(&path).unwrap();
        store.set_guard(true).unwrap();
    }
    let mut store = Store::open(&path).unwrap();
    store
        .set_sync_account(&Url::parse("https://cloud.example.com/").unwrap(), "ada")
        .unwrap();
    add_show(&mut store, FEED, &[]);
    assert_eq!(
        store
            .pending_subscriptions(store.pending_mark().unwrap())
            .unwrap()
            .add,
        [FEED]
    );
}

#[test]
fn a_newer_server_position_wins_and_replaces_the_local_change() {
    let mut store = signed_in();
    let show = add_show(&mut store, FEED, &[episode(Some("g1"), "one", Some(600))]);
    let id = episode_id(&store, show, "one");
    store.set_position(id, Duration::from_secs(30)).unwrap();

    // Recorded now; an action from 2000 is older and loses.
    let old = action(Some("g1"), "one", 300, Some(600));
    assert_eq!(store.apply_play(&old).unwrap(), Applied::LocalNewer);
    assert_eq!(
        store.episode(id).unwrap().unwrap().state.position,
        Duration::from_secs(30)
    );

    let mut new = old.clone();
    new.timestamp = Utc::now() + chrono::Duration::hours(1);
    assert_eq!(store.apply_play(&new).unwrap(), Applied::Changed(id));
    assert_eq!(
        store.episode(id).unwrap().unwrap().state.position,
        Duration::from_secs(300)
    );
    assert!(
        plays(&store).is_empty(),
        "the server's change is not echoed"
    );
}

#[test]
fn a_finished_play_marks_played_and_a_partial_one_never_unplays() {
    let mut store = signed_in();
    let show = add_show(&mut store, FEED, &[episode(Some("g1"), "one", Some(600))]);
    let id = episode_id(&store, show, "one");

    store
        .apply_play(&action(Some("g1"), "one", 597, Some(600)))
        .unwrap();
    let state = store.episode(id).unwrap().unwrap().state;
    assert!(state.played);
    assert_eq!(state.position, Duration::ZERO);

    store
        .apply_play(&action(Some("g1"), "one", 100, Some(600)))
        .unwrap();
    let state = store.episode(id).unwrap().unwrap().state;
    assert!(state.played, "played is sticky");
    assert_eq!(
        state.position,
        Duration::from_secs(100),
        "but it resumes there"
    );
    assert!(plays(&store).is_empty());

    // The same action again, as an echo, changes nothing.
    assert_eq!(
        store
            .apply_play(&action(Some("g1"), "one", 100, Some(600)))
            .unwrap(),
        Applied::Same(id)
    );
}

#[test]
fn actions_match_by_guid_then_by_enclosure() {
    let mut store = signed_in();
    let show = add_show(
        &mut store,
        FEED,
        &[episode(Some("g1"), "one", None), episode(None, "two", None)],
    );
    let one = episode_id(&store, show, "one");
    let two = episode_id(&store, show, "two");
    // A GUID the episode does not have, but its audio file.
    assert_eq!(
        store
            .apply_play(&action(Some("other"), "one", 10, None))
            .unwrap(),
        Applied::Changed(one)
    );
    assert_eq!(
        store.apply_play(&action(None, "two", 20, None)).unwrap(),
        Applied::Changed(two)
    );
    assert_eq!(
        store.apply_play(&action(None, "three", 20, None)).unwrap(),
        Applied::Unmatched(Some(show))
    );
    let mut elsewhere = action(None, "one", 20, None);
    elsewhere.podcast = "https://elsewhere.example/feed".to_owned();
    assert_eq!(
        store.apply_play(&elsewhere).unwrap(),
        Applied::Unmatched(None)
    );
}

#[test]
fn the_first_sync_snapshot_holds_every_show_and_every_episode_listened_to() {
    let mut store = Store::open_in_memory().unwrap();
    let show = add_show(
        &mut store,
        FEED,
        &[
            episode(Some("g1"), "played", Some(600)),
            episode(Some("g2"), "started", Some(600)),
            episode(Some("g3"), "untouched", Some(600)),
        ],
    );
    store
        .set_played(episode_id(&store, show, "played"), true, now())
        .unwrap();
    store
        .set_position(episode_id(&store, show, "started"), Duration::from_secs(50))
        .unwrap();
    store
        .set_sync_account(&Url::parse("https://cloud.example.com/").unwrap(), "ada")
        .unwrap();
    assert!(store.sync_account().unwrap().unwrap().is_first_sync());

    store.record_library().unwrap();
    store.record_library().unwrap();
    let mark = everything(&store);
    assert_eq!(store.pending_subscriptions(mark).unwrap().add, [FEED]);
    let plays = plays(&store);
    let summary: Vec<_> = plays
        .iter()
        .map(|p| (p.guid.as_str(), p.position, p.changed_at))
        .collect();
    assert_eq!(
        summary,
        [("g1", 600, now().timestamp()), ("g2", 50, 0)],
        "a played episode dates from when it was played; a position has no date"
    );
}

#[test]
fn finishing_a_sync_clears_only_what_it_sent_and_keeps_the_server_times() {
    let mut store = signed_in();
    let show = add_show(&mut store, FEED, &[episode(Some("g1"), "one", Some(600))]);
    let id = episode_id(&store, show, "one");
    store.set_position(id, Duration::from_secs(10)).unwrap();
    let mark = store.pending_mark().unwrap();
    // A change while the sync runs.
    store.set_position(id, Duration::from_secs(20)).unwrap();

    store.finish_sync(mark, 100, 200, now()).unwrap();
    let account = store.sync_account().unwrap().unwrap();
    assert_eq!(
        (account.subscriptions_since, account.actions_since),
        (Some(100), Some(200))
    );
    assert_eq!(account.last_synced, Some(now()));
    assert!(!account.is_first_sync());
    assert!(
        store
            .pending_subscriptions(everything(&store))
            .unwrap()
            .is_empty()
    );
    let [play] = plays(&store).try_into().unwrap();
    assert_eq!(play.position, 20, "the change made meanwhile waits");
}

#[test]
fn logging_out_forgets_the_account_and_the_pending_changes() {
    let mut store = signed_in();
    add_show(&mut store, FEED, &[]);
    store.remove_sync_account().unwrap();
    assert_eq!(store.sync_account().unwrap(), None);
    assert_eq!(store.pending_mark().unwrap(), PendingMark::default());
}
