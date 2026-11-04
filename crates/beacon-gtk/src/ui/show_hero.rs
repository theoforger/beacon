// SPDX-License-Identifier: GPL-3.0-or-later

//! The top of a podcast page, laid out as in the mockup: the cover, then
//! the category, title, author, description and Unsubscribe beside it,
//! stacked under it when the page is narrow. More Options is in the page's
//! header bar; sorting and filtering are in its filter menu.

use beacon_core::text;
use gettextrs::gettext;
use std::cell::Cell;
use std::rc::Rc;

use relm4::adw::prelude::*;
use relm4::gtk::{self, glib, pango};

use crate::ui::covers::Cover;
use crate::ui::models::ShowPageData;

/// The cover beside the text, and above it when narrow (the mockup's `xl`).
const COVER_SIZE: i32 = 200;
const NARROW_COVER_SIZE: i32 = 150;
/// Below this width the text goes under the cover: the mockup's text column
/// wants about 320px beside a 200px cover and its 24px gap.
const NARROW_WIDTH: f64 = 560.0;

pub struct ShowHero {
    root: gtk::Overlay,
    cover: Cover,
    category: gtk::Label,
    title: gtk::Label,
    author: gtk::Label,
    description: gtk::Label,
    pub unsubscribe: gtk::Button,
}

impl ShowHero {
    pub fn new() -> Self {
        let cover = Cover::new(COVER_SIZE);
        cover.widget().set_valign(gtk::Align::End);
        let label = |classes: &[&str], lines: i32| {
            gtk::Label::builder()
                .wrap(true)
                .wrap_mode(pango::WrapMode::WordChar)
                .xalign(0.0)
                .lines(lines)
                .ellipsize(pango::EllipsizeMode::End)
                .css_classes(classes)
                .build()
        };
        let category = label(&["dim-label", "caption-heading"], 1);
        let title = label(&["title-1"], 3);
        title.set_accessible_role(gtk::AccessibleRole::Heading);
        let author = label(&["dim-label"], 2);
        let description = label(&[], 6);
        description.set_margin_top(4);
        description.set_use_markup(true);

        let unsubscribe = gtk::Button::builder()
            .label(gettext("_Unsubscribe"))
            .use_underline(true)
            .halign(gtk::Align::Start)
            .margin_top(8)
            .build();

        let text = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .hexpand(true)
            .valign(gtk::Align::End)
            .build();
        for part in [&category, &title, &author, &description] {
            text.append(part);
        }
        text.append(&unsubscribe);

        let row = gtk::Box::builder()
            .spacing(24)
            .margin_top(24)
            .margin_bottom(6)
            .build();
        row.append(cover.widget());
        row.append(&text);

        // Beside the cover when there is room, under it when not. The page
        // can be narrow in a wide window (sidebar and details open), so the
        // hero follows its own width, reported by a probe that draws
        // nothing. A breakpoint bin would do this, but it does not measure
        // height for width, which cut the wrapped description short.
        let probe = gtk::DrawingArea::builder()
            .can_target(false)
            .can_focus(false)
            .build();
        let root = gtk::Overlay::builder().child(&row).build();
        root.add_overlay(&probe);
        let narrow = Rc::new(Cell::new(false));
        let (row_ref, text_ref, cover_ref) = (row.downgrade(), text.downgrade(), cover.clone());
        probe.connect_resize(move |_, width, _| {
            let wanted = f64::from(width) < NARROW_WIDTH;
            if narrow.replace(wanted) == wanted {
                return;
            }
            // Not while being allocated: on the next turn of the main loop.
            let (row, text, cover) = (row_ref.clone(), text_ref.clone(), cover_ref.clone());
            glib::idle_add_local_once(move || {
                let (Some(row), Some(text)) = (row.upgrade(), text.upgrade()) else {
                    return;
                };
                if wanted {
                    row.set_orientation(gtk::Orientation::Vertical);
                    row.set_spacing(12);
                    text.set_valign(gtk::Align::Start);
                    cover.set_size(NARROW_COVER_SIZE);
                } else {
                    row.set_orientation(gtk::Orientation::Horizontal);
                    row.set_spacing(24);
                    text.set_valign(gtk::Align::End);
                    cover.set_size(COVER_SIZE);
                }
            });
        });

        Self {
            root,
            cover,
            category,
            title,
            author,
            description,
            unsubscribe,
        }
    }

    pub fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    pub fn update(&self, data: &ShowPageData) {
        let info = &data.show.info;
        self.cover.set(data.show.cover_path.as_deref());
        set_text(&self.category, info.category.as_deref());
        self.title.set_label(&info.title);
        set_text(&self.author, info.author.as_deref());
        let description = info.description.as_deref().map(text::markup);
        let description = description.unwrap_or_default();
        self.description.set_markup(&description);
        self.description.set_visible(!description.is_empty());
    }
}

fn set_text(label: &gtk::Label, text: Option<&str>) {
    let text = text.unwrap_or_default();
    label.set_label(text);
    label.set_visible(!text.is_empty());
}
