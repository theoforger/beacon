// SPDX-License-Identifier: GPL-3.0-or-later

//! The top of a podcast page, laid out as in the mockup: the cover, then
//! the category, title, author, description and Unsubscribe beside it,
//! stacked under it when the page is narrow. More Options is in the page's
//! header bar; sorting and filtering are in its filter menu.

use beacon_core::text;
use gettextrs::gettext;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use relm4::adw::prelude::*;
use relm4::gtk::{self, glib, pango};

use crate::ui::covers::Cover;
use crate::ui::dialogs::description;
use crate::ui::links;
use crate::ui::models::ShowPageData;

/// The cover beside the text, and above it when narrow (the mockup's `xl`).
const COVER_SIZE: i32 = 200;
const NARROW_COVER_SIZE: i32 = 150;
/// Below this width the text goes under the cover: the mockup's text column
/// wants about 320px beside a 200px cover and its 24px gap.
const NARROW_WIDTH: f64 = 560.0;

/// The most description lines: beside the cover, as many of these as fit
/// its height; under it, GNOME Podcasts' four (`read_more_label.rs`).
const WIDE_LINES: i32 = 6;
const NARROW_LINES: i32 = 4;
/// The Read More link's address, handled here rather than opened.
const READ_MORE: &str = "beacon:read-more";
/// Enough lines for any description, to measure all of it.
const ALL_LINES: i32 = 10_000;

/// The hero's widgets, shared with the handlers that fit it to its width.
struct Parts {
    cover: Cover,
    row: gtk::Box,
    text: gtk::Box,
    category: gtk::Label,
    title: gtk::Label,
    author: gtk::Label,
    description: gtk::Label,
    read_more: gtk::Label,
    narrow: Cell<bool>,
    /// A fit waits for the next turn of the main loop.
    fitting: Cell<bool>,
    show_title: RefCell<String>,
    markup: RefCell<String>,
}

pub struct ShowHero {
    root: gtk::Overlay,
    parts: Rc<Parts>,
    pub unsubscribe: gtk::Button,
}

impl ShowHero {
    pub fn new() -> Self {
        let cover = Cover::new(COVER_SIZE);
        cover.widget().set_valign(gtk::Align::Start);
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
        let description = label(&[], WIDE_LINES);
        description.set_margin_top(4);
        description.set_use_markup(true);
        links::selectable_description(&description);

        // Opens the whole description when it does not fit, as GNOME
        // Podcasts' Read More does; in a dialog rather than in place, so the
        // header keeps the cover's height. A link on one line under the text
        // rather than a button, to leave the description more room (asked by
        // the user; the HIG says nothing on links). It cannot end the
        // description's last line: the ellipsis would cut it off.
        let read_more = gtk::Label::builder()
            .use_markup(true)
            .label(format!(
                "<a href=\"{READ_MORE}\">{}</a>",
                glib::markup_escape_text(&gettext("Read More"))
            ))
            .xalign(0.0)
            .visible(false)
            .build();

        let unsubscribe = gtk::Button::builder()
            .label(gettext("_Unsubscribe"))
            .use_underline(true)
            .halign(gtk::Align::Start)
            .margin_top(6)
            .build();

        let text = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .hexpand(true)
            .valign(gtk::Align::Start)
            .build();
        for part in [&category, &title, &author, &description] {
            text.append(part);
        }
        text.append(&read_more);
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

        let parts = Rc::new(Parts {
            cover,
            row,
            text,
            category,
            title,
            author,
            description,
            read_more,
            narrow: Cell::new(false),
            fitting: Cell::new(false),
            show_title: RefCell::default(),
            markup: RefCell::default(),
        });
        let weak = Rc::downgrade(&parts);
        probe.connect_resize(move |_, width, _| {
            if let Some(parts) = weak.upgrade() {
                parts.narrow.set(f64::from(width) < NARROW_WIDTH);
                Parts::fit_later(&parts);
            }
        });
        let weak = Rc::downgrade(&parts);
        parts.read_more.connect_activate_link(move |label, uri| {
            if uri != READ_MORE {
                return glib::Propagation::Proceed;
            }
            if let Some(parts) = weak.upgrade() {
                description::present(label, &parts.show_title.borrow(), &parts.markup.borrow());
            }
            glib::Propagation::Stop
        });

        Self {
            root,
            parts,
            unsubscribe,
        }
    }

    pub fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    pub fn update(&self, data: &ShowPageData) {
        let info = &data.show.info;
        let parts = &self.parts;
        parts.cover.set(data.show.cover_path.as_deref());
        set_text(&parts.category, info.category.as_deref());
        parts.title.set_label(&info.title);
        set_text(&parts.author, info.author.as_deref());
        let markup = info
            .description
            .as_deref()
            .map(text::markup)
            .unwrap_or_default();
        parts.description.set_markup(&markup);
        parts.description.set_visible(!markup.is_empty());
        parts.show_title.replace(info.title.clone());
        parts.markup.replace(markup);
        Parts::fit_later(parts);
    }
}

impl Parts {
    /// Fit the hero on the next turn of the main loop: not while it is being
    /// allocated, and once for several changes.
    fn fit_later(parts: &Rc<Self>) {
        if parts.fitting.replace(true) {
            return;
        }
        let weak = Rc::downgrade(parts);
        glib::idle_add_local_once(move || {
            if let Some(parts) = weak.upgrade() {
                parts.fitting.set(false);
                parts.fit();
            }
        });
    }

    /// Lay out for the width: beside the cover, the text keeps to the
    /// cover's height, the description losing lines until it does; under
    /// it, the description stops at four lines. Read More shows when the
    /// description is cut short.
    fn fit(&self) {
        let narrow = self.narrow.get();
        if narrow {
            self.row.set_orientation(gtk::Orientation::Vertical);
            self.row.set_spacing(12);
            self.cover.set_size(NARROW_COVER_SIZE);
        } else {
            self.row.set_orientation(gtk::Orientation::Horizontal);
            self.row.set_spacing(24);
            self.cover.set_size(COVER_SIZE);
        }
        let width = if narrow {
            self.row.width()
        } else {
            self.row.width() - COVER_SIZE - 24
        };
        if width <= 0 || !self.description.is_visible() {
            self.read_more.set_visible(false);
            return;
        }
        let height = |widget: &gtk::Label| widget.measure(gtk::Orientation::Vertical, width).1;
        // Not -1: GTK passes it to Pango, which takes it as one line.
        self.description.set_lines(ALL_LINES);
        let whole = height(&self.description);
        let mut lines = if narrow { NARROW_LINES } else { WIDE_LINES };
        self.description.set_lines(lines);
        if !narrow {
            // Measure with Read More showing: it shows whenever lines go.
            self.read_more.set_visible(true);
            while lines > 1 && self.text.measure(gtk::Orientation::Vertical, width).1 > COVER_SIZE {
                lines -= 1;
                self.description.set_lines(lines);
            }
        }
        self.read_more
            .set_visible(height(&self.description) < whole);
    }
}

fn set_text(label: &gtk::Label, text: Option<&str>) {
    let text = text.unwrap_or_default();
    label.set_label(text);
    label.set_visible(!text.is_empty());
}
