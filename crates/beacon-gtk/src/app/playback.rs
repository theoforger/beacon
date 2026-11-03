// SPDX-License-Identifier: GPL-3.0-or-later

//! Playing episodes: the player's events, saving where playback got to, and
//! the volume and speed remembered in settings.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use beacon_core::PlaybackError;
use beacon_core::model::{Episode, EpisodeId};
use beacon_core::playback;
use beacon_core::services::library::Library;
use relm4::ComponentSender;
use relm4::gtk::{gio, glib, prelude::*};

use super::library::Parts;
use super::{App, AppMsg, AppWidgets, toast};
use crate::player::{PlaybackState, Player, PlayerEvent};
use crate::ui::errors;

/// The episode in the player.
#[derive(Debug, Clone)]
pub struct NowPlaying {
    pub id: EpisodeId,
    pub duration: Option<Duration>,
    pub position: Duration,
    /// The position last written to the store.
    pub saved: Duration,
    pub state: PlaybackState,
}

/// Where playback last got to, shared with the window's close handler so it
/// is saved even when the window closes mid-episode.
pub type LastPosition = Rc<Cell<Option<(EpisodeId, Duration)>>>;

/// Start the player, its events going to the app, with the volume and speed
/// last chosen. Without an audio system, playback is unavailable and says so
/// when tried.
pub(super) fn start_player(
    settings: &gio::Settings,
    sender: &ComponentSender<App>,
) -> Result<Player, PlaybackError> {
    let input = sender.input_sender().clone();
    let player = Player::new(&glib::MainContext::default(), move |event| {
        input.emit(AppMsg::Player(event));
    })?;
    player.set_volume(settings.double("volume"));
    player.set_rate(playback::nearest_rate(settings.double("playback-rate")));
    Ok(player)
}

/// Write where playback got to, when the window closes. The window is going
/// away, so this one write happens on the main thread.
pub(super) fn save_on_close(library: Option<&Arc<Library>>, last: &LastPosition) {
    let (Some(library), Some((id, position))) = (library, last.get()) else {
        return;
    };
    if let Err(err) = library.with_store(|store| store.set_position(id, position)) {
        tracing::warn!("could not save where playback got to: {err}");
    }
}

/// The address to play an episode from: its download if it has one, or its
/// enclosure.
fn source(episode: &Episode) -> Option<String> {
    if let Some(path) = &episode.state.download_path
        && path.exists()
    {
        return glib::filename_to_uri(path, None).ok().map(String::from);
    }
    episode.info.enclosure.as_ref().map(|e| e.url.to_string())
}

impl App {
    /// The row's Play button: start this episode where it was left, or
    /// pause or resume it if it is the one playing.
    pub(super) fn play_episode(
        &mut self,
        episode: &Episode,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
    ) {
        let Some(player) = &self.player else {
            let err = self
                .player_error
                .clone()
                .unwrap_or(PlaybackError::Unavailable(String::new()));
            toast(&widgets.toasts, &errors::playback(&err));
            return;
        };
        if let Some(now) = &self.now_playing
            && now.id == episode.id
        {
            match now.state {
                PlaybackState::Playing | PlaybackState::Buffering => player.pause(),
                PlaybackState::Paused | PlaybackState::Stopped => player.play(),
            }
            return;
        }
        let Some(uri) = source(episode) else {
            toast(&widgets.toasts, &errors::playback(&PlaybackError::NoAudio));
            return;
        };
        let duration = episode.info.duration;
        let start = playback::resume_position(&episode.state, duration);
        player.load(&uri, start);
        // The episode it replaces keeps where it got to.
        self.save_position(sender, Parts::EPISODE);
        self.now_playing = Some(NowPlaying {
            id: episode.id,
            duration,
            position: start,
            saved: start,
            state: PlaybackState::Buffering,
        });
        self.last_position.set(Some((episode.id, start)));
    }

    pub(super) fn player_event(
        &mut self,
        event: PlayerEvent,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
    ) {
        match event {
            PlayerEvent::Position(position) => {
                let Some(now) = &mut self.now_playing else {
                    return;
                };
                now.position = position;
                self.last_position.set(Some((now.id, position)));
                if playback::should_save(now.saved, position) {
                    self.save_position(sender, Parts::NONE);
                }
            }
            PlayerEvent::Duration(duration) => {
                if let Some(now) = &mut self.now_playing {
                    now.duration = Some(duration);
                }
            }
            PlayerEvent::State(state) => {
                let Some(now) = &mut self.now_playing else {
                    return;
                };
                now.state = state;
                if matches!(state, PlaybackState::Paused | PlaybackState::Stopped) {
                    // Progress bars show where it stopped.
                    self.save_position(sender, Parts::EPISODE);
                }
            }
            PlayerEvent::Finished => {
                let Some(now) = self.now_playing.take() else {
                    return;
                };
                self.last_position.set(None);
                let Some(when) = self.library.as_ref().map(|l| l.now()) else {
                    return;
                };
                self.job(sender, Parts::EPISODE, false, move |store| {
                    store.finish(now.id, when)
                });
            }
            PlayerEvent::Error(err) => {
                tracing::warn!("{err}");
                toast(&widgets.toasts, &errors::playback(&err));
            }
        }
    }

    /// Write the playing episode's position in the background, reloading
    /// `parts` after.
    fn save_position(&mut self, sender: &ComponentSender<Self>, parts: Parts) {
        let Some(now) = &mut self.now_playing else {
            return;
        };
        now.saved = now.position;
        let (id, position) = (now.id, now.position);
        self.job(sender, parts, false, move |store| {
            store.set_position(id, position)
        });
    }

    /// Remember the volume and speed for next time.
    #[expect(dead_code, reason = "the player bar's volume and speed, next in M4")]
    pub(super) fn remember_playback_settings(&self, volume: f64, rate: f64) {
        for (key, value) in [("volume", volume), ("playback-rate", rate)] {
            if let Err(err) = self.settings.set_double(key, value) {
                tracing::warn!("could not remember {key}: {err}");
            }
        }
    }
}
