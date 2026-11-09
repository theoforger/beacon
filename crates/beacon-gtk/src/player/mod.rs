// SPDX-License-Identifier: GPL-3.0-or-later

//! Playback through GStreamer's `GstPlay`. The player reports what happens
//! as typed events on the main loop; the rules (where to resume, how far to
//! skip, when to save) live in `beacon_core::playback`.

mod buffering;
pub mod mpris;

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use beacon_core::PlaybackError;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_play as gst_play;
use gtk::glib;

/// How often the player reports the position while playing.
const POSITION_INTERVAL_MS: u32 = 250;

/// How often the player reports how much it holds.
const BUFFERED_INTERVAL: Duration = Duration::from_millis(500);

/// What the player is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Buffering,
    Paused,
    Playing,
}

/// What the player reports.
#[derive(Debug, Clone)]
pub enum PlayerEvent {
    State(PlaybackState),
    Position(Duration),
    Duration(Duration),
    /// How far into the episode the player holds, as a share of it.
    Buffered(f64),
    /// The episode played to its end.
    Finished,
    Error(PlaybackError),
}

type OnEvent = Arc<dyn Fn(PlayerEvent) + Send + Sync>;

/// One of the player's two `GstPlay`s: the one playing, or the other,
/// holding the next episode opened and paused so its start is buffered.
#[derive(Debug)]
struct Slot {
    play: gst_play::Play,
    _signals: gst_play::PlaySignalAdapter,
    /// What it has open.
    uri: RefCell<Option<String>>,
}

/// One episode at a time, through `GstPlay`, with the next one preloaded in
/// a second `GstPlay` once asked (asked by the user 2026-11-08): when it is
/// played, the two swap and it starts at once. Only the playing one's
/// events are reported.
pub struct Player {
    slots: [Slot; 2],
    /// Which slot plays.
    active: Arc<AtomicUsize>,
    /// Where to seek once the stream is ready: seeking before it has
    /// prerolled is not reliable.
    start: Arc<Mutex<Option<gst::ClockTime>>>,
    on_event: OnEvent,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Player")
            .field("slots", &self.slots)
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

/// Seek to the start `load` asked for, once.
fn seek_pending(play: &gst_play::Play, start: &Mutex<Option<gst::ClockTime>>) {
    if let Some(position) = start.lock().ok().and_then(|mut s| s.take()) {
        play.seek(position);
    }
}

impl Player {
    /// A player whose events arrive on `context`, playing to the default
    /// audio output, buffering streams to files in `buffer_dir`.
    pub fn new(
        context: &glib::MainContext,
        buffer_dir: &Path,
        on_event: impl Fn(PlayerEvent) + Send + Sync + 'static,
    ) -> Result<Self, PlaybackError> {
        gst::init().map_err(|err| PlaybackError::Unavailable(err.to_string()))?;
        let player = Self::build(context, Arc::new(on_event));
        for slot in &player.slots {
            buffering::download_to(&slot.play, buffer_dir);
        }
        Ok(player)
    }

    /// Report how much of the playing episode the player holds, when it
    /// changes, for as long as the player lasts.
    fn report_buffered(&self, context: &glib::MainContext) {
        let plays = self.slots.each_ref().map(|slot| slot.play.downgrade());
        let (active, emit) = (self.active.clone(), self.on_event.clone());
        let last = Cell::new(-1.0);
        let source = glib::timeout_source_new(
            BUFFERED_INTERVAL,
            Some("beacon-buffered"),
            glib::Priority::DEFAULT_IDLE,
            move || {
                let Some(play) = plays[active.load(Ordering::SeqCst) % 2].upgrade() else {
                    return glib::ControlFlow::Break;
                };
                if let Some(share) = buffering::buffered(&play)
                    && (share - last.get()).abs() >= 0.001
                {
                    last.set(share);
                    emit(PlayerEvent::Buffered(share));
                }
                glib::ControlFlow::Continue
            },
        );
        source.attach(Some(context));
    }

    /// A player that plays to sinks from `sink` instead of the audio output
    /// (tests).
    #[cfg(test)]
    fn with_audio_sink(
        context: &glib::MainContext,
        sink: impl Fn() -> gst::Element,
        on_event: impl Fn(PlayerEvent) + Send + Sync + 'static,
    ) -> Self {
        let player = Self::build(context, Arc::new(on_event));
        for slot in &player.slots {
            slot.play.pipeline().set_property("audio-sink", sink());
        }
        player
    }

    fn build(context: &glib::MainContext, on_event: OnEvent) -> Self {
        let active = Arc::new(AtomicUsize::new(0));
        let start: Arc<Mutex<Option<gst::ClockTime>>> = Arc::default();
        let slots = [0, 1].map(|index| slot(context, index, &active, &start, &on_event));
        let player = Self {
            slots,
            active,
            start,
            on_event,
        };
        player.report_buffered(context);
        player
    }

    fn playing(&self) -> &Slot {
        &self.slots[self.active.load(Ordering::SeqCst) % 2]
    }

    fn standby(&self) -> &Slot {
        &self.slots[(self.active.load(Ordering::SeqCst) + 1) % 2]
    }

    /// Open `uri` and start playing it from `start`; at once if it was
    /// preloaded.
    pub fn load(&self, uri: &str, start: Duration) {
        self.open_at(uri, start, true);
    }

    /// Open `uri` at `start`, paused, so it buffers before it is played.
    pub fn open_paused(&self, uri: &str, start: Duration) {
        self.open_at(uri, start, false);
    }

    fn open_at(&self, uri: &str, start: Duration, play: bool) {
        if let Ok(mut pending) = self.start.lock() {
            *pending = (!start.is_zero()).then(|| clock_time(start));
        }
        let preloaded = self.standby().uri.borrow().as_deref() == Some(uri);
        let old = self.playing();
        old.play.stop();
        old.uri.replace(None);
        if preloaded {
            self.active.fetch_add(1, Ordering::SeqCst);
            let slot = self.playing();
            if slot
                .play
                .media_info()
                .is_some_and(|info| info.is_seekable())
            {
                seek_pending(&slot.play, &self.start);
            }
            if play {
                slot.play.play();
            }
            // Reported while it waited, so not passed on then.
            if let Some(duration) = slot.play.duration().filter(|d| !d.is_zero()) {
                (self.on_event)(PlayerEvent::Duration(duration.into()));
            }
            return;
        }
        let slot = self.playing();
        open(&slot.play, uri);
        slot.uri.replace(Some(uri.to_owned()));
        if play {
            slot.play.play();
        } else {
            slot.play.pause();
        }
    }

    /// Open `uri` paused in the background, so it starts at once when
    /// played; `None` lets go of what was preloaded.
    pub fn preload(&self, uri: Option<&str>) {
        let slot = self.standby();
        if slot.uri.borrow().as_deref() == uri {
            return;
        }
        slot.play.stop();
        slot.uri.replace(uri.map(str::to_owned));
        let Some(uri) = uri else {
            return;
        };
        let playing = &self.playing().play;
        slot.play.set_volume(playing.volume());
        slot.play.set_rate(playing.rate());
        open(&slot.play, uri);
        slot.play.pause();
    }

    pub fn play(&self) {
        self.playing().play.play();
    }

    pub fn pause(&self) {
        self.playing().play.pause();
    }

    pub fn seek(&self, position: Duration) {
        self.playing().play.seek(clock_time(position));
    }

    pub fn set_rate(&self, rate: f64) {
        for slot in &self.slots {
            slot.play.set_rate(rate);
        }
    }

    /// Beacon's own stream volume, from 0 to 1.
    pub fn set_volume(&self, volume: f64) {
        for slot in &self.slots {
            slot.play.set_volume(volume.clamp(0.0, 1.0));
        }
    }
}

/// Open `uri` without playing it yet.
fn open(play: &gst_play::Play, uri: &str) {
    play.set_uri(Some(uri));
    // Podcasts are listened to; a video enclosure plays its sound only.
    play.set_video_track_enabled(false);
}

/// Slot `index`, reporting its events only while it is the active one.
fn slot(
    context: &glib::MainContext,
    index: usize,
    active: &Arc<AtomicUsize>,
    start: &Arc<Mutex<Option<gst::ClockTime>>>,
    on_event: &OnEvent,
) -> Slot {
    let play = gst_play::Play::new(None::<gst_play::PlayVideoRenderer>);
    let mut config = play.config();
    config.set_position_update_interval(POSITION_INTERVAL_MS);
    // Exact seeks: a chapter, or a skip, lands on its second rather
    // than the nearest keyframe, seconds away in an Ogg or MP3 stream.
    config.set_seek_accurate(true);
    // Rarely fails, and only for a bad config; the default interval
    // then applies.
    if let Err(err) = play.set_config(config) {
        tracing::warn!("could not configure the player: {err}");
    }
    let signals = gst_play::PlaySignalAdapter::with_main_context(&play, context);
    let is_active = {
        let active = active.clone();
        move || active.load(Ordering::SeqCst) % 2 == index
    };

    let (emit, current) = (on_event.clone(), is_active.clone());
    let seek = (play.clone(), start.clone());
    signals.connect_state_changed(move |_, state| {
        if !current() {
            return;
        }
        let state = match state {
            gst_play::PlayState::Buffering => PlaybackState::Buffering,
            gst_play::PlayState::Paused => PlaybackState::Paused,
            gst_play::PlayState::Playing => PlaybackState::Playing,
            _ => PlaybackState::Stopped,
        };
        // An HTTP stream plays before it is known to be seekable; the
        // seek then waits for the media info that says it is.
        if matches!(state, PlaybackState::Paused | PlaybackState::Playing)
            && seek.0.media_info().is_some_and(|info| info.is_seekable())
        {
            seek_pending(&seek.0, &seek.1);
        }
        emit(PlayerEvent::State(state));
    });
    let (seek, current) = ((play.clone(), start.clone()), is_active.clone());
    signals.connect_media_info_updated(move |_, info| {
        if current() && info.is_seekable() {
            seek_pending(&seek.0, &seek.1);
        }
    });
    let (emit, current) = (on_event.clone(), is_active.clone());
    signals.connect_position_updated(move |_, position| {
        if let Some(position) = position.filter(|_| current()) {
            emit(PlayerEvent::Position(position.into()));
        }
    });
    let (emit, current) = (on_event.clone(), is_active.clone());
    signals.connect_duration_changed(move |_, duration| {
        if let Some(duration) = duration.filter(|d| !d.is_zero() && current()) {
            emit(PlayerEvent::Duration(duration.into()));
        }
    });
    let (emit, current) = (on_event.clone(), is_active.clone());
    signals.connect_end_of_stream(move |_| {
        if current() {
            emit(PlayerEvent::Finished);
        }
    });
    let (emit, current) = (on_event.clone(), is_active);
    signals.connect_error(move |_, err, _| {
        // A preload that fails is only not preloaded.
        if current() {
            emit(PlayerEvent::Error(PlaybackError::Stream(err.to_string())));
        }
    });
    Slot {
        play,
        _signals: signals,
        uri: RefCell::default(),
    }
}

/// What only the playback test needs.
#[cfg(test)]
impl Player {
    fn stop(&self) {
        self.playing().play.stop();
    }

    /// The position now, if the stream has one.
    fn position(&self) -> Option<Duration> {
        self.playing().play.position().map(Duration::from)
    }

    /// Which slot plays.
    fn active_slot(&self) -> usize {
        self.active.load(Ordering::SeqCst) % 2
    }
}

fn clock_time(duration: Duration) -> gst::ClockTime {
    gst::ClockTime::from_nseconds(u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests;
