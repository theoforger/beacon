// SPDX-License-Identifier: GPL-3.0-or-later

//! The playback scenario on a real `GstPlay`: a short local file, a silent
//! sink that keeps real time, and a real store for the position.

use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use beacon_core::model::{EpisodeInfo, ParsedEpisode, ShowInfo};
use beacon_core::playback;
use beacon_core::store::Store;
use chrono::Utc;
use url::Url;

use super::*;

fn fixture_uri() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/tone-30s.ogg");
    glib::filename_to_uri(path.canonicalize().unwrap(), None)
        .unwrap()
        .to_string()
}

/// A sink that plays nothing but keeps real time, so the position moves.
fn silent_sink() -> gst::Element {
    gst::ElementFactory::make("fakesink")
        .property("sync", true)
        .build()
        .unwrap()
}

/// Run `context` until `done` says so, or fail after a while.
fn run_until(
    context: &glib::MainContext,
    events: &mpsc::Receiver<PlayerEvent>,
    mut done: impl FnMut(&PlayerEvent) -> bool,
) -> PlayerEvent {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        while context.iteration(false) {}
        while let Ok(event) = events.try_recv() {
            if let PlayerEvent::Error(err) = &event {
                panic!("playback failed: {err}");
            }
            if done(&event) {
                return event;
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the player"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn player(context: &glib::MainContext) -> (Player, mpsc::Receiver<PlayerEvent>) {
    gst::init().unwrap();
    let (send, events) = mpsc::channel();
    let player = Player::with_audio_sink(context, &silent_sink(), move |event| {
        let _ = send.send(event);
    });
    (player, events)
}

#[test]
fn playback_resumes_where_it_was_paused_after_closing() {
    let context = glib::MainContext::new();
    let _guard = context.acquire().unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let show = store
        .insert_show(
            &Url::parse("https://example.com/feed.xml").unwrap(),
            &ShowInfo::default(),
            Utc::now(),
        )
        .unwrap();
    let parsed = ParsedEpisode {
        info: EpisodeInfo {
            guid: "tone".into(),
            title: "Tone".into(),
            ..Default::default()
        },
        chapters: Vec::new(),
    };
    store.sync_episodes(show, &[parsed], Utc::now()).unwrap();
    let id = store.episodes(show).unwrap()[0].id;
    let uri = fixture_uri();

    // Play about two seconds, pause, save where it got to, close.
    let (first, events) = player(&context);
    first.load(&uri, Duration::ZERO);
    let duration = match run_until(&context, &events, |e| matches!(e, PlayerEvent::Duration(_))) {
        PlayerEvent::Duration(d) => d,
        _ => unreachable!(),
    };
    assert!(duration > Duration::from_secs(29) && duration < Duration::from_secs(31));
    run_until(
        &context,
        &events,
        |e| matches!(e, PlayerEvent::Position(p) if *p >= Duration::from_secs(2)),
    );
    first.pause();
    run_until(&context, &events, |e| {
        matches!(e, PlayerEvent::State(PlaybackState::Paused))
    });
    let paused_at = first.position().unwrap();
    store.set_position(id, paused_at).unwrap();
    first.stop();
    drop(first);

    // Reopen: the episode resumes at the saved second, and plays on.
    let state = store.episode(id).unwrap().unwrap().state;
    let start = playback::resume_position(&state, Some(duration));
    assert_eq!(start.as_secs(), paused_at.as_secs());
    assert!(start >= Duration::from_secs(2));

    let (second, events) = player(&context);
    second.load(&uri, start);
    let resumed = match run_until(
        &context,
        &events,
        |e| matches!(e, PlayerEvent::Position(p) if *p >= start),
    ) {
        PlayerEvent::Position(p) => p,
        _ => unreachable!(),
    };
    assert!(
        resumed < start + Duration::from_secs(1),
        "resumed at {resumed:?}, expected about {start:?}"
    );
    second.stop();
}
