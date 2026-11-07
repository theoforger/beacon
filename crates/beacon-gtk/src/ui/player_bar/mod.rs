// SPDX-License-Identifier: GPL-3.0-or-later

//! The persistent bar across the bottom of the window: what is playing (a
//! flat button that opens its details, as in GNOME Podcasts), Favourite,
//! Back 10 Seconds, a neutral round Play and Pause, Forward 30 Seconds,
//! Next Episode, a seek bar with elapsed and remaining time, then speed,
//! sleep timer and volume. In the narrow layout it stacks and leaves out
//! speed, sleep timer and volume, as in the mockup.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use beacon_core::model::EpisodeId;
use beacon_core::playback::SleepTimer;
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::gtk::{self, gio};
use relm4::{Component, ComponentParts, ComponentSender};

use crate::player::PlaybackState;
use crate::ui::covers::Cover;
use crate::ui::format;
use crate::ui::play_button;

mod build;
mod compact;

const COVER_SIZE: i32 = 48;

/// What the bar shows of the episode in the player.
#[derive(Debug, Clone)]
pub struct NowPlayingCard {
    pub id: EpisodeId,
    pub title: String,
    pub show_title: String,
    /// The show's cover, until the episode's own arrives.
    pub cover: Option<PathBuf>,
    pub favorite: bool,
    pub duration: Option<Duration>,
}

#[derive(Debug)]
pub enum PlayerBarInput {
    Episode(Option<Box<NowPlayingCard>>),
    /// The episode's own cover, once fetched.
    Cover(EpisodeId, PathBuf),
    State(PlaybackState),
    Position(Duration),
    Duration(Duration),
    Favorite(bool),
    Rate(f64),
    Volume(f64),
    SleepTimer(SleepTimer),
    HasNext(bool),
    /// The narrow layout, where the bar is the bottom sheet's content.
    Sheet(bool),
}

#[derive(Debug)]
pub enum PlayerBarOutput {
    PlayPause,
    SkipBack,
    SkipForward,
    Next,
    Seek(Duration),
    Rate(f64),
    Volume(f64),
    SleepTimer(SleepTimer),
    ToggleFavorite,
    OpenDetails,
}

pub struct PlayerBar {
    episode: Option<EpisodeId>,
    duration: Option<Duration>,
    /// Ignore the seek bar's own changes while the bar sets it.
    setting: Rc<Cell<bool>>,
    /// The second the times show, so they change once a second rather than
    /// with each of the player's four position updates.
    shown_second: Option<(u64, u64)>,
}

pub struct PlayerBarWidgets {
    pub(super) mini: compact::MiniBar,
    pub(super) sheet_header: compact::SheetHeader,
    pub(super) now_button: gtk::Button,
    /// Holds the bar, or in the sheet the column.
    pub(super) holder: gtk::Box,
    pub(super) column: gtk::Box,
    pub(super) start: gtk::Box,
    pub(super) center: gtk::Box,
    pub(super) root: gtk::Stack,
    pub(super) bar: gtk::CenterBox,
    pub(super) extras: gtk::Box,
    pub(super) cover: Cover,
    pub(super) title: gtk::Label,
    pub(super) show: gtk::Label,
    pub(super) favorite: gtk::Button,
    pub(super) play: gtk::Button,
    pub(super) next: gtk::Button,
    pub(super) seek: gtk::Scale,
    pub(super) elapsed: gtk::Label,
    pub(super) remaining: gtk::Label,
    pub(super) rate: gtk::MenuButton,
    pub(super) rate_action: gio::SimpleAction,
    pub(super) sleep: gtk::MenuButton,
    pub(super) sleep_dot: gtk::Box,
    pub(super) sleep_action: gio::SimpleAction,
    pub(super) volume: gtk::Scale,
    pub(super) volume_icon: gtk::Image,
}

impl PlayerBarWidgets {
    /// The narrow layout's slim bar, which opens the sheet the player is in.
    pub fn mini(&self) -> &gtk::Stack {
        &self.mini.root
    }

    /// Lay the player out for the narrow layout's bottom sheet, stacked under
    /// a large cover with speed, sleep timer and volume kept, or as the wide
    /// layout's bar.
    pub fn set_sheet(&self, sheet: bool) {
        if sheet == self.column.parent().is_some() {
            return;
        }
        self.now_button.set_visible(!sheet);
        self.sheet_header.root.set_visible(sheet);
        self.extras.set_halign(if sheet {
            gtk::Align::Center
        } else {
            gtk::Align::Fill
        });
        self.extras.set_margin_top(if sheet { 12 } else { 0 });
        // A column rather than the bar stood on end: a vertical centre box
        // keeps its middle centred, leaving a gap as tall as the cover.
        if sheet {
            self.bar.set_start_widget(None::<&gtk::Widget>);
            self.bar.set_center_widget(None::<&gtk::Widget>);
            self.bar.set_end_widget(None::<&gtk::Widget>);
            for part in [&self.start, &self.center, &self.extras] {
                self.column.append(part);
            }
            self.holder.remove(&self.bar);
            self.holder.append(&self.column);
        } else {
            for part in [&self.start, &self.center, &self.extras] {
                self.column.remove(part);
            }
            self.bar.set_start_widget(Some(&self.start));
            self.bar.set_center_widget(Some(&self.center));
            self.bar.set_end_widget(Some(&self.extras));
            self.holder.remove(&self.column);
            self.holder.append(&self.bar);
        }
    }
}

impl Component for PlayerBar {
    type Init = ();
    type Input = PlayerBarInput;
    type Output = PlayerBarOutput;
    type CommandOutput = ();
    type Root = gtk::Stack;
    type Widgets = PlayerBarWidgets;

    fn init_root() -> Self::Root {
        // Each page its own height: the stacked narrow bar is much taller
        // than "Nothing Playing".
        gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .vhomogeneous(false)
            .interpolate_size(true)
            .build()
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let setting = Rc::new(Cell::new(false));
        let widgets = build::widgets(root, &sender, &setting);
        let model = PlayerBar {
            episode: None,
            duration: None,
            setting,
            shown_second: None,
        };
        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        // The bar setting its own controls is not the listener moving them.
        self.setting.set(true);
        match message {
            PlayerBarInput::Episode(None) => {
                self.episode = None;
                widgets.root.set_visible_child_name("nothing");
                widgets.mini.root.set_visible_child_name("nothing");
            }
            PlayerBarInput::Episode(Some(card)) => {
                self.episode = Some(card.id);
                self.duration = card.duration;
                for cover in [
                    &widgets.cover,
                    &widgets.mini.cover,
                    &widgets.sheet_header.cover,
                ] {
                    cover.set(card.cover.as_deref());
                }
                for title in [
                    &widgets.title,
                    &widgets.mini.title,
                    &widgets.sheet_header.title,
                ] {
                    title.set_label(&card.title);
                }
                for show in [
                    &widgets.show,
                    &widgets.mini.show,
                    &widgets.sheet_header.show,
                ] {
                    show.set_label(&card.show_title);
                }
                widgets.mini.root.set_visible_child_name("player");
                set_favorite(&widgets.favorite, card.favorite);
                widgets.root.set_visible_child_name("player");
                set_position(self, widgets, Duration::ZERO);
            }
            PlayerBarInput::Cover(id, path) => {
                if self.episode == Some(id) {
                    for cover in [
                        &widgets.cover,
                        &widgets.mini.cover,
                        &widgets.sheet_header.cover,
                    ] {
                        cover.set(Some(&path));
                    }
                }
            }
            PlayerBarInput::State(state) => {
                for play in [&widgets.play, &widgets.mini.play] {
                    // The round button stays; only its icon spins.
                    play_button::set_icon(play, Some(state), false);
                }
            }
            PlayerBarInput::Position(position) => set_position(self, widgets, position),
            PlayerBarInput::Duration(duration) if Some(duration) == self.duration => {}
            PlayerBarInput::Duration(duration) => self.duration = Some(duration),
            PlayerBarInput::Favorite(favorite) => set_favorite(&widgets.favorite, favorite),
            PlayerBarInput::Rate(rate) => {
                widgets.rate.set_label(&format::rate(rate));
                widgets.rate_action.set_state(&rate.to_variant());
                widgets
                    .rate
                    .update_property(&[gtk::accessible::Property::Label(
                        &gettext("Playback Speed: {rate}").replace("{rate}", &format::rate(rate)),
                    )]);
            }
            PlayerBarInput::Volume(volume) => {
                widgets.volume.set_value(volume);
                let icon = match volume {
                    v if v <= 0.0 => "audio-volume-muted-symbolic",
                    v if v < 0.34 => "audio-volume-low-symbolic",
                    v if v < 0.67 => "audio-volume-medium-symbolic",
                    _ => "audio-volume-high-symbolic",
                };
                widgets.volume_icon.set_icon_name(Some(icon));
            }
            PlayerBarInput::SleepTimer(timer) => {
                widgets
                    .sleep_action
                    .set_state(&build::timer_name(timer).to_variant());
                let on = timer != SleepTimer::Off;
                widgets.sleep_dot.set_visible(on);
                // The dot is only seen; say it too.
                if on {
                    widgets
                        .sleep
                        .update_property(&[gtk::accessible::Property::Description(&gettext(
                            "The sleep timer is on",
                        ))]);
                } else {
                    widgets
                        .sleep
                        .reset_property(gtk::AccessibleProperty::Description);
                }
            }
            PlayerBarInput::HasNext(has_next) => widgets.next.set_sensitive(has_next),
            PlayerBarInput::Sheet(sheet) => widgets.set_sheet(sheet),
        }
        self.setting.set(false);
    }
}

fn set_position(bar: &mut PlayerBar, widgets: &PlayerBarWidgets, position: Duration) {
    let total = bar.duration.unwrap_or_default();
    widgets.seek.set_range(
        0.0,
        total.as_secs_f64().max(position.as_secs_f64()).max(1.0),
    );
    widgets.seek.set_value(position.as_secs_f64());
    if !total.is_zero() {
        widgets
            .mini
            .progress
            .set_fraction((position.as_secs_f64() / total.as_secs_f64()).min(1.0));
    }
    let second = (position.as_secs(), total.as_secs());
    if bar.shown_second.replace(second) == Some(second) {
        return;
    }
    widgets.elapsed.set_label(&format::clock(position));
    let left = total.saturating_sub(position);
    // Translators: time left in the episode, as in "-33:28".
    widgets
        .remaining
        .set_label(&gettext("-{time}").replace("{time}", &format::clock(left)));
    widgets
        .seek
        .update_property(&[gtk::accessible::Property::ValueText(
            &gettext("{elapsed} of {total}")
                .replace("{elapsed}", &format::clock(position))
                .replace("{total}", &format::clock(total)),
        )]);
}

fn set_favorite(button: &gtk::Button, favorite: bool) {
    // The state shows through the icon only.
    let (icon, label) = if favorite {
        ("starred-symbolic", gettext("Remove From Favorites"))
    } else {
        ("non-starred-symbolic", gettext("Add to Favorites"))
    };
    button.set_icon_name(icon);
    button.set_tooltip_text(Some(&label));
    button.update_property(&[gtk::accessible::Property::Label(&label)]);
}
