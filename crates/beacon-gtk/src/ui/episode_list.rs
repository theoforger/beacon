// SPDX-License-Identifier: GPL-3.0-or-later

//! A page's episodes in a recycling list, as the HIG asks of long lists
//! ("For large or dynamic lists, see list views"): only the rows near the
//! screen exist, bound again as the list scrolls, so a feed of thousands of
//! episodes costs what a screenful does. Decided 2026-11-08, departing from
//! GNOME Podcasts' `GtkListBox`es, which laid out every row they held.
//!
//! The list is a one-column `GtkGridView` named as a list for screen
//! readers, not a `GtkListView`: a list view keeps about 200 rows whatever
//! the window's height, a grid view about 30 per column, and every row kept
//! is restyled whenever the narrow layout's split views move the page,
//! which froze the window for most of a second (250ms a list against 40).
//!
//! The list holds the page's whole content, in order: its header (a podcast
//! page's hero), the group and season headings, the episodes and the room
//! between them, since only a view that scrolls itself recycles its rows.
//! Each group's episodes are drawn as a card, as a boxed list (`style.css`).
//! Setting new entries changes only what changed, so the list keeps its
//! place and the rows that did not change are left alone.

use std::cell::{OnceCell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use beacon_core::model::{EpisodeId, ShowId};

use gtk::subclass::prelude::*;
use gtk::{self, gio, glib, pango, prelude::*};

use crate::ui::models::{EpisodeItem, RowAction};
use crate::ui::widgets::episode_row::{EpisodeRow, RowSetup};

/// A season of a show, by number; `None` for its episodes without one.
pub type FoldKey = (ShowId, Option<u32>);

/// One item of a page's list.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    /// The page's header, scrolled with the list.
    Header,
    /// A group's or season's title, `top` pixels below what is above it.
    /// A season's (`fold` set) folds its episodes away.
    Heading {
        title: String,
        fold: Option<FoldKey>,
        top: i32,
    },
    /// An episode; `first` and `last` round its group's card. A `movable`
    /// one, queued, has a drag handle and Move Up and Move Down.
    Episode {
        item: Box<EpisodeItem>,
        first: bool,
        last: bool,
        movable: bool,
    },
    /// A line of text in place of the episodes.
    Message(String),
    /// Room between entries, or after the last.
    Space(i32),
}

/// What an entry is, whatever its state: entries with the same key are the
/// same item of the list, perhaps changed.
#[derive(PartialEq)]
enum Key<'a> {
    Header,
    Heading(&'a str),
    Episode(EpisodeId),
    Message,
    Space,
}

impl Entry {
    fn key(&self) -> Key<'_> {
        match self {
            Entry::Header => Key::Header,
            Entry::Heading { title, .. } => Key::Heading(title),
            Entry::Episode { item, .. } => Key::Episode(item.episode.id),
            Entry::Message(_) => Key::Message,
            Entry::Space(_) => Key::Space,
        }
    }

    /// The episodes of `items` as one card's entries.
    pub fn card<'a>(items: impl IntoIterator<Item = &'a EpisodeItem>) -> Vec<Entry> {
        Self::card_of(items, false)
    }

    /// The queue's episodes as one card's entries, which can be reordered.
    pub fn queue_card<'a>(items: impl IntoIterator<Item = &'a EpisodeItem>) -> Vec<Entry> {
        Self::card_of(items, true)
    }

    fn card_of<'a>(items: impl IntoIterator<Item = &'a EpisodeItem>, movable: bool) -> Vec<Entry> {
        let items: Vec<_> = items.into_iter().collect();
        let count = items.len();
        items
            .into_iter()
            .enumerate()
            .map(|(i, item)| Entry::Episode {
                item: Box::new(item.clone()),
                first: i == 0,
                last: i + 1 == count,
                movable,
            })
            .collect()
    }
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct EntryObject {
        pub entry: OnceCell<Entry>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for EntryObject {
        const NAME: &'static str = "BeaconListEntry";
        type Type = super::EntryObject;
    }

    impl ObjectImpl for EntryObject {}
}

glib::wrapper! {
    /// An entry, as the list's model holds it.
    pub struct EntryObject(ObjectSubclass<imp::EntryObject>);
}

impl EntryObject {
    fn new(entry: Entry) -> Self {
        let object: Self = glib::Object::new();
        let _ = object.imp().entry.set(entry);
        object
    }

    fn entry(&self) -> &Entry {
        self.imp()
            .entry
            .get()
            .expect("every entry object is made with its entry")
    }
}

/// Folded seasons, and what to do when one is folded or unfolded.
#[derive(Clone)]
pub struct Folding {
    pub folded: Rc<RefCell<HashSet<FoldKey>>>,
    pub changed: Rc<dyn Fn()>,
}

pub struct EpisodeList {
    scrolled: gtk::ScrolledWindow,
    store: gio::ListStore,
}

/// How many rows a list keeps around the screen, at most (see the module
/// documentation).
#[cfg(test)]
pub const KEPT_ROWS: usize = 60;

impl std::fmt::Debug for EpisodeList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EpisodeList")
            .field("entries", &self.store.n_items())
            .finish_non_exhaustive()
    }
}

impl EpisodeList {
    /// A list named `label` for screen readers, whose rows act through
    /// `setup`, headed by `header` and folding seasons through `folding`.
    pub fn new(
        setup: &RowSetup,
        label: &str,
        header: Option<gtk::Widget>,
        folding: Option<Folding>,
    ) -> Self {
        let store = gio::ListStore::new::<EntryObject>();
        let factory = factory(setup, header, folding);
        let view = gtk::GridView::builder()
            .model(&gtk::NoSelection::new(Some(store.clone())))
            .factory(&factory)
            .min_columns(1)
            .max_columns(1)
            // Read as the list it is, not a grid of one column.
            .accessible_role(gtk::AccessibleRole::List)
            // Rows open on one click, as the boxed lists' did.
            .single_click_activate(true)
            // Tab visits the focused row's buttons, then leaves the list;
            // the arrow keys move between rows.
            .tab_behavior(gtk::ListTabBehavior::Item)
            .margin_start(12)
            .margin_end(12)
            .css_classes(["episode-list"])
            .build();
        view.update_property(&[gtk::accessible::Property::Label(label)]);
        let setup = setup.clone();
        view.connect_activate(move |view, position| {
            let Some(entry) = view.model().and_then(|m| m.item(position)) else {
                return;
            };
            let Some(entry) = entry.downcast_ref::<EntryObject>() else {
                return;
            };
            let Entry::Episode { item, .. } = entry.entry() else {
                return;
            };
            if setup.selection.is_active() {
                setup.selection.toggle(item.episode.id);
            } else {
                (setup.on_action)(item, RowAction::ShowDetails);
            }
        });
        // As wide as the pages' content always was, centred beyond it; only
        // a view that scrolls itself recycles its rows.
        let clamp = adw::ClampScrollable::builder()
            .maximum_size(900)
            .child(&view)
            .build();
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&clamp)
            .build();
        Self { scrolled, store }
    }

    pub fn widget(&self) -> &gtk::ScrolledWindow {
        &self.scrolled
    }

    /// Show `entries`, changing only what changed (see `sync`).
    pub fn set(&self, entries: Vec<Entry>) {
        sync(&self.store, entries);
    }
}

/// Make `store` hold `entries`. The entries it starts and ends with
/// already stay where they are, replaced only if they changed; the ones
/// between are swapped for the new ones. So a change to one episode
/// replaces one row, and the list keeps its place.
fn sync(store: &gio::ListStore, entries: Vec<Entry>) {
    let old: Vec<EntryObject> = (0..store.n_items())
        .filter_map(|i| store.item(i).and_downcast())
        .collect();
    let (n, m) = (old.len(), entries.len());
    let mut start = 0;
    while start < n.min(m) && old[start].entry().key() == entries[start].key() {
        start += 1;
    }
    let mut end = 0;
    while end < n.min(m) - start && old[n - 1 - end].entry().key() == entries[m - 1 - end].key() {
        end += 1;
    }
    let mut head = entries;
    let tail = head.split_off(m - end);
    let middle = head.split_off(start);
    // From the end, so the old positions still hold.
    let kept = (0..start).zip(head).chain((n - end..n).zip(tail));
    for (i, entry) in kept.rev() {
        if *old[i].entry() != entry {
            store.splice(position(i), 1, &[EntryObject::new(entry)]);
        }
    }
    if n - start - end > 0 || !middle.is_empty() {
        let middle: Vec<EntryObject> = middle.into_iter().map(EntryObject::new).collect();
        store.splice(position(start), position(n - start - end), &middle);
    }
}

fn position(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// Rows for the list's entries: episode rows are kept and bound again as
/// the list scrolls; the few others are made when shown.
fn factory(
    setup: &RowSetup,
    header: Option<gtk::Widget>,
    folding: Option<Folding>,
) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    let setup = setup.clone();
    let shown = header.clone();
    factory.connect_bind(move |_, object| {
        let Some(item) = object.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let Some(entry) = item.item().and_downcast::<EntryObject>() else {
            return;
        };
        // Only episodes are rows to focus and open; the others are text,
        // or hold their own controls.
        let is_episode = matches!(entry.entry(), Entry::Episode { .. });
        item.set_activatable(is_episode);
        item.set_focusable(is_episode);
        item.set_selectable(false);
        match entry.entry() {
            Entry::Episode {
                item: episode,
                first,
                last,
                movable,
            } => {
                let row = item
                    .child()
                    .and_downcast::<EpisodeRow>()
                    .unwrap_or_else(|| {
                        let row = EpisodeRow::new(&setup);
                        item.set_child(Some(&row));
                        row
                    });
                row.bind(episode);
                row.set_edges(*first, *last);
                row.set_movable(*movable, *first, *last);
                // The list item is what has focus, so it carries the name.
                item.set_accessible_label(&row.accessible_name());
            }
            Entry::Header => {
                if let Some(header) = &shown {
                    item.set_child(Some(header));
                }
            }
            Entry::Heading { title, fold, top } => {
                let heading = match (fold, &folding) {
                    (Some(key), Some(folding)) => season(title, *key, folding),
                    _ => heading(title),
                };
                heading.set_margin_top(*top);
                heading.set_margin_bottom(6);
                item.set_child(Some(&heading));
            }
            Entry::Message(text) => {
                let label = gtk::Label::builder()
                    .label(text)
                    .wrap(true)
                    .margin_top(24)
                    .css_classes(["dim-label"])
                    .build();
                item.set_child(Some(&label));
            }
            Entry::Space(height) => {
                item.set_child(Some(&gtk::Box::builder().height_request(*height).build()));
            }
        }
    });
    // The header is one widget: let it go when its entry scrolls away, so
    // it is free to be shown again in another row.
    factory.connect_unbind(move |_, object| {
        if let Some(item) = object.downcast_ref::<gtk::ListItem>()
            && header.is_some()
            && item.child() == header
        {
            item.set_child(None::<&gtk::Widget>);
        }
    });
    factory
}

fn heading_label(title: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .ellipsize(pango::EllipsizeMode::End)
        .css_classes(["heading"])
        .accessible_role(gtk::AccessibleRole::Heading)
        .build()
}

fn heading(title: &str) -> gtk::Widget {
    heading_label(title).upcast()
}

/// A season's title as a `GtkExpander`, which folds its episodes away.
fn season(title: &str, key: FoldKey, folding: &Folding) -> gtk::Widget {
    let expander = gtk::Expander::builder()
        .label_widget(&heading_label(title))
        .expanded(!folding.folded.borrow().contains(&key))
        .build();
    let folding = folding.clone();
    expander.connect_expanded_notify(move |expander| {
        {
            let mut folded = folding.folded.borrow_mut();
            if expander.is_expanded() {
                folded.remove(&key);
            } else {
                folded.insert(key);
            }
        }
        (folding.changed)();
    });
    expander.upcast()
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use beacon_core::model::{Episode, EpisodeInfo, EpisodeState};
    use chrono::DateTime;

    use super::*;

    fn episode(id: i64, played: bool) -> Entry {
        Entry::Episode {
            item: Box::new(EpisodeItem {
                episode: Episode {
                    id: EpisodeId(id),
                    show_id: ShowId(1),
                    info: EpisodeInfo::default(),
                    state: EpisodeState {
                        played,
                        ..Default::default()
                    },
                    added_at: DateTime::UNIX_EPOCH,
                },
                show_title: String::new(),
                cover: None,
                in_queue: false,
            }),
            first: false,
            last: false,
            movable: false,
        }
    }

    fn heading(title: &str) -> Entry {
        Entry::Heading {
            title: title.into(),
            fold: None,
            top: 24,
        }
    }

    fn entries(store: &gio::ListStore) -> Vec<Entry> {
        (0..store.n_items())
            .filter_map(|i| store.item(i).and_downcast::<EntryObject>())
            .map(|e| e.entry().clone())
            .collect()
    }

    /// Syncs `store` to `new`, returning how many entries were replaced or
    /// added: what the view binds again.
    fn rebound(store: &gio::ListStore, new: Vec<Entry>) -> u32 {
        let added = Rc::new(Cell::new(0));
        let count = added.clone();
        let handler = store.connect_items_changed(move |_, _, _, n| count.set(count.get() + n));
        sync(store, new.clone());
        store.disconnect(handler);
        assert_eq!(entries(store), new);
        added.get()
    }

    #[test]
    fn a_changed_episode_replaces_only_its_entry() {
        let store = gio::ListStore::new::<EntryObject>();
        let list = |played| {
            let mut list = vec![Entry::Header, heading("Today")];
            list.extend((1..=500).map(|id| episode(id, id == 250 && played)));
            list.push(Entry::Space(24));
            list
        };
        assert_eq!(rebound(&store, list(false)), 503);
        assert_eq!(rebound(&store, list(true)), 1);
        assert_eq!(rebound(&store, list(true)), 0);
    }

    #[test]
    fn added_and_removed_episodes_leave_the_rest_in_place() {
        let store = gio::ListStore::new::<EntryObject>();
        let with = |ids: &[i64]| {
            let mut list = vec![heading("Today")];
            list.extend(ids.iter().map(|id| episode(*id, false)));
            list.push(heading("Yesterday"));
            list.extend((100..200).map(|id| episode(id, false)));
            list
        };
        rebound(&store, with(&[1, 2, 3]));
        // Marked played under an Unplayed filter: one entry goes.
        assert_eq!(rebound(&store, with(&[1, 3])), 0);
        // A refresh finds a new episode.
        assert_eq!(rebound(&store, with(&[0, 1, 3])), 1);
        // Another order entirely is all new.
        assert_eq!(rebound(&store, vec![Entry::Message("None".into())]), 1);
        assert_eq!(rebound(&store, Vec::new()), 0);
    }
}
