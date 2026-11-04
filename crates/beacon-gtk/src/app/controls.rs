// SPDX-License-Identifier: GPL-3.0-or-later

//! What the player bar asks for: play and pause, the skips and seeking,
//! speed and volume, the sleep timer and its fade, Favourite, Next Episode,
//! and opening the playing episode's details.

use std::time::{Duration, Instant};

use beacon_core::playback::{self, SleepTimer};
use relm4::gtk::{gio::prelude::*, glib};
use relm4::{ComponentController, ComponentSender};

use super::library::Parts;
use super::{App, AppCommand, AppMsg, AppWidgets};
use crate::player::PlaybackState;
use crate::ui::details_pane::DetailsPaneInput;
use crate::ui::models;
use crate::ui::player_bar::{PlayerBarInput, PlayerBarOutput};

/// How often the fade lowers the volume.
const FADE_STEP: Duration = Duration::from_millis(100);

/// The sleep timer as chosen, and when a timed one started.
#[derive(Debug, Default)]
pub struct Sleep {
    timer: SleepTimer,
    since: Option<Instant>,
    fading: bool,
}

impl App {
    pub(super) fn bar_output(
        &mut self,
        output: PlayerBarOutput,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
    ) {
        let player = self.player.clone();
        match output {
            PlayerBarOutput::PlayPause => {
                if let (Some(player), Some(now)) = (&player, &self.now_playing) {
                    match now.state {
                        PlaybackState::Playing | PlaybackState::Buffering => player.pause(),
                        PlaybackState::Paused | PlaybackState::Stopped => player.play(),
                    }
                }
            }
            PlayerBarOutput::SkipBack => {
                if let Some(now) = &self.now_playing {
                    let to = playback::skip_back(now.position);
                    self.seek(to);
                }
            }
            PlayerBarOutput::SkipForward => {
                if let Some(now) = &self.now_playing {
                    let to = playback::skip_forward(now.position, now.duration);
                    self.seek(to);
                }
            }
            PlayerBarOutput::Seek(to) => self.seek(to),
            PlayerBarOutput::Next => self.play_next(sender),
            PlayerBarOutput::Rate(rate) => {
                let rate = playback::nearest_rate(rate);
                if let Some(player) = &player {
                    player.set_rate(rate);
                }
                self.bar.emit(PlayerBarInput::Rate(rate));
                self.remember("playback-rate", rate);
            }
            PlayerBarOutput::Volume(volume) => {
                let volume = volume.clamp(0.0, 1.0);
                if let Some(player) = &player
                    && !self.sleep.fading
                {
                    player.set_volume(volume);
                }
                self.bar.emit(PlayerBarInput::Volume(volume));
                self.remember("volume", volume);
            }
            PlayerBarOutput::SleepTimer(timer) => {
                self.sleep = Sleep {
                    timer,
                    since: timer.length().map(|_| Instant::now()),
                    fading: false,
                };
                self.bar.emit(PlayerBarInput::SleepTimer(timer));
            }
            PlayerBarOutput::ToggleFavorite => {
                let Some(now) = &mut self.now_playing else {
                    return;
                };
                now.favorite = !now.favorite;
                let (id, favorite) = (now.id, now.favorite);
                self.bar.emit(PlayerBarInput::Favorite(favorite));
                self.job(sender, Parts::EPISODE, false, move |store| {
                    store.set_favorite(id, favorite)
                });
            }
            PlayerBarOutput::OpenDetails => {
                let Some(now) = &self.now_playing else {
                    return;
                };
                let (id, show) = (now.id, now.show_id);
                self.details_episode = Some((id, show));
                self.focus_details(widgets);
                widgets.details_split.set_show_sidebar(true);
                self.reload(
                    sender,
                    Parts {
                        details: true,
                        ..Parts::NONE
                    },
                );
                self.fetch_episode_cover(id, sender);
            }
        }
    }

    fn seek(&mut self, to: Duration) {
        let Some(now) = &mut self.now_playing else {
            return;
        };
        now.position = to;
        if let Some(player) = &self.player {
            player.seek(to);
        }
        self.bar.emit(PlayerBarInput::Position(to));
    }

    /// Next Episode: the first episode in the queue. Until auto-continue
    /// (M5), that is all that comes next.
    fn play_next(&self, sender: &ComponentSender<Self>) {
        let Some(library) = self.library.clone() else {
            return;
        };
        let current = self.now_playing.as_ref().map(|now| now.id);
        sender.spawn_oneshot_command(move || {
            let next = library.with_store(|store| {
                let queue = store.queue()?;
                match queue.into_iter().find(|id| Some(*id) != current) {
                    Some(id) => models::episode_item(store, id),
                    None => Ok(None),
                }
            });
            AppCommand::PlayNext(next.ok().flatten().map(Box::new))
        });
    }

    /// Whether Next Episode has anything to play.
    pub(super) fn set_queue_length(&self, length: u32) {
        self.bar.emit(PlayerBarInput::HasNext(length > 0));
    }

    /// Fade out and pause once a timed sleep timer runs out.
    pub(super) fn check_sleep_timer(&mut self, sender: &ComponentSender<Self>) {
        let due = match (self.sleep.timer.length(), self.sleep.since) {
            (Some(length), Some(since)) => since.elapsed() >= length,
            _ => false,
        };
        if !due || self.sleep.fading {
            return;
        }
        let Some(player) = self.player.clone() else {
            return;
        };
        self.sleep.fading = true;
        let volume = self.settings.double("volume");
        let started = Instant::now();
        let input = sender.input_sender().clone();
        glib::timeout_add_local(FADE_STEP, move || {
            let elapsed = started.elapsed();
            if elapsed >= playback::FADE {
                player.pause();
                // The next play starts at the volume chosen, not silent.
                player.set_volume(volume);
                input.emit(AppMsg::SleepTimerDone);
                return glib::ControlFlow::Break;
            }
            player.set_volume(playback::fade_volume(volume, elapsed));
            glib::ControlFlow::Continue
        });
    }

    /// The sleep timer has done its work, or the episode ended.
    pub(super) fn end_sleep_timer(&mut self) {
        if self.sleep.timer == SleepTimer::Off {
            return;
        }
        self.sleep = Sleep::default();
        self.bar.emit(PlayerBarInput::SleepTimer(SleepTimer::Off));
    }

    /// The episode's own cover arrived: the bar and the details pane show it.
    pub(super) fn episode_cover_arrived(
        &self,
        id: beacon_core::model::EpisodeId,
        path: std::path::PathBuf,
    ) {
        self.bar.emit(PlayerBarInput::Cover(id, path.clone()));
        self.details.emit(DetailsPaneInput::EpisodeCover(id, path));
    }
}
