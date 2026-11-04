// SPDX-License-Identifier: GPL-3.0-or-later

//! Playback through GStreamer's `GstPlay`. The player reports what happens
//! as typed events on the main loop; the rules (where to resume, how far to
//! skip, when to save) live in `beacon_core::playback`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use beacon_core::PlaybackError;
use gstreamer as gst;
#[cfg(test)]
use gstreamer::prelude::*;
use gstreamer_play as gst_play;
use relm4::gtk::glib;

/// How often the player reports the position while playing.
const POSITION_INTERVAL_MS: u32 = 250;

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
    /// The episode played to its end.
    Finished,
    Error(PlaybackError),
}

type OnEvent = Arc<dyn Fn(PlayerEvent) + Send + Sync>;

/// One episode at a time, through `GstPlay`.
#[derive(Debug)]
pub struct Player {
    play: gst_play::Play,
    _signals: gst_play::PlaySignalAdapter,
    /// Where to seek once the stream is ready: seeking before it has
    /// prerolled is not reliable.
    start: Arc<Mutex<Option<gst::ClockTime>>>,
}

impl Player {
    /// A player whose events arrive on `context`, playing to the default
    /// audio output.
    pub fn new(
        context: &glib::MainContext,
        on_event: impl Fn(PlayerEvent) + Send + Sync + 'static,
    ) -> Result<Self, PlaybackError> {
        gst::init().map_err(|err| PlaybackError::Unavailable(err.to_string()))?;
        Ok(Self::build(context, Arc::new(on_event)))
    }

    /// A player that plays to `sink` instead of the audio output (tests).
    #[cfg(test)]
    fn with_audio_sink(
        context: &glib::MainContext,
        sink: &gst::Element,
        on_event: impl Fn(PlayerEvent) + Send + Sync + 'static,
    ) -> Self {
        let player = Self::build(context, Arc::new(on_event));
        player.play.pipeline().set_property("audio-sink", sink);
        player
    }

    fn build(context: &glib::MainContext, on_event: OnEvent) -> Self {
        let play = gst_play::Play::new(None::<gst_play::PlayVideoRenderer>);
        let mut config = play.config();
        config.set_position_update_interval(POSITION_INTERVAL_MS);
        // Rarely fails, and only for a bad config; the default interval
        // then applies.
        if let Err(err) = play.set_config(config) {
            tracing::warn!("could not configure the player: {err}");
        }
        let signals = gst_play::PlaySignalAdapter::with_main_context(&play, context);
        let start: Arc<Mutex<Option<gst::ClockTime>>> = Arc::default();

        let emit = on_event.clone();
        let seek = (play.clone(), start.clone());
        signals.connect_state_changed(move |_, state| {
            let state = match state {
                gst_play::PlayState::Buffering => PlaybackState::Buffering,
                gst_play::PlayState::Paused => PlaybackState::Paused,
                gst_play::PlayState::Playing => PlaybackState::Playing,
                _ => PlaybackState::Stopped,
            };
            if matches!(state, PlaybackState::Paused | PlaybackState::Playing)
                && let Some(position) = seek.1.lock().ok().and_then(|mut s| s.take())
            {
                seek.0.seek(position);
            }
            emit(PlayerEvent::State(state));
        });
        let emit = on_event.clone();
        signals.connect_position_updated(move |_, position| {
            if let Some(position) = position {
                emit(PlayerEvent::Position(position.into()));
            }
        });
        let emit = on_event.clone();
        signals.connect_duration_changed(move |_, duration| {
            if let Some(duration) = duration.filter(|d| !d.is_zero()) {
                emit(PlayerEvent::Duration(duration.into()));
            }
        });
        let emit = on_event.clone();
        signals.connect_end_of_stream(move |_| emit(PlayerEvent::Finished));
        let emit = on_event;
        signals.connect_error(move |_, err, _| {
            emit(PlayerEvent::Error(PlaybackError::Stream(err.to_string())));
        });

        Self {
            play,
            _signals: signals,
            start,
        }
    }

    /// Open `uri` and start playing it from `start`.
    pub fn load(&self, uri: &str, start: Duration) {
        self.play.stop();
        if let Ok(mut pending) = self.start.lock() {
            *pending = (!start.is_zero()).then(|| clock_time(start));
        }
        self.play.set_uri(Some(uri));
        // Podcasts are listened to; a video enclosure plays its sound only.
        self.play.set_video_track_enabled(false);
        self.play.play();
    }

    pub fn play(&self) {
        self.play.play();
    }

    pub fn pause(&self) {
        self.play.pause();
    }

    pub fn seek(&self, position: Duration) {
        self.play.seek(clock_time(position));
    }

    pub fn set_rate(&self, rate: f64) {
        self.play.set_rate(rate);
    }

    /// Beacon's own stream volume, from 0 to 1.
    pub fn set_volume(&self, volume: f64) {
        self.play.set_volume(volume.clamp(0.0, 1.0));
    }
}

/// What only the playback test needs.
#[cfg(test)]
impl Player {
    fn stop(&self) {
        self.play.stop();
    }

    /// The position now, if the stream has one.
    fn position(&self) -> Option<Duration> {
        self.play.position().map(Duration::from)
    }
}

fn clock_time(duration: Duration) -> gst::ClockTime {
    gst::ClockTime::from_nseconds(u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests;
