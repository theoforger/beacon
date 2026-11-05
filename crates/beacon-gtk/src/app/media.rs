// SPDX-License-Identifier: GPL-3.0-or-later

//! Media keys and the desktop's media controls, through MPRIS: what they
//! ask is done as the player bar does it, and they are told what plays.

use std::cell::{OnceCell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use beacon_core::model::EpisodeId;
use beacon_core::playback::{self, SeekBy};
use relm4::adw::prelude::*;
use relm4::{ComponentSender, adw, gtk::glib};

use super::{App, AppMsg, AppWidgets};
use crate::player::PlaybackState;
use crate::player::mpris::{MediaControl, Mpris, Track};
use crate::ui::player_bar::PlayerBarOutput;

/// The MPRIS server once it has its bus name, and the track it shows.
/// Without a session bus, or with the name taken, there is none and media
/// keys do nothing; Beacon plays on.
#[derive(Debug, Default)]
pub struct Media {
    server: Rc<OnceCell<Mpris>>,
    track: RefCell<Option<Track>>,
}

impl Media {
    /// Start the server in the background; what it is asked arrives as
    /// `AppMsg::Media`.
    pub fn start(&self, sender: &ComponentSender<App>) {
        let server = self.server.clone();
        let input = sender.input_sender().clone();
        glib::spawn_future_local(async move {
            match Mpris::start(move |control| input.emit(AppMsg::Media(control))).await {
                Ok(mpris) => {
                    let _ = server.set(mpris);
                }
                Err(err) => tracing::warn!("no media keys: {err}"),
            }
        });
    }

    fn with(&self, f: impl FnOnce(&Mpris)) {
        if let Some(server) = self.server.get() {
            f(server);
        }
    }

    pub fn set_track(&self, track: Option<Track>) {
        self.track.replace(track.clone());
        self.with(|server| server.set_track(track));
    }

    /// Change the track shown, if it is episode `id`.
    pub fn update_track(&self, id: EpisodeId, change: impl FnOnce(&mut Track)) {
        let track = {
            let mut track = self.track.borrow_mut();
            let Some(track) = track.as_mut().filter(|t| t.id == id) else {
                return;
            };
            change(track);
            track.clone()
        };
        self.with(|server| server.set_track(Some(track)));
    }

    pub fn set_cover(&self, id: EpisodeId, cover: PathBuf) {
        self.update_track(id, |track| track.cover = Some(cover));
    }

    pub fn set_state(&self, state: PlaybackState) {
        self.with(|server| server.set_state(state));
    }

    pub fn set_position(&self, position: Duration) {
        self.with(|server| server.set_position(position));
    }

    pub fn seeked(&self, position: Duration) {
        self.with(|server| server.seeked(position));
    }
}

impl App {
    pub(super) fn media_control(
        &mut self,
        control: MediaControl,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
        root: &adw::ApplicationWindow,
    ) {
        let player = self.player.clone();
        let now = self
            .now_playing
            .as_ref()
            .map(|now| (now.id, now.position, now.duration));
        match control {
            MediaControl::PlayPause => self.bar_output(PlayerBarOutput::PlayPause, widgets, sender),
            MediaControl::Next => self.bar_output(PlayerBarOutput::Next, widgets, sender),
            // Stopping keeps the place, as pausing does: an episode is
            // resumed, not started over.
            MediaControl::Pause | MediaControl::Stop => {
                if let (Some(player), Some(_)) = (&player, now) {
                    player.pause();
                }
            }
            MediaControl::Play => {
                if let (Some(player), Some(_)) = (&player, now) {
                    player.play();
                }
            }
            MediaControl::SeekBy(offset) => {
                let Some((_, position, duration)) = now else {
                    return;
                };
                match playback::seek_by(position, offset, duration) {
                    SeekBy::To(to) => self.seek(to),
                    SeekBy::PastEnd => self.bar_output(PlayerBarOutput::Next, widgets, sender),
                }
            }
            MediaControl::SetPosition(id, position) => {
                if let Some((_, _, duration)) = now.filter(|(now, _, _)| *now == id)
                    && let Some(to) = playback::seek_to(position, duration)
                {
                    self.seek(to);
                }
            }
            MediaControl::Raise => root.present(),
        }
    }
}
