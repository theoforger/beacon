// SPDX-License-Identifier: GPL-3.0-or-later

//! The narrow layout's player, as GNOME Podcasts has it (`player_small.ui`,
//! `sheet_player.ui`): a slim bar at the bottom of the window (progress,
//! cover, title and show, Play and Pause) that opens a bottom sheet with the
//! whole player, headed by a large cover and the episode's title and show.

use gettextrs::gettext;
use relm4::ComponentSender;
use relm4::gtk::{self, pango, prelude::*};

use super::{PlayerBar, PlayerBarOutput};
use crate::ui::covers::Cover;
use crate::ui::play_button;

/// GNOME Podcasts' small player cover.
const MINI_COVER: i32 = 42;
/// The sheet's cover, as large as a narrow window comfortably shows.
const SHEET_COVER: i32 = 200;

/// The slim bar under the narrow layout's content.
pub struct MiniBar {
    pub root: gtk::Stack,
    pub cover: Cover,
    pub title: gtk::Label,
    pub show: gtk::Label,
    pub play: gtk::Button,
    pub progress: gtk::ProgressBar,
}

/// The top of the sheet: the cover, which opens the episode's details, then
/// its title and show.
pub struct SheetHeader {
    pub root: gtk::Box,
    pub cover: Cover,
    pub title: gtk::Label,
    pub show: gtk::Label,
}

pub fn mini_bar(sender: &ComponentSender<PlayerBar>) -> MiniBar {
    let root = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .build();
    root.update_property(&[gtk::accessible::Property::Label(&gettext("Now Playing"))]);
    let nothing = gtk::Label::builder()
        .label(gettext("Nothing Playing"))
        .css_classes(["dim-label"])
        .height_request(56)
        .build();
    root.add_named(&nothing, Some("nothing"));

    // Added, not set: setting the classes would drop `horizontal`.
    let progress = gtk::ProgressBar::new();
    progress.add_css_class("mini-progress");
    let cover = Cover::new(MINI_COVER);
    let title = gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(pango::EllipsizeMode::End)
        .css_classes(["caption-heading"])
        .build();
    let show = gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(pango::EllipsizeMode::End)
        .css_classes(["caption", "dim-label"])
        .build();
    let labels = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Center)
        .hexpand(true)
        .spacing(2)
        .build();
    labels.append(&title);
    labels.append(&show);
    let label = gettext("Play");
    let play = gtk::Button::builder()
        .icon_name("media-playback-start-symbolic")
        .tooltip_text(&label)
        .valign(gtk::Align::Center)
        .css_classes(["flat", "circular"])
        .build();
    play.update_property(&[gtk::accessible::Property::Label(&label)]);
    let s = sender.clone();
    play.connect_clicked(move |_| {
        let _ = s.output(PlayerBarOutput::PlayPause);
    });
    let row = gtk::Box::builder()
        .spacing(12)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(12)
        .margin_end(12)
        .build();
    row.append(cover.widget());
    row.append(&labels);
    row.append(&play_button::with_spinner(&play));
    let bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    bar.append(&progress);
    bar.append(&row);
    // Choosing the bar opens the sheet (`AdwBottomSheet`); say so.
    bar.set_tooltip_text(Some(&gettext("Show Player")));
    root.add_named(&bar, Some("player"));
    root.set_visible_child_name("nothing");

    MiniBar {
        root,
        cover,
        title,
        show,
        play,
        progress,
    }
}

pub fn sheet_header(sender: &ComponentSender<PlayerBar>) -> SheetHeader {
    let cover = Cover::new(SHEET_COVER);
    let cover_button = gtk::Button::builder()
        .child(cover.widget())
        .halign(gtk::Align::Center)
        .css_classes(["flat", "sheet-cover"])
        .tooltip_text(gettext("Show Episode Details"))
        .build();
    cover_button.update_property(&[gtk::accessible::Property::Label(&gettext(
        "Show Episode Details",
    ))]);
    let s = sender.clone();
    cover_button.connect_clicked(move |_| {
        let _ = s.output(PlayerBarOutput::OpenDetails);
    });
    let title = gtk::Label::builder()
        .justify(gtk::Justification::Center)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .css_classes(["title-3"])
        .build();
    let show = gtk::Label::builder()
        .justify(gtk::Justification::Center)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        // Clear of the sheet's drag handle.
        .margin_top(30)
        .margin_bottom(12)
        .visible(false)
        .build();
    root.append(&cover_button);
    root.append(&title);
    root.append(&show);
    SheetHeader {
        root,
        cover,
        title,
        show,
    }
}
