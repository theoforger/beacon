// SPDX-License-Identifier: GPL-3.0-or-later

//! MPRIS, through which media keys, GNOME's media controls and other apps
//! play, pause, seek and skip. Built on `mpris-server` as GNOME Podcasts'
//! `player_mpris.rs` is; the server runs on the main loop and reports what
//! it is asked as `MediaControl`s, and the app tells it what is playing.

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use beacon_core::model::EpisodeId;
use gettextrs::gettext;
use gtk::glib;
use mpris_server::{Metadata, PlaybackStatus, Time, TrackId};

use crate::config::APP_ID;
use crate::player::PlaybackState;

/// What another app asks of the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaControl {
    PlayPause,
    Play,
    Pause,
    Stop,
    Next,
    /// Seek by this many microseconds, back when negative.
    SeekBy(i64),
    /// Seek to this many microseconds into this episode.
    SetPosition(EpisodeId, i64),
    /// Show the window.
    Raise,
}

/// The episode playing, as other apps show it.
#[derive(Debug, Clone)]
pub struct Track {
    pub id: EpisodeId,
    pub title: String,
    pub show: String,
    pub cover: Option<PathBuf>,
    pub duration: Option<Duration>,
}

/// The MPRIS server, while the session bus has it.
pub struct Mpris {
    player: Rc<mpris_server::Player>,
}

impl std::fmt::Debug for Mpris {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mpris").finish_non_exhaustive()
    }
}

/// The D-Bus object path naming an episode, as MPRIS track IDs are.
fn track_id(id: EpisodeId) -> Option<TrackId> {
    TrackId::try_from(format!("/com/theoforger/Beacon/Episode/{}", id.0)).ok()
}

/// The episode a track ID names.
fn episode_of(track: &TrackId) -> Option<EpisodeId> {
    track
        .as_str()
        .strip_prefix("/com/theoforger/Beacon/Episode/")?
        .parse()
        .ok()
        .map(EpisodeId)
}

fn time(duration: Duration) -> Time {
    Time::from_micros(i64::try_from(duration.as_micros()).unwrap_or(i64::MAX))
}

impl Mpris {
    /// Take the name `org.mpris.MediaPlayer2.com.theoforger.Beacon` on the
    /// session bus and answer on it, passing what is asked to `on_control`.
    pub async fn start(on_control: impl Fn(MediaControl) + 'static) -> Result<Self, String> {
        Self::start_as(APP_ID, on_control).await
    }

    /// As `start`, as `org.mpris.MediaPlayer2.{name}`.
    async fn start_as(
        name: &str,
        on_control: impl Fn(MediaControl) + 'static,
    ) -> Result<Self, String> {
        let player = mpris_server::Player::builder(name)
            .identity(gettext("Beacon"))
            .desktop_entry(APP_ID)
            .can_raise(true)
            .can_control(true)
            .build()
            .await
            .map_err(|err| err.to_string())?;
        let on_control: Rc<dyn Fn(MediaControl)> = Rc::new(on_control);
        let send = |control: MediaControl| {
            let on_control = on_control.clone();
            move |_: &mpris_server::Player| on_control(control)
        };
        player.connect_play_pause(send(MediaControl::PlayPause));
        player.connect_play(send(MediaControl::Play));
        player.connect_pause(send(MediaControl::Pause));
        player.connect_stop(send(MediaControl::Stop));
        player.connect_next(send(MediaControl::Next));
        player.connect_raise(send(MediaControl::Raise));
        let seek = on_control.clone();
        player.connect_seek(move |_, offset| seek(MediaControl::SeekBy(offset.as_micros())));
        let set = on_control;
        player.connect_set_position(move |_, track, position| {
            if let Some(id) = episode_of(track) {
                set(MediaControl::SetPosition(id, position.as_micros()));
            }
        });

        let player = Rc::new(player);
        glib::spawn_future_local(player.run());
        Ok(Self { player })
    }

    /// Run `change` on the server, logging what fails: other apps then show
    /// stale state, nothing the listener needs to hear about.
    fn update<F>(&self, change: impl FnOnce(Rc<mpris_server::Player>) -> F + 'static)
    where
        F: Future<Output = mpris_server::zbus::Result<()>> + 'static,
    {
        let player = self.player.clone();
        glib::spawn_future_local(async move {
            if let Err(err) = change(player).await {
                tracing::warn!("could not update MPRIS: {err}");
            }
        });
    }

    /// What plays now, or nothing.
    pub fn set_track(&self, track: Option<Track>) {
        let mut metadata = Metadata::new();
        let playing = track.is_some();
        if let Some(track) = track {
            metadata.set_trackid(track_id(track.id));
            metadata.set_title(Some(track.title));
            metadata.set_artist(Some([track.show.clone()]));
            metadata.set_album(Some(track.show));
            metadata.set_length(track.duration.map(time));
            metadata.set_art_url(
                track
                    .cover
                    .and_then(|path| glib::filename_to_uri(path, None).ok())
                    .map(String::from),
            );
        }
        self.update(move |player| async move {
            player.set_metadata(metadata).await?;
            player.set_can_play(playing).await?;
            player.set_can_pause(playing).await?;
            player.set_can_seek(playing).await?;
            player.set_can_go_next(playing).await
        });
    }

    pub fn set_state(&self, state: PlaybackState) {
        let status = status(state);
        self.update(move |player| async move { player.set_playback_status(status).await });
    }

    /// Where playback is; read by other apps when they ask, so no signal.
    pub fn set_position(&self, position: Duration) {
        self.player.set_position(time(position));
    }

    /// Playback jumped to `position`: other apps' position bars follow.
    pub fn seeked(&self, position: Duration) {
        self.set_position(position);
        self.update(move |player| async move { player.seeked(time(position)).await });
    }
}

/// The MPRIS playback status for the player's state.
fn status(state: PlaybackState) -> PlaybackStatus {
    match state {
        PlaybackState::Playing | PlaybackState::Buffering => PlaybackStatus::Playing,
        PlaybackState::Paused => PlaybackStatus::Paused,
        PlaybackState::Stopped => PlaybackStatus::Stopped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_ids_name_their_episode() {
        let id = track_id(EpisodeId(42)).unwrap();
        assert_eq!(id.as_str(), "/com/theoforger/Beacon/Episode/42");
        assert_eq!(episode_of(&id), Some(EpisodeId(42)));
        assert_eq!(episode_of(&TrackId::NO_TRACK), None);
    }

    /// Another app asks for Play/Pause and a seek, and reads the status:
    /// as GNOME Shell's media controls do. Needs a session bus (run under
    /// `dbus-run-session`); without one, as in CI, there is nothing to ask.
    #[test]
    fn other_apps_control_playback_and_see_its_state() {
        use std::cell::RefCell;

        use mpris_server::zbus::{self, zvariant::OwnedVariant};

        if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
            return;
        }
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                context.block_on(async {
                    let name = format!("com.theoforger.Beacon.Test{}", std::process::id());
                    let asked = Rc::new(RefCell::new(Vec::new()));
                    let record = asked.clone();
                    let mpris =
                        Mpris::start_as(&name, move |control| record.borrow_mut().push(control))
                            .await
                            .expect("the session bus gives the test its own name");
                    mpris.set_track(Some(Track {
                        id: EpisodeId(7),
                        title: "Letters home".into(),
                        show: "Paper Trails".into(),
                        cover: None,
                        duration: Some(Duration::from_secs(60)),
                    }));
                    mpris.set_state(PlaybackState::Playing);

                    let client = zbus::Connection::session().await.unwrap();
                    let destination = format!("org.mpris.MediaPlayer2.{name}");
                    let call = |method: &'static str, body: Option<i64>| {
                        let (client, destination) = (client.clone(), destination.clone());
                        async move {
                            let path = "/org/mpris/MediaPlayer2";
                            let interface = Some("org.mpris.MediaPlayer2.Player");
                            match body {
                                Some(offset) => {
                                    client
                                        .call_method(
                                            Some(destination.as_str()),
                                            path,
                                            interface,
                                            method,
                                            &(offset,),
                                        )
                                        .await
                                }
                                None => {
                                    client
                                        .call_method(
                                            Some(destination.as_str()),
                                            path,
                                            interface,
                                            method,
                                            &(),
                                        )
                                        .await
                                }
                            }
                            .unwrap();
                        }
                    };
                    call("PlayPause", None).await;
                    call("Seek", Some(-5_000_000)).await;
                    // The server answers on the main loop; let it run.
                    for _ in 0..50 {
                        if asked.borrow().len() == 2 {
                            break;
                        }
                        glib::timeout_future(Duration::from_millis(10)).await;
                    }
                    assert_eq!(
                        *asked.borrow(),
                        [MediaControl::PlayPause, MediaControl::SeekBy(-5_000_000)]
                    );

                    let reply = client
                        .call_method(
                            Some(destination.as_str()),
                            "/org/mpris/MediaPlayer2",
                            Some("org.freedesktop.DBus.Properties"),
                            "Get",
                            &("org.mpris.MediaPlayer2.Player", "PlaybackStatus"),
                        )
                        .await
                        .unwrap();
                    let status: OwnedVariant = reply.body().deserialize().unwrap();
                    assert_eq!(String::try_from(status).unwrap(), "Playing");
                })
            })
            .unwrap();
    }

    #[test]
    fn buffering_counts_as_playing() {
        assert_eq!(status(PlaybackState::Buffering), PlaybackStatus::Playing);
        assert_eq!(status(PlaybackState::Paused), PlaybackStatus::Paused);
        assert_eq!(status(PlaybackState::Stopped), PlaybackStatus::Stopped);
    }
}
