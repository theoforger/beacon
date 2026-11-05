// SPDX-License-Identifier: GPL-3.0-or-later

//! Episode lists as `boxed-list` `GtkListBox`es, as in GNOME Podcasts, and
//! the ways of keeping long lists from slowing the UI: rows are added a page
//! at a time as the list is scrolled, each page a little at a time at idle
//! priority (GNOME Podcasts' `lazy_load`), and a list whose episodes are
//! unchanged is updated in place, only changed rows bound again.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

use beacon_core::model::EpisodeId;
use relm4::gtk::{self, glib, prelude::*};

use crate::ui::models::{EpisodeItem, RowAction};
use crate::ui::widgets::episode_row::{EpisodeRow, RowSetup};

/// Time spent adding rows before the main loop gets a turn, well inside a
/// 60 Hz frame so scrolling and clicks stay smooth while a long feed fills.
const BUDGET: Duration = Duration::from_millis(8);

/// A boxed list whose rows open their episode's details when activated,
/// or in selection mode select it ("Clicking anywhere on a content item
/// should toggle its selection state"), named `label` for screen readers
/// (the group or season it holds): its heading is a label, which focus
/// never reaches, so the list says it.
pub fn new_list(setup: &RowSetup, label: &str) -> gtk::ListBox {
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        // Tab visits the focused row's buttons, then leaves the list; the
        // arrow keys move between rows. With the default, Tab walked every
        // row of a long feed (GNOME Podcasts sets the same on its grid).
        .tab_behavior(gtk::ListTabBehavior::Item)
        .css_classes(["boxed-list"])
        .build();
    list.update_property(&[gtk::accessible::Property::Label(label)]);
    let setup = setup.clone();
    list.connect_row_activated(move |_, row| {
        let Some(item) = row.downcast_ref::<EpisodeRow>().and_then(EpisodeRow::item) else {
            return;
        };
        if setup.selection.is_active() {
            setup.selection.toggle(item.episode.id);
        } else {
            (setup.on_action)(&item, RowAction::ShowDetails);
        }
    });
    list
}

/// The episode rows of `lists`, in order.
fn rows(lists: &[gtk::ListBox]) -> Vec<EpisodeRow> {
    let mut rows = Vec::new();
    for list in lists {
        let mut child = list.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if let Ok(row) = widget.downcast::<EpisodeRow>() {
                rows.push(row);
            }
        }
    }
    rows
}

/// If `lists` and the rows still to come already hold exactly these
/// episodes in this order, bring them up to date and return true: rows whose
/// episode changed are bound again, the others left alone, and the rows
/// still to come take the new items. Otherwise change nothing.
pub fn update_in_place(lists: &[gtk::ListBox], items: &[&EpisodeItem], fill: &LazyFill) -> bool {
    let rows = rows(lists);
    if rows.len() + fill.pending() != items.len()
        || !rows
            .iter()
            .zip(items)
            .all(|(row, item)| row.item().map(|i| i.episode.id) == Some(item.episode.id))
    {
        return false;
    }
    for (row, item) in rows.iter().zip(items) {
        if row.item().as_ref() != Some(*item) {
            row.bind(item);
        }
    }
    fill.replace_pending(items[rows.len()..].iter().map(|item| (*item).clone()));
    true
}

/// Rows added at a time: about two screens. A `GtkListBox` lays out every
/// row it holds, so a long feed gets its rows a page at a time, as the list
/// is scrolled towards its end.
const PAGE: usize = 40;

#[derive(Default)]
struct Fill {
    /// Bumped by each new fill, ending the one before.
    generation: Cell<u64>,
    /// Rows still to add, with the list each goes in.
    pending: RefCell<VecDeque<(gtk::ListBox, EpisodeItem)>>,
    /// Rows that may be added before the list is scrolled further.
    allowance: Cell<usize>,
    running: Cell<bool>,
    setup: RefCell<Option<RowSetup>>,
}

/// Fills lists with rows a page at a time, each page a little at a time so
/// the main loop keeps its frames. Starting a new fill ends the one before.
#[derive(Default, Clone)]
pub struct LazyFill {
    inner: Rc<Fill>,
}

impl std::fmt::Debug for LazyFill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LazyFill")
            .field("pending", &self.pending())
            .finish_non_exhaustive()
    }
}

impl LazyFill {
    /// Add a row for each item to its list, the first page now.
    pub fn start(&self, work: Vec<(gtk::ListBox, EpisodeItem)>, setup: RowSetup) {
        let fill = &self.inner;
        fill.generation.set(fill.generation.get() + 1);
        fill.pending.replace(work.into());
        fill.setup.replace(Some(setup));
        fill.allowance.set(PAGE);
        fill.running.set(false);
        self.run();
    }

    /// Add the next page of rows, if there are any.
    pub fn more(&self) {
        if self.pending() == 0 {
            return;
        }
        self.inner
            .allowance
            .set(self.inner.allowance.get().max(PAGE));
        self.run();
    }

    /// Rows not added yet.
    pub fn pending(&self) -> usize {
        self.inner.pending.borrow().len()
    }

    /// Give the rows not added yet new items, keeping the lists they go in.
    fn replace_pending(&self, items: impl Iterator<Item = EpisodeItem>) {
        for ((_, item), new) in self.inner.pending.borrow_mut().iter_mut().zip(items) {
            *item = new;
        }
    }

    /// Stop any fill in progress.
    pub fn cancel(&self) {
        let fill = &self.inner;
        fill.generation.set(fill.generation.get() + 1);
        fill.pending.borrow_mut().clear();
        fill.running.set(false);
    }

    /// Add more rows whenever `scrolled` comes within a screen of its end,
    /// or its content does not fill it yet.
    pub fn follow(&self, scrolled: &gtk::ScrolledWindow) {
        let adjustment = scrolled.vadjustment();
        let check = {
            let fill = self.clone();
            move |adjustment: &gtk::Adjustment| {
                let end = adjustment.upper() - adjustment.page_size();
                if adjustment.value() >= end - adjustment.page_size() {
                    fill.more();
                }
            }
        };
        adjustment.connect_value_changed(check.clone());
        adjustment.connect_changed(check);
    }

    fn run(&self) {
        let fill = self.inner.clone();
        if fill.running.replace(true) {
            return;
        }
        let generation = fill.generation.get();
        glib::spawn_future_local(async move {
            let started = Instant::now();
            loop {
                if fill.generation.get() != generation {
                    return;
                }
                let turn = Instant::now();
                // At least one row a turn, however slow the machine.
                while fill.allowance.get() > 0 {
                    let Some((list, item)) = fill.pending.borrow_mut().pop_front() else {
                        break;
                    };
                    let Some(setup) = fill.setup.borrow().clone() else {
                        break;
                    };
                    let row = EpisodeRow::new(&setup);
                    row.bind(&item);
                    list.append(&row);
                    fill.allowance.set(fill.allowance.get() - 1);
                    if turn.elapsed() >= BUDGET {
                        break;
                    }
                }
                if fill.allowance.get() == 0 || fill.pending.borrow().is_empty() {
                    fill.running.set(false);
                    tracing::debug!(
                        "added a page of episode rows in {:?}, {} to come",
                        started.elapsed(),
                        fill.pending.borrow().len()
                    );
                    return;
                }
                glib::timeout_future_with_priority(glib::Priority::DEFAULT_IDLE, Duration::ZERO)
                    .await;
            }
        });
    }
}

/// The ids of a list of items, for comparing structure.
pub fn ids<'a>(items: impl IntoIterator<Item = &'a EpisodeItem>) -> Vec<EpisodeId> {
    items.into_iter().map(|i| i.episode.id).collect()
}
