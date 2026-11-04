// SPDX-License-Identifier: GPL-3.0-or-later

//! The persistent bar across the bottom of the window: what is playing (a
//! flat button that opens its details, as in GNOME Podcasts), Favourite,
//! Back 10 Seconds, a neutral round Play and Pause, Forward 30 Seconds,
//! Next Episode, a seek bar with elapsed and remaining time, then speed,
//! sleep timer and volume. In the narrow layout it stacks and leaves out
//! speed, sleep timer and volume, as in the mockup.

use std::path::PathBuf;
use std::time::Duration;

use beacon_core::model::EpisodeId;
use beacon_core::playback::{self, SleepTimer};
use gettextrs::{gettext, ngettext};
use relm4::adw::prelude::*;
use relm4::gtk::{self, gio, glib, pango};
use relm4::{Component, ComponentParts, ComponentSender};

use crate::player::PlaybackState;
use crate::ui::covers::Cover;
use crate::ui::format;

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
    setting: std::rc::Rc<std::cell::Cell<bool>>,
    /// The second the times show, so they change once a second rather than
    /// with each of the player's four position updates.
    shown_second: Option<(u64, u64)>,
}

pub struct PlayerBarWidgets {
    root: gtk::Stack,
    bar: gtk::CenterBox,
    extras: gtk::Box,
    cover: Cover,
    title: gtk::Label,
    show: gtk::Label,
    favorite: gtk::Button,
    play: gtk::Button,
    next: gtk::Button,
    seek: gtk::Scale,
    elapsed: gtk::Label,
    remaining: gtk::Label,
    rate: gtk::MenuButton,
    rate_action: gio::SimpleAction,
    sleep: gtk::MenuButton,
    sleep_dot: gtk::Box,
    sleep_action: gio::SimpleAction,
    volume: gtk::Scale,
    volume_icon: gtk::Image,
}

impl PlayerBarWidgets {
    /// The bar, which the narrow layout stacks.
    pub fn bar(&self) -> &gtk::CenterBox {
        &self.bar
    }

    /// Speed, sleep timer and volume, which the narrow layout leaves out.
    pub fn extras(&self) -> &gtk::Box {
        &self.extras
    }
}

/// An icon-only button with its tooltip and accessible label.
fn icon_button(icon: &str, label: &str, classes: &[&str]) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(label)
        .valign(gtk::Align::Center)
        .css_classes(classes)
        .build();
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button
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
        root.update_property(&[gtk::accessible::Property::Label(&gettext(
            "Playback Controls",
        ))]);
        let nothing = gtk::Label::builder()
            .label(gettext("Nothing Playing"))
            .css_classes(["dim-label"])
            .height_request(64)
            .build();
        root.add_named(&nothing, Some("nothing"));

        // What is playing: opens its details, as GNOME Podcasts' cover
        // button does.
        let cover = Cover::new(COVER_SIZE);
        let title = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(pango::EllipsizeMode::End)
            .max_width_chars(36)
            .css_classes(["caption-heading"])
            .build();
        let show = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(pango::EllipsizeMode::End)
            .max_width_chars(36)
            .css_classes(["caption", "dim-label"])
            .build();
        let labels = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::Center)
            .spacing(2)
            .build();
        labels.append(&title);
        labels.append(&show);
        let now = gtk::Box::builder().spacing(12).build();
        now.append(cover.widget());
        now.append(&labels);
        let now_button = gtk::Button::builder()
            .child(&now)
            .css_classes(["flat", "now-playing"])
            .tooltip_text(gettext("Show Episode Details"))
            .build();
        now_button.connect_clicked({
            let sender = sender.clone();
            move |_| {
                let _ = sender.output(PlayerBarOutput::OpenDetails);
            }
        });

        // The controls, Favourite first, as in the mockup.
        let favorite = icon_button(
            "non-starred-symbolic",
            &gettext("Add to Favorites"),
            &["flat", "circular"],
        );
        let back = icon_button(
            "skip-back-10-symbolic",
            &gettext("Back 10 Seconds"),
            &["flat", "circular"],
        );
        // Neutral and round: the one filled control, not the accent.
        let play = icon_button(
            "media-playback-start-symbolic",
            &gettext("Play"),
            &["circular", "play-button"],
        );
        let forward = icon_button(
            "skip-forward-30-symbolic",
            &gettext("Forward 30 Seconds"),
            &["flat", "circular"],
        );
        let next = icon_button(
            "media-skip-forward-symbolic",
            &gettext("Next Episode"),
            &["flat", "circular"],
        );
        let clicked = |button: &gtk::Button, output: fn() -> PlayerBarOutput| {
            let sender = sender.clone();
            button.connect_clicked(move |_| {
                let _ = sender.output(output());
            });
        };
        clicked(&favorite, || PlayerBarOutput::ToggleFavorite);
        clicked(&back, || PlayerBarOutput::SkipBack);
        clicked(&play, || PlayerBarOutput::PlayPause);
        clicked(&forward, || PlayerBarOutput::SkipForward);
        clicked(&next, || PlayerBarOutput::Next);
        let controls = gtk::Box::builder()
            .spacing(6)
            .halign(gtk::Align::Center)
            .build();
        for button in [&favorite, &back, &play, &forward, &next] {
            controls.append(button);
        }

        // The seek bar, with elapsed and remaining time.
        let elapsed = gtk::Label::builder()
            .css_classes(["caption", "numeric", "dim-label"])
            .xalign(1.0)
            .width_chars(7)
            .build();
        let remaining = gtk::Label::builder()
            .css_classes(["caption", "numeric", "dim-label"])
            .xalign(0.0)
            .width_chars(7)
            .build();
        let seek = gtk::Scale::builder()
            .orientation(gtk::Orientation::Horizontal)
            .draw_value(false)
            .hexpand(true)
            .width_request(240)
            .build();
        seek.update_property(&[gtk::accessible::Property::Label(&gettext("Seek"))]);
        let setting = std::rc::Rc::new(std::cell::Cell::new(false));
        seek.connect_value_changed({
            let sender = sender.clone();
            let setting = setting.clone();
            move |scale| {
                if !setting.get() {
                    let position = Duration::from_secs_f64(scale.value().max(0.0));
                    let _ = sender.output(PlayerBarOutput::Seek(position));
                }
            }
        });
        let seek_row = gtk::Box::builder().spacing(6).build();
        seek_row.append(&elapsed);
        seek_row.append(&seek);
        seek_row.append(&remaining);
        let center = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .hexpand(true)
            .build();
        center.append(&controls);
        center.append(&seek_row);

        // Speed: GNOME Podcasts' rate menu (`player_rate.ui`).
        let actions = gio::SimpleActionGroup::new();
        let rate_action = gio::SimpleAction::new_stateful(
            "rate",
            Some(glib::VariantTy::DOUBLE),
            &1.0.to_variant(),
        );
        rate_action.connect_activate({
            let sender = sender.clone();
            move |_, target| {
                if let Some(rate) = target.and_then(|t| t.get::<f64>()) {
                    let _ = sender.output(PlayerBarOutput::Rate(rate));
                }
            }
        });
        actions.add_action(&rate_action);
        let rates = gio::Menu::new();
        for rate in playback::RATES.into_iter().rev() {
            let item = gio::MenuItem::new(Some(&format::rate(rate)), None);
            item.set_action_and_target_value(Some("player.rate"), Some(&rate.to_variant()));
            rates.append_item(&item);
        }
        let rate = gtk::MenuButton::builder()
            .label(format::rate(1.0))
            .menu_model(&rates)
            .tooltip_text(gettext("Playback Speed"))
            .valign(gtk::Align::Center)
            .css_classes(["flat", "numeric"])
            .build();

        // Sleep timer: a menu of lengths, and a dot while it runs.
        let sleep_action = gio::SimpleAction::new_stateful(
            "sleep",
            Some(glib::VariantTy::STRING),
            &"off".to_variant(),
        );
        sleep_action.connect_activate({
            let sender = sender.clone();
            move |_, target| {
                if let Some(value) = target.and_then(|t| t.get::<String>()) {
                    let _ = sender.output(PlayerBarOutput::SleepTimer(timer_from(&value)));
                }
            }
        });
        actions.add_action(&sleep_action);
        let lengths = gio::Menu::new();
        for choice in SleepTimer::CHOICES {
            let label = match choice {
                SleepTimer::Off => gettext("_Off"),
                SleepTimer::Minutes(n) => {
                    // Translators: a sleep timer length; the access key is on
                    // its first digit.
                    ngettext("_{n} Minute", "_{n} Minutes", n).replace("{n}", &n.to_string())
                }
                SleepTimer::EndOfEpisode => gettext("_End of Episode"),
            };
            let item = gio::MenuItem::new(Some(&label), None);
            item.set_action_and_target_value(
                Some("player.sleep"),
                Some(&timer_name(choice).to_variant()),
            );
            lengths.append_item(&item);
        }
        let sleep = gtk::MenuButton::builder()
            .icon_name("moon-symbolic")
            .menu_model(&lengths)
            .tooltip_text(gettext("Sleep Timer"))
            .valign(gtk::Align::Center)
            .css_classes(["flat", "circular"])
            .build();
        sleep.update_property(&[gtk::accessible::Property::Label(&gettext("Sleep Timer"))]);
        let sleep_dot = gtk::Box::builder()
            .halign(gtk::Align::End)
            .valign(gtk::Align::Start)
            .margin_end(5)
            .margin_top(5)
            .can_focus(false)
            .can_target(false)
            .visible(false)
            .css_classes(["filter-dot"])
            .build();
        let sleep_overlay = gtk::Overlay::builder().child(&sleep).build();
        sleep_overlay.add_overlay(&sleep_dot);

        // Volume: Beacon's own stream volume.
        let volume_icon = gtk::Image::builder()
            .icon_name("audio-volume-high-symbolic")
            .accessible_role(gtk::AccessibleRole::Presentation)
            .build();
        let volume = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.05);
        volume.set_draw_value(false);
        volume.set_width_request(100);
        volume.update_property(&[gtk::accessible::Property::Label(&gettext("Volume"))]);
        volume.connect_value_changed({
            let sender = sender.clone();
            let setting = setting.clone();
            move |scale| {
                if !setting.get() {
                    let _ = sender.output(PlayerBarOutput::Volume(scale.value()));
                }
            }
        });
        let extras = gtk::Box::builder()
            .spacing(6)
            .valign(gtk::Align::Center)
            .build();
        extras.append(&rate);
        extras.append(&sleep_overlay);
        extras.append(&volume_icon);
        extras.append(&volume);

        let bar = gtk::CenterBox::builder()
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(12)
            .build();
        bar.set_start_widget(Some(&now_button));
        bar.set_center_widget(Some(&center));
        bar.set_end_widget(Some(&extras));
        root.add_named(&bar, Some("player"));
        root.set_visible_child_name("nothing");
        root.insert_action_group("player", Some(&actions));

        let model = PlayerBar {
            episode: None,
            duration: None,
            setting,
            shown_second: None,
        };
        let widgets = PlayerBarWidgets {
            root,
            bar,
            extras,
            cover,
            title,
            show,
            favorite,
            play,
            next,
            seek,
            elapsed,
            remaining,
            rate,
            rate_action,
            sleep,
            sleep_dot,
            sleep_action,
            volume,
            volume_icon,
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
            }
            PlayerBarInput::Episode(Some(card)) => {
                self.episode = Some(card.id);
                self.duration = card.duration;
                widgets.cover.set(card.cover.as_deref());
                widgets.title.set_label(&card.title);
                widgets.show.set_label(&card.show_title);
                set_favorite(&widgets.favorite, card.favorite);
                widgets.root.set_visible_child_name("player");
                set_position(self, widgets, Duration::ZERO);
            }
            PlayerBarInput::Cover(id, path) => {
                if self.episode == Some(id) {
                    widgets.cover.set(Some(&path));
                }
            }
            PlayerBarInput::State(state) => {
                let playing = matches!(state, PlaybackState::Playing | PlaybackState::Buffering);
                let (icon, label) = if playing {
                    ("media-playback-pause-symbolic", gettext("Pause"))
                } else {
                    ("media-playback-start-symbolic", gettext("Play"))
                };
                widgets.play.set_icon_name(icon);
                widgets.play.set_tooltip_text(Some(&label));
                widgets
                    .play
                    .update_property(&[gtk::accessible::Property::Label(&label)]);
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
                    .set_state(&timer_name(timer).to_variant());
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

fn set_position(bar: &mut PlayerBar, widgets: &PlayerBarWidgets, position: Duration) {
    let total = bar.duration.unwrap_or_default();
    widgets.seek.set_range(
        0.0,
        total.as_secs_f64().max(position.as_secs_f64()).max(1.0),
    );
    widgets.seek.set_value(position.as_secs_f64());
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

/// The sleep timer menu's action target for a choice.
fn timer_name(timer: SleepTimer) -> String {
    match timer {
        SleepTimer::Off => "off".into(),
        SleepTimer::Minutes(n) => n.to_string(),
        SleepTimer::EndOfEpisode => "end".into(),
    }
}

fn timer_from(name: &str) -> SleepTimer {
    match name {
        "end" => SleepTimer::EndOfEpisode,
        n => n.parse().map_or(SleepTimer::Off, SleepTimer::Minutes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_timer_choices_round_trip_through_the_menu() {
        for choice in SleepTimer::CHOICES {
            assert_eq!(timer_from(&timer_name(choice)), choice);
        }
        assert_eq!(timer_from("nonsense"), SleepTimer::Off);
    }
}
