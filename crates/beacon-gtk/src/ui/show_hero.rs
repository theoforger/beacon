// SPDX-License-Identifier: GPL-3.0-or-later

//! The top of a podcast page, laid out as in the mockup: the cover, then
//! the category, title, author, description and Unsubscribe (or, on a
//! show's preview, the accent Subscribe) beside it, stacked under it when
//! the page is narrow. More Options is in the page's
//! header bar; sorting and filtering are in its filter menu.

use beacon_core::text;
use gettextrs::gettext;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{self, glib, pango};

use crate::ui::covers::Cover;
use crate::ui::dialogs::description;
use crate::ui::links;
use crate::ui::models::ShowPageData;

/// What the hero shows: a stored show's, or a previewed feed's.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeroData {
    pub cover: Option<std::path::PathBuf>,
    pub category: Option<String>,
    pub title: String,
    pub author: Option<String>,
    /// The feed's description, HTML or text.
    pub description: Option<String>,
}

impl From<&ShowPageData> for HeroData {
    fn from(data: &ShowPageData) -> Self {
        let info = &data.show.info;
        Self {
            cover: data.show.cover_path.clone(),
            category: info.category.clone(),
            title: info.title.clone(),
            author: info.author.clone(),
            description: info.description.clone(),
        }
    }
}

/// The cover beside the text, and above it when narrow: GNOME Music's large
/// and medium album art (`ArtSize`), which gives the text room beside it.
const COVER_SIZE: i32 = 256;
const NARROW_COVER_SIZE: i32 = 192;
/// Between the cover and the text, as on GNOME Music's album page.
const GAP: i32 = 32;
/// Below this width the text goes under the cover: the text column wants
/// about 320px beside the cover and the gap.
const NARROW_WIDTH: f64 = 610.0;

/// The most description lines: beside the cover, as many of these as fit
/// its height; under it, GNOME Podcasts' four (`read_more_label.rs`).
const WIDE_LINES: i32 = 6;
const NARROW_LINES: i32 = 4;
/// The Read More link's address, handled here rather than opened.
const READ_MORE: &str = "beacon:read-more";

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
    unsubscribe: gtk::Button,
    subscribe: gtk::Button,
    narrow: Cell<bool>,
    /// The layout last set up (stacked or not), and the width the
    /// description was last fitted to: re-aligning every label, or fitting
    /// a long description again, on each step of a window resize made the
    /// page lag (found by the user on Destination Linux, 2026-11-08).
    laid_out: Cell<Option<bool>>,
    fitted: Cell<i32>,
    /// A fit waits for the next turn of the main loop.
    fitting: Cell<bool>,
    show_title: RefCell<String>,
    markup: RefCell<String>,
}

pub struct ShowHero {
    root: gtk::Overlay,
    parts: Rc<Parts>,
    pub unsubscribe: gtk::Button,
    pub subscribe: gtk::Button,
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
                // The page sets the width; asking each label how wide it
                // would be unwrapped laid its whole text out again on every
                // step of a resize.
                .natural_wrap_mode(gtk::NaturalWrapMode::None)
                .css_classes(classes)
                .build()
        };
        let category = label(&["dim-label", "caption-heading"], 1);
        let title = label(&["title-1"], 3);
        title.set_accessible_role(gtk::AccessibleRole::Heading);
        let author = label(&["dim-label"], 2);
        let description = label(&[], WIDE_LINES);
        description.set_margin_top(6);
        description.set_use_markup(true);
        links::arrow_keys_between_links(&description);

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
            .margin_top(12)
            .build();
        // The accent button, on a preview, where Unsubscribe is once
        // subscribed.
        let subscribe = gtk::Button::builder()
            .label(gettext("_Subscribe"))
            .use_underline(true)
            .halign(gtk::Align::Start)
            .margin_top(12)
            .css_classes(["suggested-action"])
            .visible(false)
            .build();

        let text = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            // GNOME Software's spacing between the lines of its app header.
            .spacing(6)
            .hexpand(true)
            .valign(gtk::Align::Start)
            .build();
        for part in [&category, &title, &author, &description] {
            text.append(part);
        }
        text.append(&read_more);
        text.append(&unsubscribe);
        text.append(&subscribe);

        let row = gtk::Box::builder()
            .spacing(GAP)
            // GNOME Podcasts' 32px above its show header.
            .margin_top(32)
            .margin_bottom(12)
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
            unsubscribe: unsubscribe.clone(),
            subscribe: subscribe.clone(),
            narrow: Cell::new(false),
            laid_out: Cell::new(None),
            fitted: Cell::new(0),
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
            subscribe,
        }
    }

    /// The description's line limit, whether Read More shows, and the text
    /// column's height, for tests.
    #[cfg(test)]
    pub fn fitted(&self) -> (i32, bool, i32) {
        let parts = &self.parts;
        (
            parts.description.lines(),
            parts.read_more.is_visible(),
            parts.text.height(),
        )
    }

    /// The large title, which the page's header title stands in for once
    /// it has scrolled away.
    pub fn title(&self) -> &gtk::Label {
        &self.parts.title
    }

    pub fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    /// Subscribe on a preview, Unsubscribe on a stored show's page.
    pub fn set_subscribed(&self, subscribed: bool) {
        self.unsubscribe.set_visible(subscribed);
        self.subscribe.set_visible(!subscribed);
    }

    pub fn update(&self, info: &HeroData) {
        let parts = &self.parts;
        parts.cover.set(info.cover.as_deref());
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
        parts.fitted.set(0);
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

    /// Stacked, everything is centred under the cover (asked by the user);
    /// beside it, everything starts at the cover's edge.
    fn align(&self, centred: bool) {
        let (align, xalign, justify) = if centred {
            (gtk::Align::Center, 0.5, gtk::Justification::Center)
        } else {
            (gtk::Align::Start, 0.0, gtk::Justification::Left)
        };
        self.cover.widget().set_halign(align);
        for label in [
            &self.category,
            &self.title,
            &self.author,
            &self.description,
            &self.read_more,
        ] {
            label.set_xalign(xalign);
            label.set_justify(justify);
        }
        self.read_more.set_halign(align);
        self.unsubscribe.set_halign(align);
        self.subscribe.set_halign(align);
    }

    /// Lay out for the width: beside the cover, the text keeps to the
    /// cover's height, the description losing lines until it does; under
    /// it, the description stops at four lines. Read More shows when the
    /// description is cut short.
    fn fit(&self) {
        let narrow = self.narrow.get();
        if self.laid_out.replace(Some(narrow)) != Some(narrow) {
            if narrow {
                self.row.set_orientation(gtk::Orientation::Vertical);
                self.row.set_spacing(12);
                self.cover.set_size(NARROW_COVER_SIZE);
            } else {
                self.row.set_orientation(gtk::Orientation::Horizontal);
                self.row.set_spacing(GAP);
                self.cover.set_size(COVER_SIZE);
            }
            self.align(narrow);
            self.fitted.set(0);
        }
        let width = if narrow {
            self.row.width()
        } else {
            self.row.width() - COVER_SIZE - GAP
        };
        if width <= 0 || !self.description.is_visible() {
            self.read_more.set_visible(false);
            return;
        }
        if self.fitted.replace(width) == width {
            return;
        }
        // Measured on a layout of its own: setting the label's lines to
        // measure it, as this did, threw away its cached layout, so every
        // frame of a resize laid the whole description out again.
        let bottoms = self.line_bottoms(width);
        let total = bottoms.len() as i32;
        let (lines, cut) = if narrow {
            (NARROW_LINES, total > NARROW_LINES)
        } else {
            // The rest of the text beside the cover, without the
            // description and Read More: what the description's lines and
            // Read More have to share the cover's height with.
            let measure =
                |widget: &gtk::Widget| widget.measure(gtk::Orientation::Vertical, width).1;
            let read_more = measure(self.read_more.upcast_ref()) + self.text.spacing();
            let rest = measure(self.text.upcast_ref())
                - measure(self.description.upcast_ref())
                - if self.read_more.is_visible() {
                    read_more
                } else {
                    0
                };
            let margin = self.description.margin_top();
            let fits = |lines: i32, extra: i32| {
                rest + extra + margin + bottoms[(lines.min(total) - 1).max(0) as usize]
                    <= COVER_SIZE
            };
            if total <= WIDE_LINES && fits(total, 0) {
                (WIDE_LINES, false)
            } else {
                let lines = (1..=WIDE_LINES.min(total))
                    .rev()
                    .find(|&lines| fits(lines, read_more))
                    .unwrap_or(1);
                (lines, lines < total)
            }
        };
        // Both are no-ops when unchanged, which leaves the cached layout.
        self.description.set_lines(lines);
        self.read_more.set_visible(cut);
    }

    /// Where each of the description's lines ends at `width`, in pixels.
    fn line_bottoms(&self, width: i32) -> Vec<i32> {
        let layout = self.description.create_pango_layout(None);
        layout.set_markup(&links::without_links(&self.markup.borrow()));
        layout.set_width(width * pango::SCALE);
        layout.set_wrap(pango::WrapMode::WordChar);
        let mut bottoms = Vec::new();
        let mut iter = layout.iter();
        loop {
            let (_, bottom) = iter.line_yrange();
            bottoms.push(pango::units_to_double(bottom).ceil() as i32);
            if !iter.next_line() {
                break;
            }
        }
        bottoms
    }
}

fn set_text(label: &gtk::Label, text: Option<&str>) {
    let text = text.unwrap_or_default();
    label.set_label(text);
    label.set_visible(!text.is_empty());
}
