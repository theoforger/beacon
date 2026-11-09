// SPDX-License-Identifier: GPL-3.0-or-later

//! Which episodes of a page are selected, and whether the page is in
//! selection mode (the HIG's Selection & Edit Modes). One per page with
//! episode lists; its rows and its selection bar follow it.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use beacon_core::model::EpisodeId;
use gtk::glib;
use gtk::prelude::*;

use crate::navigation::Location;
use crate::ui::models::EpisodeItem;
use crate::ui::widgets::episode_row::EpisodeRow;

/// Called after each change.
type Listener = Rc<dyn Fn(&Selection)>;

#[derive(Default)]
struct Inner {
    active: Cell<bool>,
    /// Every episode the page lists, in order, rows not added yet too.
    items: RefCell<Vec<EpisodeItem>>,
    chosen: RefCell<HashSet<EpisodeId>>,
    rows: RefCell<Vec<glib::WeakRef<EpisodeRow>>>,
    listeners: RefCell<Vec<Listener>>,
    /// The section selected in, where one selection serves several (see
    /// `set_scope`), and each section's items.
    scope: Cell<Option<Location>>,
    scoped: RefCell<HashMap<Location, Vec<EpisodeItem>>>,
}

/// A page's selection, shared by its rows and its selection bar.
#[derive(Clone, Default)]
pub struct Selection {
    inner: Rc<Inner>,
}

impl std::fmt::Debug for Selection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Selection")
            .field("active", &self.is_active())
            .field("count", &self.count())
            .finish_non_exhaustive()
    }
}

impl Selection {
    /// Whether the page is in selection mode.
    pub fn is_active(&self) -> bool {
        self.inner.active.get()
    }

    pub fn contains(&self, id: EpisodeId) -> bool {
        self.inner.chosen.borrow().contains(&id)
    }

    pub fn count(&self) -> usize {
        self.inner.chosen.borrow().len()
    }

    /// Whether every episode the page lists is selected.
    pub fn all_selected(&self) -> bool {
        let items = self.inner.items.borrow();
        !items.is_empty() && self.count() == items.len()
    }

    /// Enter selection mode, or leave it, which selects nothing.
    pub fn set_active(&self, active: bool) {
        if active == self.is_active() {
            return;
        }
        self.inner.active.set(active);
        if !active {
            self.inner.chosen.borrow_mut().clear();
        }
        self.changed();
    }

    /// Select an episode, or unselect it. Selecting enters selection mode,
    /// as the HIG asks: "When one or more items are selected, selection
    /// mode should be active."
    pub fn set(&self, id: EpisodeId, selected: bool) {
        let changed = {
            let mut chosen = self.inner.chosen.borrow_mut();
            if selected {
                chosen.insert(id)
            } else {
                chosen.remove(&id)
            }
        };
        if selected {
            self.inner.active.set(true);
        }
        if changed || selected {
            self.changed();
        }
    }

    pub fn toggle(&self, id: EpisodeId) {
        self.set(id, !self.contains(id));
    }

    /// Select every episode the page lists, entering selection mode.
    pub fn select_all(&self) {
        let ids = self
            .inner
            .items
            .borrow()
            .iter()
            .map(|i| i.episode.id)
            .collect();
        self.inner.chosen.replace(ids);
        self.inner.active.set(true);
        self.changed();
    }

    /// Select nothing, staying in selection mode.
    pub fn select_none(&self) {
        self.inner.chosen.borrow_mut().clear();
        self.changed();
    }

    /// The episodes the page lists now. Selected episodes no longer listed
    /// are unselected; the others keep their selection with their new state.
    pub fn set_items(&self, items: Vec<EpisodeItem>) {
        {
            let listed: HashSet<_> = items.iter().map(|i| i.episode.id).collect();
            self.inner
                .chosen
                .borrow_mut()
                .retain(|id| listed.contains(id));
        }
        self.inner.items.replace(items);
        if self.is_active() {
            self.changed();
        }
    }

    /// One selection serves the sections under the content header (New,
    /// Queue, Favorites, Downloaded), whose selection bar is one: each sets
    /// its items for itself, and the section showing is the one selected
    /// in. Showing another leaves selection mode.
    pub fn set_scope(&self, scope: Location) {
        self.set_active(false);
        self.inner.scope.set(Some(scope));
        let items = self
            .inner
            .scoped
            .borrow()
            .get(&scope)
            .cloned()
            .unwrap_or_default();
        self.set_items(items);
    }

    /// `scope`'s items, taking effect when it is the section showing.
    pub fn set_items_for(&self, scope: Location, items: Vec<EpisodeItem>) {
        if self.inner.scope.get() == Some(scope) {
            self.set_items(items.clone());
        }
        self.inner.scoped.borrow_mut().insert(scope, items);
    }

    /// The selected episodes, in the page's order.
    pub fn selected(&self) -> Vec<EpisodeItem> {
        let chosen = self.inner.chosen.borrow();
        self.inner
            .items
            .borrow()
            .iter()
            .filter(|i| chosen.contains(&i.episode.id))
            .cloned()
            .collect()
    }

    /// Keep `row` showing whether it is selected.
    pub fn watch(&self, row: &EpisodeRow) {
        let mut rows = self.inner.rows.borrow_mut();
        // Rows come and go as lists are rebuilt; drop the gone ones
        // whenever the list doubles.
        if rows.len() >= 64 && rows.len().is_power_of_two() {
            rows.retain(|row| row.upgrade().is_some());
        }
        rows.push(row.downgrade());
    }

    /// Call `f` after every change.
    pub fn connect_changed(&self, f: impl Fn(&Selection) + 'static) {
        self.inner.listeners.borrow_mut().push(Rc::new(f));
    }

    fn changed(&self) {
        let rows: Vec<_> = self
            .inner
            .rows
            .borrow()
            .iter()
            .filter_map(glib::WeakRef::upgrade)
            .collect();
        for row in rows {
            row.show_selection();
        }
        let listeners = self.inner.listeners.borrow().clone();
        for listener in listeners {
            listener(self);
        }
    }
}

#[cfg(test)]
mod tests {
    use beacon_core::model::{Episode, EpisodeInfo, EpisodeState, ShowId};
    use chrono::Utc;

    use super::*;

    fn item(id: i64) -> EpisodeItem {
        EpisodeItem {
            episode: Episode {
                id: EpisodeId(id),
                show_id: ShowId(1),
                info: EpisodeInfo::default(),
                state: EpisodeState::default(),
                added_at: Utc::now(),
            },
            show_title: String::new(),
            cover: None,
            in_queue: false,
        }
    }

    fn ids(selection: &Selection) -> Vec<i64> {
        selection
            .selected()
            .iter()
            .map(|i| i.episode.id.0)
            .collect()
    }

    #[test]
    fn selecting_enters_selection_mode_and_cancelling_clears_it() {
        let selection = Selection::default();
        selection.set_items(vec![item(1), item(2), item(3)]);
        assert!(!selection.is_active());

        selection.toggle(EpisodeId(3));
        selection.toggle(EpisodeId(1));
        assert!(selection.is_active());
        // In the page's order, not the order chosen.
        assert_eq!(ids(&selection), [1, 3]);

        // Unselecting the last keeps selection mode, until it is left.
        selection.toggle(EpisodeId(1));
        selection.toggle(EpisodeId(3));
        assert!(selection.is_active());
        selection.set_active(false);
        selection.set_active(true);
        assert_eq!(selection.count(), 0);
    }

    #[test]
    fn select_all_covers_every_episode_and_reloads_keep_what_is_listed() {
        let selection = Selection::default();
        selection.set_items(vec![item(1), item(2), item(3)]);
        selection.select_all();
        assert!(selection.is_active() && selection.all_selected());

        // A filter hides episode 2: it is unselected; 1 and 3 stay.
        selection.set_items(vec![item(1), item(3)]);
        assert_eq!(ids(&selection), [1, 3]);
        selection.select_none();
        assert_eq!(selection.count(), 0);
        assert!(selection.is_active());
    }

    #[test]
    fn each_section_selects_in_its_own_items() {
        let selection = Selection::default();
        selection.set_scope(Location::New);
        selection.set_items_for(Location::New, vec![item(1), item(2)]);
        // The Queue's items wait until it shows.
        selection.set_items_for(Location::Queue, vec![item(3)]);
        selection.select_all();
        assert_eq!(ids(&selection), [1, 2]);

        // Another section: selection mode ends, and its items are selected in.
        selection.set_scope(Location::Queue);
        assert!(!selection.is_active());
        selection.select_all();
        assert_eq!(ids(&selection), [3]);
        selection.set_items_for(Location::New, vec![item(1)]);
        assert_eq!(ids(&selection), [3], "New's update leaves the Queue alone");
    }
}
