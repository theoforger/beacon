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
use gtk::{gio, glib, prelude::*};

use super::library::Parts;
use super::sender::AppSender;
use super::{App, AppCommand, AppMsg, toast};
use crate::player::mpris::Track;
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
    pub duration: Option<Duration>,
    pub position: Duration,
    /// The position last written to the store.
    pub saved: Duration,
    /// What the app shows the player doing.
    pub state: PlaybackState,
    /// What the player last said it was doing, which a hold can hide.
    pub reported: PlaybackState,
    /// Whether the player holds all of it: the next one is preloaded only
    /// then, so the two do not compete for the connection.
    pub buffered: bool,
    /// Opened paused at launch to buffer, and not yet played: the player's
    /// buffering shows as paused, and its errors wait until it is played.
    pub held: bool,
    /// The episode last played, put back in the bar at launch, paused
    /// where it was left (asked by the user 2026-11-09); the player opens
    /// it only when it is played, so a launch costs no data and an
    /// offline one shows no error.
    pub unopened: Option<Box<EpisodeItem>>,
}

impl NowPlaying {
    /// The restored episode, not yet opened in the player.
    pub fn is_unopened(&self) -> bool {
        self.unopened.is_some()
    }

    /// The player says it is `state`: what to show. Buffering at launch is
    /// not playing, so while held it shows as paused.
    pub fn report(&mut self, state: PlaybackState) -> PlaybackState {
        self.reported = state;
        self.held &= state != PlaybackState::Playing;
        self.state = match state {
            PlaybackState::Buffering if self.held => PlaybackState::Paused,
            state => state,
        };
        self.state
    }

    /// The episode is played: end the hold, and return what the hold hid
    /// if it differs from what shows. A player still buffering reports
    /// nothing new when played, so its loading would otherwise not show.
    pub fn release(&mut self) -> Option<PlaybackState> {
        let held = std::mem::take(&mut self.held);
        (held && self.reported != self.state).then(|| {
            self.state = self.reported;
            self.state
        })
    }
}

/// Where playback last got to, shared with the window's close handler so it
/// is saved even when the window closes mid-episode.
pub type LastPosition = Rc<Cell<Option<(EpisodeId, Duration)>>>;

/// Start the player, its events going to the app, with the volume and speed
/// last chosen. Without an audio system, playback is unavailable and says so
/// when tried.
pub(super) fn start_player(
    settings: &gio::Settings,
    sender: &AppSender,
) -> Result<Rc<Player>, PlaybackError> {
    let input = sender.clone();
    let buffers = glib::user_cache_dir().join("beacon").join("buffer");
    let player = Player::new(&glib::MainContext::default(), &buffers, move |event| {
        input.input(AppMsg::Player(event));
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

/// Whether streaming ahead of time is wanted: on a connection that is up
/// and not metered, where it would spend data on what may not be played.
pub(super) fn may_prefetch() -> bool {
    let monitor = gio::NetworkMonitor::default();
    monitor.is_network_available() && !monitor.is_network_metered()
}

/// Tell the app when the connection changes, so what is fetched ahead
/// follows whether it is metered.
pub(super) fn watch_network(sender: &AppSender) {
    let monitor = gio::NetworkMonitor::default();
    let input = sender.clone();
    monitor.connect_network_metered_notify(move |_| input.input(AppMsg::NetworkChanged));
    let input = sender.clone();
    monitor.connect_network_changed(move |_, _| input.input(AppMsg::NetworkChanged));
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

/// Where the episode last played comes back in the bar at launch: where it
/// was left. Not if it was played to the end, or has no audio.
fn restored_position(episode: &Episode) -> Option<Duration> {
    if episode.state.played || source(episode).is_none() {
        return None;
    }
    Some(playback::resume_position(
        &episode.state,
        episode.info.duration,
    ))
}

impl App {
    /// Open the episode that plays next in the background, once the playing
    /// one has finished buffering, so it starts at once (asked by the user
    /// 2026-11-08). Not on a metered connection, where it would spend data
    /// on an episode that may never be played.
    pub(super) fn preload_next(&self) {
        let Some(player) = &self.player else {
            return;
        };
        let buffered = self.now_playing.as_ref().is_some_and(|now| now.buffered);
        let may = may_prefetch();
        let uri = self
            .up_next
            .as_ref()
            .filter(|next| buffered && may && Some(next.id) != self.playing_id())
            .and_then(source);
        tracing::debug!(buffered, may, uri = ?uri, "preload");
        player.preload(uri.as_deref());
    }

    fn playing_id(&self) -> Option<EpisodeId> {
        self.now_playing.as_ref().map(|now| now.id)
    }

    /// The row's Play button: start this episode where it was left, or
    /// pause or resume it if it is the one playing.
    pub(super) fn play_episode(&mut self, item: &EpisodeItem) {
        self.play_episode_from(item, None);
    }

    /// Play `item` from `start` (a chapter), or from where it was left; if
    /// it is the one playing, seek there, or without `start` pause or
    /// resume it.
    pub(super) fn play_episode_from(&mut self, item: &EpisodeItem, start: Option<Duration>) {
        let episode = &item.episode;
        let Some(player) = self.player.clone() else {
            let err = self
                .player_error
                .clone()
                .unwrap_or(PlaybackError::Unavailable(String::new()));
            toast(&self.widgets.toasts, &errors::playback(&err));
            return;
        };
        // The restored episode opens where it was left, or was moved to.
        let unopened = self
            .now_playing
            .as_ref()
            .filter(|now| now.id == episode.id && now.is_unopened())
            .map(|now| now.position);
        let start = start.or(unopened);
        if let Some(state) = self
            .now_playing
            .as_ref()
            .filter(|now| now.id == episode.id && !now.is_unopened())
            .map(|now| now.state)
        {
            self.release_hold();
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
            toast(
                &self.widgets.toasts,
                &errors::playback(&PlaybackError::NoAudio),
            );
            return;
        };
        let duration = episode.info.duration;
        let start = start.unwrap_or_else(|| playback::resume_position(&episode.state, duration));
        player.load(&uri, start);
        // The episode it replaces keeps where it got to.
        self.save_position(Parts::PROGRESS);
        self.show_in_player(item, start, PlaybackState::Buffering, None);
        self.after = Some(episode.id);
        let _ = self.settings.set_int64("last-episode", episode.id.0);
        // Playing takes an episode off the queue: the queue is what comes
        // next, as in a music app. It also clears the blue dot, as viewing
        // its details does (the mockup's `playEp`).
        let id = episode.id;
        self.job(Parts::EPISODE, false, move |store| {
            store.dequeue(id)?;
            store.mark_seen(id)
        });
    }

    /// Put `item` in the player bar, the desktop's media controls and the
    /// rows, at `position`.
    fn show_in_player(
        &mut self,
        item: &EpisodeItem,
        position: Duration,
        state: PlaybackState,
        unopened: Option<Box<EpisodeItem>>,
    ) {
        let episode = &item.episode;
        let duration = episode.info.duration;
        self.now_playing = Some(NowPlaying {
            id: episode.id,
            show_id: episode.show_id,
            duration,
            position,
            saved: position,
            state,
            reported: state,
            buffered: false,
            held: false,
            unopened,
        });
        self.last_position.set(Some((episode.id, position)));
        self.bar
            .send(PlayerBarInput::Episode(Some(Box::new(NowPlayingCard {
                id: episode.id,
                title: episode.info.title.clone(),
                show_title: item.show_title.clone(),
                cover: item.cover.clone(),
                duration,
            }))));
        self.bar.send(PlayerBarInput::Position(position));
        if let Some(duration) = duration {
            self.bar.send(PlayerBarInput::Duration(duration));
        }
        self.media.set_track(Some(Track {
            id: episode.id,
            title: episode.info.title.clone(),
            show: item.show_title.clone(),
            cover: item.cover.clone(),
            duration,
        }));
        self.media.set_position(position);
        // The narrow layout's slim bar now opens the player.
        self.sheet.set_can_open(true);
        self.fetch_episode_cover(episode.id);
        self.load_chapters(episode.id);
    }

    /// Read back the episode last played, to put it in the bar at launch.
    pub(super) fn restore_last_episode(&self) {
        let (Some(library), Some(id)) = (self.library.clone(), self.after) else {
            return;
        };
        self.sender.spawn_blocking(move || {
            let item = library
                .with_store(|store| crate::ui::models::episode_item(store, id))
                .unwrap_or_else(|err| {
                    tracing::warn!("could not read the last episode played: {err}");
                    None
                });
            AppCommand::Restore(item.map(Box::new))
        });
    }

    /// The episode last played, back in the bar, paused where it was left,
    /// unless something already plays or it was played to the end.
    pub(super) fn restore(&mut self, item: EpisodeItem) {
        let Some(position) =
            restored_position(&item.episode).filter(|_| self.now_playing.is_none())
        else {
            return;
        };
        let unopened = Some(Box::new(item.clone()));
        self.show_in_player(&item, position, PlaybackState::Paused, unopened);
        let id = item.episode.id;
        episode_row::show_playing(Some((id, PlaybackState::Paused)));
        self.bar.send(PlayerBarInput::State(PlaybackState::Paused));
        self.media.set_state(PlaybackState::Paused);
        self.buffer_restored();
    }

    /// Start buffering the restored episode, paused where it was left, so
    /// Play starts it at once (asked by the user 2026-11-09): straight
    /// away if it is downloaded, else once the connection is not metered.
    pub(super) fn buffer_restored(&mut self) {
        let (Some(player), Some(now)) = (&self.player, &mut self.now_playing) else {
            return;
        };
        let Some(uri) = now.unopened.as_ref().and_then(|item| source(&item.episode)) else {
            return;
        };
        if !uri.starts_with("file:") && !may_prefetch() {
            return;
        }
        tracing::debug!(uri, "buffering the restored episode");
        player.open_paused(&uri, now.position);
        now.unopened = None;
        now.held = true;
    }

    /// The player is asked to play: what it does now is shown as it is.
    /// Call it before asking the player to play, having read the state.
    pub(super) fn release_hold(&mut self) {
        if let Some(state) = self.now_playing.as_mut().and_then(NowPlaying::release) {
            self.show_state(state);
        }
    }

    /// Play the restored episode, if that is what the bar holds. Whether it
    /// did.
    pub(super) fn open_unopened(&mut self) -> bool {
        let Some(item) = self
            .now_playing
            .as_ref()
            .and_then(|now| now.unopened.clone())
        else {
            return false;
        };
        self.play_episode(&item);
        true
    }

    /// The episode in the player, what the player is doing, and where it is.
    pub(super) fn playback_state(&self) -> Option<(EpisodeId, PlaybackState, Duration)> {
        self.now_playing
            .as_ref()
            .map(|now| (now.id, now.state, now.position))
    }

    /// Everything the details pane loads for an episode beyond what its row
    /// knows: its own cover and its chapters, and where playback is.
    pub(super) fn load_details(&self, id: EpisodeId) {
        self.fetch_episode_cover(id);
        self.details
            .send(DetailsPaneInput::Playback(self.playback_state()));
        self.load_chapters(id);
    }

    /// Load an episode's chapters, for the details pane and, if it is the
    /// one playing, the player bar.
    pub(super) fn load_chapters(&self, id: EpisodeId) {
        if let Some(library) = self.library.clone() {
            self.sender.spawn(async move {
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
    pub(super) fn fetch_episode_cover(&self, id: EpisodeId) {
        if let Some(library) = self.library.clone() {
            self.sender.spawn(async move {
                AppCommand::EpisodeCover(id, library.episode_cover(id).await.ok().flatten())
            });
        }
    }

    pub(super) fn player_event(&mut self, event: PlayerEvent) {
        match event {
            PlayerEvent::Position(position) => {
                let Some(now) = &mut self.now_playing else {
                    return;
                };
                now.position = position;
                let due = playback::should_save(now.saved, position);
                self.last_position.set(Some((now.id, position)));
                self.bar.send(PlayerBarInput::Position(position));
                self.media.set_position(position);
                self.details
                    .send(DetailsPaneInput::Playback(self.playback_state()));
                if due {
                    self.save_position(Parts::NONE);
                }
                self.check_sleep_timer();
            }
            PlayerEvent::Duration(duration) => {
                if let Some(now) = &mut self.now_playing {
                    now.duration = Some(duration);
                    self.media
                        .update_track(now.id, |track| track.duration = Some(duration));
                }
                self.bar.send(PlayerBarInput::Duration(duration));
            }
            PlayerEvent::State(state) => {
                let Some(now) = &mut self.now_playing else {
                    return;
                };
                let state = now.report(state);
                self.show_state(state);
                if matches!(state, PlaybackState::Paused | PlaybackState::Stopped) {
                    // Progress bars show where it stopped.
                    self.save_position(Parts::PROGRESS);
                    self.schedule_sync(super::sync::AFTER_PAUSE);
                }
            }
            PlayerEvent::Buffered(share) => {
                let Some(now) = &mut self.now_playing else {
                    return;
                };
                self.bar.send(PlayerBarInput::Buffered(share));
                if share >= 0.999 && !now.buffered {
                    now.buffered = true;
                    self.preload_next();
                }
            }
            PlayerEvent::Finished => {
                let Some(now) = self.now_playing.take() else {
                    return;
                };
                self.last_position.set(None);
                episode_row::show_playing(None);
                self.bar.send(PlayerBarInput::State(PlaybackState::Stopped));
                self.media.set_state(PlaybackState::Stopped);
                self.media.set_track(None);
                self.details.send(DetailsPaneInput::Playback(None));
                // "End of Episode" stops here; otherwise the queue, then the
                // rest of the list, carries on (as the mockup's `advance`).
                let carry_on = !self.sleeps_at_end();
                self.end_sleep_timer();
                let Some(when) = self.library.as_ref().map(|l| l.now()) else {
                    return;
                };
                self.job(Parts::EPISODE, false, move |store| {
                    store.finish(now.id, when)
                });
                if carry_on {
                    self.play_next(Some(now.id));
                }
            }
            PlayerEvent::Error(err) => {
                tracing::warn!("{err}");
                // Buffering at launch failed; Play tries again, and says so.
                if self.now_playing.as_ref().is_some_and(|now| now.held) {
                    return;
                }
                toast(&self.widgets.toasts, &errors::playback(&err));
            }
        }
    }

    /// Show the playing episode as `state` in its row, the player bar, the
    /// desktop's media controls and the details pane.
    fn show_state(&self, state: PlaybackState) {
        let Some(now) = &self.now_playing else {
            return;
        };
        episode_row::show_playing(Some((now.id, state)));
        self.bar.send(PlayerBarInput::State(state));
        self.media.set_state(state);
        self.details
            .send(DetailsPaneInput::Playback(self.playback_state()));
    }

    /// Write the playing episode's position in the background, reloading
    /// `parts` after.
    fn save_position(&mut self, parts: Parts) {
        let Some(now) = &mut self.now_playing else {
            return;
        };
        now.saved = now.position;
        let (id, position) = (now.id, now.position);
        self.job(parts, false, move |store| store.set_position(id, position));
    }

    /// Remember a playback setting (`volume`, `playback-rate`) for next
    /// time.
    pub(super) fn remember(&self, key: &str, value: f64) {
        if let Err(err) = self.settings.set_double(key, value) {
            tracing::warn!("could not remember {key}: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::episode_actions::item;

    #[test]
    fn the_last_episode_comes_back_where_it_was_left() {
        let mut episode = item().episode;
        episode.info.duration = Some(Duration::from_secs(3000));
        episode.state.position = Duration::from_secs(754);
        episode.info.enclosure = Some(beacon_core::model::Enclosure {
            url: "https://example.com/1.mp3".parse().unwrap(),
            mime_type: None,
            length: None,
        });
        assert_eq!(restored_position(&episode), Some(Duration::from_secs(754)));
        episode.state.played = true;
        assert_eq!(restored_position(&episode), None, "played to the end");
        episode.state.played = false;
        episode.info.enclosure = None;
        assert_eq!(restored_position(&episode), None, "nothing to play");
    }

    #[test]
    fn playing_a_held_episode_shows_it_loading() {
        let mut now = NowPlaying {
            id: EpisodeId(1),
            show_id: ShowId(1),
            duration: None,
            position: Duration::ZERO,
            saved: Duration::ZERO,
            state: PlaybackState::Paused,
            reported: PlaybackState::Paused,
            buffered: false,
            held: true,
            unopened: None,
        };
        // Buffering at launch shows as paused.
        assert_eq!(now.report(PlaybackState::Buffering), PlaybackState::Paused);
        // Played while still buffering: the player says nothing new, so
        // ending the hold shows what it is doing (found by the user).
        assert_eq!(now.release(), Some(PlaybackState::Buffering));
        assert_eq!(now.state, PlaybackState::Buffering);
        assert_eq!(now.release(), None, "released once");
        assert_eq!(now.report(PlaybackState::Playing), PlaybackState::Playing);

        // Held and done buffering: nothing to show until it plays.
        now.held = true;
        assert_eq!(now.report(PlaybackState::Paused), PlaybackState::Paused);
        assert_eq!(now.release(), None);
    }
}
