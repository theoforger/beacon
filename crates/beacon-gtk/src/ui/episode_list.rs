// SPDX-License-Identifier: GPL-3.0-or-later

//! Episode lists as `boxed-list` `GtkListBox`es, as in GNOME Podcasts, and
//! its way of keeping long lists from blocking the UI: rows are added a few
//! at a time at idle priority (GNOME Podcasts' `lazy_load`), and a list
//! whose episodes are unchanged is updated in place instead of rebuilt.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use beacon_core::model::EpisodeId;
use relm4::gtk::{self, glib, prelude::*};

use crate::ui::models::{EpisodeItem, RowAction};
use crate::ui::widgets::episode_row::{EpisodeRow, OnRowAction};

/// Time spent adding rows before the main loop gets a turn, well inside a
/// 60 Hz frame so scrolling and clicks stay smooth while a long feed fills.
const BUDGET: Duration = Duration::from_millis(8);

/// A boxed list whose rows open their episode's details when activated.
pub fn new_list(on_action: &OnRowAction) -> gtk::ListBox {
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    let on_action = on_action.clone();
    list.connect_row_activated(move |_, row| {
        if let Some(item) = row.downcast_ref::<EpisodeRow>().and_then(EpisodeRow::item) {
            on_action(&item, RowAction::ShowDetails);
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

/// If `lists` already show exactly these episodes in this order, rebind
/// their rows to the new state and return true. Otherwise change nothing.
pub fn update_in_place(lists: &[gtk::ListBox], items: &[&EpisodeItem]) -> bool {
    let rows = rows(lists);
    let same = rows.len() == items.len()
        && rows
            .iter()
            .zip(items)
            .all(|(row, item)| row.item().map(|i| i.episode.id) == Some(item.episode.id));
    if same {
        for (row, item) in rows.iter().zip(items) {
            row.bind(item);
        }
    }
    same
}

/// Fills lists with rows a chunk at a time. Starting a new fill cancels the
/// one before.
#[derive(Debug, Default, Clone)]
pub struct LazyFill {
    generation: Rc<Cell<u64>>,
}

impl LazyFill {
    /// Append a row for each item to its list, a chunk at a time, yielding to
    /// the main loop in between.
    pub fn start(&self, work: Vec<(gtk::ListBox, EpisodeItem)>, on_action: OnRowAction) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        let current = self.generation.clone();
        glib::spawn_future_local(async move {
            let (rows, all) = (work.len(), Instant::now());
            let mut work = work.into_iter().peekable();
            while work.peek().is_some() {
                if current.get() != generation {
                    return;
                }
                let started = Instant::now();
                // At least one row a turn, however slow the machine.
                for (list, item) in work.by_ref() {
                    let row = EpisodeRow::new(on_action.clone());
                    row.bind(&item);
                    list.append(&row);
                    if started.elapsed() >= BUDGET {
                        break;
                    }
                }
                glib::timeout_future_with_priority(glib::Priority::DEFAULT_IDLE, Duration::ZERO)
                    .await;
            }
            tracing::debug!("filled {rows} episode rows in {:?}", all.elapsed());
        });
    }

    /// Stop any fill in progress.
    pub fn cancel(&self) {
        self.generation.set(self.generation.get() + 1);
    }
}

/// The ids of a list of items, for comparing structure.
pub fn ids<'a>(items: impl IntoIterator<Item = &'a EpisodeItem>) -> Vec<EpisodeId> {
    items.into_iter().map(|i| i.episode.id).collect()
}
