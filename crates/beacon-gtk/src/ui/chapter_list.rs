// SPDX-License-Identifier: GPL-3.0-or-later

//! An episode's chapters as list rows, the same in the details pane and the
//! player bar's chapter popover: each row the chapter's start time and
//! title, the one playing grey and bold.

use beacon_core::model::Chapter;
use gettextrs::gettext;
use relm4::gtk::{self, pango, prelude::*};

use crate::ui::format;

/// Fill the chapter list: each row its start time and title.
pub fn fill(list: &gtk::ListBox, chapters: &[Chapter]) {
    list.remove_all();
    for chapter in chapters {
        let time = gtk::Label::builder()
            .label(format::clock(chapter.start))
            .xalign(1.0)
            .width_chars(7)
            .css_classes(["dim-label", "numeric"])
            .build();
        let title = gtk::Label::builder()
            .label(&chapter.title)
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(pango::WrapMode::WordChar)
            .hexpand(true)
            .build();
        let line = gtk::Box::builder()
            .spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        line.append(&time);
        line.append(&title);
        let row = gtk::ListBoxRow::builder()
            .child(&line)
            .activatable(true)
            .build();
        row.update_property(&[gtk::accessible::Property::Label(
            &gettext("{time}, {title}")
                .replace("{time}", &format::clock(chapter.start))
                .replace("{title}", &chapter.title),
        )]);
        list.append(&row);
    }
}

/// Show `row` as the chapter playing, or not: grey and bold, and said.
pub fn mark(row: &gtk::ListBoxRow, current: bool) {
    if current {
        row.add_css_class("current-chapter");
        row.update_property(&[gtk::accessible::Property::Description(&gettext(
            "Playing now",
        ))]);
    } else {
        row.remove_css_class("current-chapter");
        row.reset_property(gtk::AccessibleProperty::Description);
    }
}
