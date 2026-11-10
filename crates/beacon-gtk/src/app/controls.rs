// SPDX-License-Identifier: GPL-3.0-or-later

//! What the player bar asks for: play and pause, the skips and seeking,
//! speed and volume, the sleep timer and its fade, Favourite, Next Episode,
//! and opening the playing episode's details.

use std::time::{Duration, Instant};

use beacon_core::model::EpisodeId;
use beacon_core::playback::{self, SleepTimer};
use gtk::{gio::prelude::*, glib};

use super::library::Parts;
use super::{App, AppCommand, AppMsg};
use crate::player::PlaybackState;
use crate::ui::details_pane::{DetailsPaneInput, DetailsPaneOutput};
use crate::ui::player_bar::{PlayerBarInput, PlayerBarOutput};
use crate::ui::queue_data;

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
    pub(super) fn bar_output(&mut self, output: PlayerBarOutput) {
        let player = self.player.clone();
        match output {
            PlayerBarOutput::PlayPause => {
                if self.open_unopened() {
                    return;
                }
                let (Some(player), Some(state)) =
                    (&player, self.now_playing.as_ref().map(|now| now.state))
                else {
                    return;
                };
                match state {
                    PlaybackState::Playing | PlaybackState::Buffering => player.pause(),
                    PlaybackState::Paused | PlaybackState::Stopped => {
                        self.release_hold();
                        player.play();
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
            PlayerBarOutput::Next => {
                let current = self.now_playing.as_ref().map(|now| now.id).or(self.after);
                self.play_next(current);
            }
            PlayerBarOutput::Rate(rate) => {
                let rate = playback::nearest_rate(rate);
                if let Some(player) = &player {
                    player.set_rate(rate);
                }
                self.bar.send(PlayerBarInput::Rate(rate));
                self.remember("playback-rate", rate);
            }
            PlayerBarOutput::Volume(volume) => {
                let volume = volume.clamp(0.0, 1.0);
                if let Some(player) = &player
                    && !self.sleep.fading
                {
                    player.set_volume(volume);
                }
                self.bar.send(PlayerBarInput::Volume(volume));
                self.remember("volume", volume);
            }
            PlayerBarOutput::SleepTimer(timer) => {
                self.sleep = Sleep {
                    timer,
                    since: timer.length().map(|_| Instant::now()),
                    fading: false,
                };
                self.bar.send(PlayerBarInput::SleepTimer(timer));
            }
            PlayerBarOutput::OpenDetails => {
                let Some(now) = &self.now_playing else {
                    return;
                };
                let (id, show) = (now.id, now.show_id);
                // From the narrow layout's sheet: the details replace it.
                self.sheet.set_open(false);
                self.details_episode = Some((id, show));
                self.focus_details();
                self.widgets.details_split.set_show_sidebar(true);
                self.reload(Parts {
                    details: true,
                    ..Parts::NONE
                });
                self.load_details(id);
            }
        }
    }

    pub(super) fn seek(&mut self, to: Duration) {
        let Some(now) = &mut self.now_playing else {
            return;
        };
        now.position = to;
        // The restored episode, not yet opened, starts from there.
        self.last_position.set(Some((now.id, to)));
        if let Some(player) = self.player.as_ref().filter(|_| !now.is_unopened()) {
            player.seek(to);
        }
        self.bar.send(PlayerBarInput::Position(to));
        self.media.seeked(to);
    }

    /// Next Episode, and auto-continue when an episode ends: the queue's
    /// first, else the next in the list `current` was started from.
    pub(super) fn play_next(&self, current: Option<EpisodeId>) {
        let (Some(library), Some(lists)) = (self.library.clone(), self.lists()) else {
            return;
        };
        let source = self.source;
        self.sender.spawn_blocking(move || {
            let next =
                library.with_store(|store| queue_data::next_item(store, current, source, lists));
            AppCommand::PlayNext(next.ok().flatten().map(Box::new))
        });
    }

    /// Fade out and pause once a timed sleep timer runs out.
    pub(super) fn check_sleep_timer(&mut self) {
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
        let input = self.sender.clone();
        glib::timeout_add_local(FADE_STEP, move || {
            let elapsed = started.elapsed();
            if elapsed >= playback::FADE {
                player.pause();
                // The next play starts at the volume chosen, not silent.
                player.set_volume(volume);
                input.input(AppMsg::SleepTimerDone);
                return glib::ControlFlow::Break;
            }
            player.set_volume(playback::fade_volume(volume, elapsed));
            glib::ControlFlow::Continue
        });
    }

    /// The sleep timer has done its work, or the episode ended.
    /// Whether the sleep timer stops playback when this episode ends.
    pub(super) fn sleeps_at_end(&self) -> bool {
        self.sleep.timer == SleepTimer::EndOfEpisode
    }

    pub(super) fn end_sleep_timer(&mut self) {
        if self.sleep.timer == SleepTimer::Off {
            return;
        }
        self.sleep = Sleep::default();
        self.bar.send(PlayerBarInput::SleepTimer(SleepTimer::Off));
    }

    /// What the details pane asks for.
    pub(super) fn details_output(&mut self, output: DetailsPaneOutput) {
        match output {
            DetailsPaneOutput::Close => self.widgets.details_split.set_show_sidebar(false),
            DetailsPaneOutput::Action(item, action) => {
                let source = self.details_source;
                self.row_action(*item, action, source);
            }
            DetailsPaneOutput::OpenShow(show) => {
                // The full-page details of the narrow layout would hide it.
                if self.widgets.details_split.is_collapsed() {
                    self.widgets.details_split.set_show_sidebar(false);
                }
                self.widgets.split_view.set_show_content(true);
                self.open_show(show);
            }
            DetailsPaneOutput::PlayFrom(item, start) => {
                self.take_source(item.episode.id, self.details_source);
                self.play_episode_from(&item, Some(start));
            }
        }
    }

    /// The episode's own cover arrived: the bar and the details pane show it.
    pub(super) fn episode_cover_arrived(
        &self,
        id: beacon_core::model::EpisodeId,
        path: std::path::PathBuf,
    ) {
        self.bar.send(PlayerBarInput::Cover(id, path.clone()));
        self.media.set_cover(id, path.clone());
        self.details.send(DetailsPaneInput::EpisodeCover(id, path));
    }
}
