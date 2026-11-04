// SPDX-License-Identifier: GPL-3.0-or-later

//! The player bar's widgets, built once: what is playing, the controls, the
//! seek bar with its times, and speed, sleep timer and volume.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use beacon_core::playback::{self, SleepTimer};
use gettextrs::{gettext, ngettext};
use relm4::ComponentSender;
use relm4::adw::prelude::*;
use relm4::gtk::{self, gio, glib, pango};

use super::{COVER_SIZE, PlayerBar, PlayerBarOutput, PlayerBarWidgets};
use crate::ui::covers::Cover;
use crate::ui::format;

/// Build the bar into `root`, its controls reporting to `sender`. While
/// `setting` is set, changes to the sliders are the bar's own, not the
/// listener's.
pub(super) fn widgets(
    root: gtk::Stack,
    sender: &ComponentSender<PlayerBar>,
    setting: &Rc<Cell<bool>>,
) -> PlayerBarWidgets {
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
    let rate_action =
        gio::SimpleAction::new_stateful("rate", Some(glib::VariantTy::DOUBLE), &1.0.to_variant());
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

    PlayerBarWidgets {
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

/// The sleep timer menu's action target for a choice.
pub(super) fn timer_name(timer: SleepTimer) -> String {
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
