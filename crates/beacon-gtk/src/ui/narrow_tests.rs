// SPDX-License-Identifier: GPL-3.0-or-later

//! The narrowest window, 360px wide: the parts that span it must fit.
//! Measured with libadwaita's stylesheet and Beacon's, so this needs a
//! display; without one (CI has none) there is nothing to measure and the
//! test passes. One test, since GTK starts on one thread only.

use std::rc::Rc;
use std::time::Duration;

use beacon_core::model::{Episode, EpisodeId, EpisodeInfo, EpisodeState, ShowId};
use chrono::Utc;
use relm4::gtk::{self, gdk, prelude::*};
use relm4::{Component, ComponentController, adw};

use crate::ui::dialogs::description::text_height;
use crate::ui::models::EpisodeItem;
use crate::ui::player_bar::PlayerBar;
use crate::ui::selection::Selection;
use crate::ui::widgets::episode_row::{EpisodeRow, RowSetup};

/// The narrowest window Beacon supports.
const NARROWEST: i32 = 360;
/// What New's list takes from the window's width around a row: 12px
/// margins on each side and the boxed list's 1px borders.
const AROUND_ROW: i32 = 12 * 2 + 2;

fn partly_played() -> EpisodeItem {
    EpisodeItem {
        episode: Episode {
            id: EpisodeId(1),
            show_id: ShowId(1),
            info: EpisodeInfo {
                title: "A long episode title that goes on and on".into(),
                duration: Some(Duration::from_secs(3600)),
                ..Default::default()
            },
            state: EpisodeState {
                position: Duration::from_secs(600),
                ..Default::default()
            },
            added_at: Utc::now(),
        },
        show_title: "A show with a long name".into(),
        cover: None,
        in_queue: false,
    }
}

fn minimum_width(widget: &impl IsA<gtk::Widget>) -> i32 {
    widget.measure(gtk::Orientation::Horizontal, -1).0
}

#[test]
fn the_narrow_layout_fits_the_narrowest_window() {
    if gtk::init().is_err() {
        return;
    }
    adw::init().expect("libadwaita starts once GTK has");
    let css = gtk::CssProvider::new();
    css.load_from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/style.css"));
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("GTK started, so there is a display"),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // A partly played row: Adwaita's progress bar alone wanted 150px.
    let row = EpisodeRow::new(&RowSetup {
        on_action: Rc::new(|_, _| {}),
        show_identity: true,
        selection: Selection::default(),
    });
    row.bind(&partly_played());
    let list = gtk::ListBox::builder().css_classes(["boxed-list"]).build();
    list.append(&row);
    let needed = minimum_width(&row) + AROUND_ROW;
    assert!(needed <= NARROWEST, "a partly played row needs {needed}px");

    // The player spans the window: in the narrow layout, the slim bar and
    // the sheet with the whole player, speed, sleep timer and volume kept.
    let bar = PlayerBar::builder().launch(()).detach();
    bar.widgets().set_sheet(true);
    for (part, needed) in [
        ("slim bar", minimum_width(bar.widgets().mini())),
        ("player sheet", minimum_width(bar.widget())),
    ] {
        assert!(needed <= NARROWEST, "the narrow {part} needs {needed}px");
    }

    // Read More's dialog is as tall as its text at the width it gets: more
    // lines in a narrow window, never the single line it measured before.
    let text = gtk::Label::builder()
        .label("A show description long enough to wrap onto several lines. ".repeat(8))
        .wrap(true)
        .margin_start(24)
        .margin_end(24)
        .build();
    let line = gtk::Label::new(Some("One line"))
        .measure(gtk::Orientation::Vertical, -1)
        .1;
    let narrow = text_height(&text, NARROWEST);
    let wide = text_height(&text, 560);
    assert!(
        narrow > wide && wide > 3 * line,
        "{narrow}px narrow, {wide}px wide"
    );
}
