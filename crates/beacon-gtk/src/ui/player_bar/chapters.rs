// SPDX-License-Identifier: GPL-3.0-or-later

//! The player bar's Chapters button (asked by the user, 2026-11-08, in
//! place of Favourite): a popover listing the playing episode's chapters,
//! the one playing grey with its time in the accent colour, choosing one
//! seeks to it; and a tick on
//! the seek bar where each chapter starts.
//!
//! A popover, as the HIG gives "a list of content items that can be
//! opened" as popover content, rather than a menu, which "should contain
//! between three and twelve items" where episodes often have more chapters.
//! The list scrolls, so the popover stays small ("they should not cover
//! more than a third of their parent window"). GNOME Podcasts pushes a
//! Chapters page instead (`chapters.ui`). The ticks follow the HIG's
//! sliders: "it is helpful to mark significant values along the length of
//! the slider with text or tick marks".

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beacon_core::model::Chapter;
use beacon_core::playback;
use gettextrs::gettext;
use relm4::ComponentSender;
use relm4::gtk::{self, prelude::*};

use super::{PlayerBar, PlayerBarOutput};
use crate::ui::chapter_list;

/// The popover's list is at most this tall, scrolling beyond it, and at
/// most a third of the window's height, but never shorter than
/// `LIST_HEIGHT_LEAST` (about two rows of two-line titles).
const LIST_HEIGHT: i32 = 320;
const LIST_HEIGHT_LEAST: i32 = 160;
const LIST_WIDTH: i32 = 300;

pub(super) struct ChapterMenu {
    pub(super) button: gtk::MenuButton,
    list: gtk::ListBox,
    chapters: Rc<RefCell<Vec<Chapter>>>,
    current: Cell<Option<usize>>,
}

impl ChapterMenu {
    pub(super) fn new(sender: &ComponentSender<PlayerBar>) -> Self {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["chapter-list"])
            .build();
        list.update_property(&[gtk::accessible::Property::Label(&gettext("Chapters"))]);
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(LIST_HEIGHT)
            .width_request(LIST_WIDTH)
            .child(&list)
            .build();
        let popover = gtk::Popover::builder().child(&scrolled).build();
        let button = gtk::MenuButton::builder()
            .icon_name("view-list-ordered-symbolic")
            .popover(&popover)
            .valign(gtk::Align::Center)
            .css_classes(["flat", "circular"])
            .build();
        button.update_property(&[gtk::accessible::Property::Label(&gettext("Chapters"))]);

        let chapters: Rc<RefCell<Vec<Chapter>>> = Rc::default();
        let (sender, shown) = (sender.clone(), chapters.clone());
        let closing = popover.clone();
        list.connect_row_activated(move |_, row| {
            let start = usize::try_from(row.index())
                .ok()
                .and_then(|i| shown.borrow().get(i).map(|c| c.start));
            if let Some(start) = start {
                closing.popdown();
                let _ = sender.output(PlayerBarOutput::Seek(start));
            }
        });
        // Open on the chapter playing, so it is in view and has focus.
        let opened = list.clone();
        let sized = scrolled.clone();
        popover.connect_show(move |popover| {
            if let Some(window) = popover.root() {
                sized.set_max_content_height(
                    (window.height() / 3).clamp(LIST_HEIGHT_LEAST, LIST_HEIGHT),
                );
            }
            let current = (0..)
                .map_while(|i| opened.row_at_index(i))
                .find(|row| row.has_css_class("current-chapter"));
            if let Some(row) = current.or_else(|| opened.row_at_index(0)) {
                row.grab_focus();
            }
        });

        let menu = Self {
            button,
            list,
            chapters,
            current: Cell::new(None),
        };
        menu.set(&[], None);
        menu
    }

    /// Show `chapters`, the playing episode's, and tick where each starts
    /// on `seek`; none leaves the button unavailable.
    pub(super) fn set(&self, chapters: &[Chapter], seek: Option<&gtk::Scale>) {
        self.chapters.replace(chapters.to_vec());
        self.current.set(None);
        chapter_list::fill(&self.list, chapters);
        let any = !chapters.is_empty();
        self.button.set_sensitive(any);
        self.button.set_tooltip_text(Some(&if any {
            gettext("Chapters")
        } else {
            gettext("No Chapters")
        }));
        if let Some(seek) = seek {
            self.tick(seek);
        }
    }

    /// Tick where each chapter starts on `seek`. Called again whenever its
    /// range changes: a scale keeps a mark where it fell when added, so one
    /// added before the episode's length is known stays at the end.
    pub(super) fn tick(&self, seek: &gtk::Scale) {
        seek.clear_marks();
        // Not at the very start: the bar's own end marks it.
        for chapter in self.chapters.borrow().iter().filter(|c| !c.start.is_zero()) {
            seek.add_mark(chapter.start.as_secs_f64(), gtk::PositionType::Bottom, None);
        }
    }

    #[cfg(test)]
    pub(super) fn list(&self) -> gtk::ListBox {
        self.list.clone()
    }

    /// Show which chapter plays at `position`.
    pub(super) fn mark(&self, position: std::time::Duration) {
        let current = playback::current_chapter(&self.chapters.borrow(), position);
        if current == self.current.get() {
            return;
        }
        for (index, mark) in [(self.current.get(), false), (current, true)] {
            if let Some(row) = index
                .and_then(|i| i32::try_from(i).ok())
                .and_then(|i| self.list.row_at_index(i))
            {
                chapter_list::mark(&row, mark);
            }
        }
        self.current.set(current);
    }
}
