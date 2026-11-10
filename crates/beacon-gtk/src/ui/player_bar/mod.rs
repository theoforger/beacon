// SPDX-License-Identifier: GPL-3.0-or-later

//! The persistent bar across the bottom of the window: what is playing (a
//! flat button that opens its details, as in GNOME Podcasts), Chapters,
//! Back 10 Seconds, a neutral round Play and Pause, Forward 30 Seconds,
//! Next Episode, a seek bar with elapsed and remaining time, then speed,
//! sleep timer and volume. In the narrow layout it stacks and leaves out
//! speed, sleep timer and volume, as in the mockup.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use beacon_core::model::{Chapter, EpisodeId};
use beacon_core::playback::SleepTimer;
use gettextrs::gettext;
use gtk::glib;
use gtk::{self, gio};

use crate::player::PlaybackState;
use crate::ui::covers::Cover;
use crate::ui::format;
use crate::ui::messages::{Inbox, Output, ViewSender};
use crate::ui::play_button;

mod build;
mod chapters;
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
    /// How far into the episode the player holds, as a share of it.
    Buffered(f64),
    /// An episode's chapters, shown if it is the one playing.
    Chapters(EpisodeId, Vec<Chapter>),
    Rate(f64),
    Volume(f64),
    SleepTimer(SleepTimer),
    HasNext(bool),
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
    OpenDetails,
}

/// What the bar's widgets report through.
type Sender = ViewSender<PlayerBarInput, PlayerBarOutput>;

#[derive(Default)]
pub struct State {
    episode: Option<EpisodeId>,
    duration: Option<Duration>,
    /// Ignore the seek bar's own changes while the bar sets it.
    setting: Rc<Cell<bool>>,
    /// The second the times show, so they change once a second rather than
    /// with each of the player's four position updates.
    shown_second: Option<(u64, u64)>,
    /// How much of the episode the player holds, shown as the seek bar's
    /// fill (asked by the user 2026-11-09).
    buffered: f64,
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
    chapters: chapters::ChapterMenu,
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
    /// The Chapters button, the list in its popover and the seek bar its
    /// ticks are on, for tests.
    #[cfg(test)]
    pub fn chapters(&self) -> (&gtk::MenuButton, gtk::ListBox, &gtk::Scale) {
        (&self.chapters.button, self.chapters.list(), &self.seek)
    }

    /// The slim bar's Play, for tests.
    #[cfg(test)]
    pub fn mini_play(&self) -> &gtk::Button {
        &self.mini.play
    }

    /// The narrow layout's slim bar, which opens the sheet the player is in.
    pub fn mini(&self) -> &gtk::Stack {
        &self.mini.root
    }

    /// Stacked under a large cover with speed, sleep timer and volume kept,
    /// in the narrow layout's bottom sheet; or the wide layout's bar.
    pub(super) fn set_sheet(&self, sheet: bool) {
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

mod imp {
    use std::cell::{OnceCell, RefCell};

    use super::*;

    #[derive(Default)]
    pub struct PlayerBar {
        pub widgets: OnceCell<PlayerBarWidgets>,
        pub state: RefCell<State>,
        pub inbox: Inbox<PlayerBarInput>,
        pub output: Output<PlayerBarOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlayerBar {
        const NAME: &'static str = "BeaconPlayerBar";
        type Type = super::PlayerBar;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for PlayerBar {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            // Each page its own height: the stacked narrow bar is much
            // taller than "Nothing Playing". Not interpolated: the stack
            // then gave the player less height than it needs while it grew
            // (GTK warned).
            let root = gtk::Stack::builder()
                .transition_type(gtk::StackTransitionType::Crossfade)
                .vhomogeneous(false)
                .build();
            let bar = obj.downgrade();
            let sender = Sender::new(
                move |message| {
                    if let Some(bar) = bar.upgrade() {
                        bar.send(message);
                    }
                },
                self.output.clone(),
            );
            let setting = self.state.borrow().setting.clone();
            let _ = self
                .widgets
                .set(build::widgets(root.clone(), &sender, &setting));
            obj.set_child(Some(&root));
        }
    }
    impl WidgetImpl for PlayerBar {}
    impl BinImpl for PlayerBar {}
}

glib::wrapper! {
    pub struct PlayerBar(ObjectSubclass<imp::PlayerBar>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for PlayerBar {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl PlayerBar {
    pub fn connect_output(&self, receive: impl Fn(PlayerBarOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    pub fn send(&self, input: PlayerBarInput) {
        let imp = self.imp();
        imp.inbox.deliver(input, |message| {
            imp.state.borrow_mut().update(self.widgets(), message);
        });
    }

    /// Lay the player out for the narrow layout's bottom sheet, or as the
    /// wide layout's bar.
    pub fn set_sheet(&self, sheet: bool) {
        self.widgets().set_sheet(sheet);
    }

    pub fn widgets(&self) -> &PlayerBarWidgets {
        self.imp().widgets.get().expect("built when constructed")
    }
}

impl State {
    fn update(&mut self, widgets: &PlayerBarWidgets, message: PlayerBarInput) {
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
                // Until the episode's own arrive.
                widgets.chapters.set(&[], Some(&widgets.seek));
                widgets.root.set_visible_child_name("player");
                self.buffered = 0.0;
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
                    play_button::set_icon(play, Some(state));
                }
            }
            PlayerBarInput::Position(position) => set_position(self, widgets, position),
            PlayerBarInput::Duration(duration) if Some(duration) == self.duration => {}
            PlayerBarInput::Duration(duration) => self.duration = Some(duration),
            PlayerBarInput::Buffered(share) => {
                self.buffered = share;
                fill(self, widgets);
            }
            PlayerBarInput::Chapters(id, chapters) => {
                if self.episode == Some(id) {
                    widgets.chapters.set(&chapters, Some(&widgets.seek));
                    widgets
                        .chapters
                        .mark(Duration::from_secs_f64(widgets.seek.value()));
                }
            }
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
        }
        self.setting.set(false);
    }
}

/// Fill the seek bar's trough as far as the player holds.
fn fill(bar: &State, widgets: &PlayerBarWidgets) {
    let total = bar.duration.unwrap_or_default().as_secs_f64();
    widgets.seek.set_fill_level(bar.buffered * total);
    // GTK reallocates the scale but not its trough, whose size is
    // unchanged, so the fill would keep its old width.
    let mut child = widgets.seek.first_child();
    while let Some(c) = child {
        if c.css_name() == "trough" {
            c.queue_allocate();
        }
        child = c.next_sibling();
    }
}

fn set_position(bar: &mut State, widgets: &PlayerBarWidgets, position: Duration) {
    let total = bar.duration.unwrap_or_default();
    let end = total.as_secs_f64().max(position.as_secs_f64()).max(1.0);
    if widgets.seek.adjustment().upper() != end {
        widgets.seek.set_range(0.0, end);
        widgets.chapters.tick(&widgets.seek);
        fill(bar, widgets);
    }
    widgets.seek.set_value(position.as_secs_f64());
    widgets.chapters.mark(position);
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
