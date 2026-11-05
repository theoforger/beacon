// SPDX-License-Identifier: GPL-3.0-or-later

//! Playing episodes: the player's events, saving where playback got to, and
//! the volume and speed remembered in settings.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use beacon_core::PlaybackError;
use beacon_core::model::{Episode, EpisodeId, ShowId};
use beacon_core::playback;
use beacon_core::services::library::Library;
use relm4::gtk::{gio, glib, prelude::*};
use relm4::{ComponentController, ComponentSender};

use super::library::Parts;
use super::{App, AppCommand, AppMsg, AppWidgets, toast};
use crate::player::{PlaybackState, Player, PlayerEvent};
use crate::ui::details_pane::DetailsPaneInput;
use crate::ui::errors;
use crate::ui::models::EpisodeItem;
use crate::ui::player_bar::{NowPlayingCard, PlayerBarInput};
use crate::ui::widgets::episode_row;

/// The episode in the player.
#[derive(Debug, Clone)]
pub struct NowPlaying {
    pub id: EpisodeId,
    pub show_id: ShowId,
    pub favorite: bool,
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
) -> Result<Rc<Player>, PlaybackError> {
    let input = sender.input_sender().clone();
    let player = Player::new(&glib::MainContext::default(), move |event| {
        input.emit(AppMsg::Player(event));
    })?;
    player.set_volume(settings.double("volume"));
    player.set_rate(playback::nearest_rate(settings.double("playback-rate")));
    Ok(Rc::new(player))
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
        item: &EpisodeItem,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
    ) {
        self.play_episode_from(item, None, widgets, sender);
    }

    /// Play `item` from `start` (a chapter), or from where it was left; if
    /// it is the one playing, seek there, or without `start` pause or
    /// resume it.
    pub(super) fn play_episode_from(
        &mut self,
        item: &EpisodeItem,
        start: Option<Duration>,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
    ) {
        let episode = &item.episode;
        let Some(player) = self.player.clone() else {
            let err = self
                .player_error
                .clone()
                .unwrap_or(PlaybackError::Unavailable(String::new()));
            toast(&widgets.toasts, &errors::playback(&err));
            return;
        };
        if let Some(state) = self
            .now_playing
            .as_ref()
            .filter(|now| now.id == episode.id)
            .map(|now| now.state)
        {
            let playing = matches!(state, PlaybackState::Playing | PlaybackState::Buffering);
            if let Some(start) = start {
                self.seek(start);
                if !playing {
                    player.play();
                }
                return;
            }
            match state {
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
        let start = start.unwrap_or_else(|| playback::resume_position(&episode.state, duration));
        player.load(&uri, start);
        // The episode it replaces keeps where it got to.
        self.save_position(sender, Parts::PROGRESS);
        self.now_playing = Some(NowPlaying {
            id: episode.id,
            show_id: episode.show_id,
            favorite: episode.state.favorite,
            duration,
            position: start,
            saved: start,
            state: PlaybackState::Buffering,
        });
        self.last_position.set(Some((episode.id, start)));
        self.bar
            .emit(PlayerBarInput::Episode(Some(Box::new(NowPlayingCard {
                id: episode.id,
                title: episode.info.title.clone(),
                show_title: item.show_title.clone(),
                cover: item.cover.clone(),
                favorite: episode.state.favorite,
                duration,
            }))));
        self.bar.emit(PlayerBarInput::Position(start));
        self.fetch_episode_cover(episode.id, sender);
        // Playing takes an episode off the queue: the queue is what comes
        // next, as in a music app (the Now Playing group, M5).
        let id = episode.id;
        self.job(sender, Parts::EPISODE, false, move |store| {
            store.dequeue(id)
        });
    }

    /// The episode in the player, whether it plays, and where it is.
    pub(super) fn playback_state(&self) -> Option<(EpisodeId, bool, Duration)> {
        self.now_playing.as_ref().map(|now| {
            let playing = matches!(now.state, PlaybackState::Playing | PlaybackState::Buffering);
            (now.id, playing, now.position)
        })
    }

    /// Everything the details pane loads for an episode beyond what its row
    /// knows: its own cover and its chapters, and where playback is.
    pub(super) fn load_details(&self, id: EpisodeId, sender: &ComponentSender<Self>) {
        self.fetch_episode_cover(id, sender);
        self.details
            .emit(DetailsPaneInput::Playback(self.playback_state()));
        if let Some(library) = self.library.clone() {
            sender.oneshot_command(async move {
                let chapters = library.chapters(id).await.unwrap_or_else(|err| {
                    tracing::warn!("could not load the chapters of {id:?}: {err}");
                    Vec::new()
                });
                AppCommand::Chapters(id, chapters)
            });
        }
    }

    /// Fetch the episode's own cover for the player bar and the details
    /// pane; the show's stands in until it comes, or if it never does.
    pub(super) fn fetch_episode_cover(&self, id: EpisodeId, sender: &ComponentSender<Self>) {
        if let Some(library) = self.library.clone() {
            sender.oneshot_command(async move {
                AppCommand::EpisodeCover(id, library.episode_cover(id).await.ok().flatten())
            });
        }
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
                let due = playback::should_save(now.saved, position);
                self.last_position.set(Some((now.id, position)));
                self.bar.emit(PlayerBarInput::Position(position));
                self.details
                    .emit(DetailsPaneInput::Playback(self.playback_state()));
                if due {
                    self.save_position(sender, Parts::NONE);
                }
                self.check_sleep_timer(sender);
            }
            PlayerEvent::Duration(duration) => {
                if let Some(now) = &mut self.now_playing {
                    now.duration = Some(duration);
                }
                self.bar.emit(PlayerBarInput::Duration(duration));
            }
            PlayerEvent::State(state) => {
                let Some(now) = &mut self.now_playing else {
                    return;
                };
                now.state = state;
                let playing = matches!(state, PlaybackState::Playing | PlaybackState::Buffering);
                episode_row::show_playing(Some((now.id, playing)));
                self.bar.emit(PlayerBarInput::State(state));
                self.details
                    .emit(DetailsPaneInput::Playback(self.playback_state()));
                if matches!(state, PlaybackState::Paused | PlaybackState::Stopped) {
                    // Progress bars show where it stopped.
                    self.save_position(sender, Parts::PROGRESS);
                }
            }
            PlayerEvent::Finished => {
                let Some(now) = self.now_playing.take() else {
                    return;
                };
                self.last_position.set(None);
                episode_row::show_playing(None);
                self.bar.emit(PlayerBarInput::State(PlaybackState::Stopped));
                self.details.emit(DetailsPaneInput::Playback(None));
                // "End of Episode" has done its work; nothing plays next yet
                // (auto-continue arrives with the queue, M5).
                self.end_sleep_timer();
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

    /// Remember a playback setting (`volume`, `playback-rate`) for next
    /// time.
    pub(super) fn remember(&self, key: &str, value: f64) {
        if let Err(err) = self.settings.set_double(key, value) {
            tracing::warn!("could not remember {key}: {err}");
        }
    }
}
