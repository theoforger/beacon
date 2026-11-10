// SPDX-License-Identifier: GPL-3.0-or-later

//! The Queue's Next in Queue card, which lays out its own rows so they can
//! be reordered by hand (asked by the user 2026-11-10): a row dragged by its
//! handle follows the pointer up and down, never out of the card, and the
//! rows it passes slide out of its way; let go, it settles into its place
//! and the queue is saved in that order. A list view recycles its rows and
//! cannot move them like this, so the card is its own widget, one entry of
//! the Queue's list (`ui/episode_list.rs`). The queue is short, so it holds
//! every row.

use std::cell::{Cell, OnceCell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use crate::ui::episode_list;
use crate::ui::models::{EpisodeItem, RowAction};
use crate::ui::widgets::episode_row::{EpisodeRow, RowSetup};

/// How long a row takes to slide into its new place.
const SLIDE_MS: u32 = 200;

/// A row being dragged: which, how far from its place, and where it would
/// land now.
#[derive(Debug, Clone, Copy)]
pub struct Drag {
    index: usize,
    dy: f64,
    target: usize,
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct QueueCard {
        pub setup: OnceCell<RowSetup>,
        pub rows: RefCell<Vec<EpisodeRow>>,
        /// How far each row is drawn from its place while it slides.
        pub offsets: RefCell<Vec<f64>>,
        /// Each row's slide under way, if any.
        pub slides: RefCell<Vec<Option<adw::TimedAnimation>>>,
        pub drag: Cell<Option<Drag>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for QueueCard {
        const NAME: &'static str = "BeaconQueueCard";
        type Type = super::QueueCard;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("queuecard");
            klass.set_accessible_role(gtk::AccessibleRole::List);
        }
    }

    impl ObjectImpl for QueueCard {
        fn dispose(&self) {
            for row in self.rows.take() {
                row.unparent();
            }
        }
    }

    impl WidgetImpl for QueueCard {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let rows = self.rows.borrow();
            let sizes = rows.iter().map(|row| row.measure(orientation, for_size));
            let (min, nat) = if orientation == gtk::Orientation::Horizontal {
                sizes.fold((0, 0), |(min, nat), (m, n, _, _)| (min.max(m), nat.max(n)))
            } else {
                sizes.fold((0, 0), |(min, nat), (m, n, _, _)| (min + m, nat + n))
            };
            (min, nat, -1, -1)
        }

        fn size_allocate(&self, width: i32, _height: i32, _baseline: i32) {
            let rows = self.rows.borrow();
            let offsets = self.offsets.borrow();
            let slots = slots(&rows, width);
            let drag = self.drag.get();
            for (index, row) in rows.iter().enumerate() {
                let (top, height) = slots[index];
                let y = match drag {
                    Some(drag) if drag.index == index => top + drag.dy,
                    _ => top + offsets.get(index).copied().unwrap_or_default(),
                };
                #[allow(clippy::cast_possible_truncation)]
                let at = gsk::Transform::new().translate(&graphene::Point::new(0.0, y as f32));
                row.allocate(width, height as i32, -1, Some(at));
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let rows = self.rows.borrow();
            let dragged = self.drag.get().map(|drag| drag.index);
            // The dragged row last, over the rows it passes.
            for (index, row) in rows.iter().enumerate() {
                if Some(index) != dragged {
                    self.obj().snapshot_child(row, snapshot);
                }
            }
            if let Some(row) = dragged.and_then(|index| rows.get(index)) {
                self.obj().snapshot_child(row, snapshot);
            }
        }
    }
}

glib::wrapper! {
    pub struct QueueCard(ObjectSubclass<imp::QueueCard>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

/// Where each row's place starts, and its height, at `width`.
fn slots(rows: &[EpisodeRow], width: i32) -> Vec<(f64, f64)> {
    let mut top = 0.0;
    rows.iter()
        .map(|row| {
            let height = f64::from(row.measure(gtk::Orientation::Vertical, width).1);
            let slot = (top, height);
            top += height;
            slot
        })
        .collect()
}

/// Where a row of the `slots` dragged from `index` by `dy` would land: the
/// place its middle is over.
fn landing(slots: &[(f64, f64)], index: usize, dy: f64) -> usize {
    let (top, height) = slots[index];
    let middle = top + dy + height / 2.0;
    slots
        .iter()
        .position(|(top, height)| middle < top + height)
        .unwrap_or(slots.len().saturating_sub(1))
}

/// How far the row at `other` moves aside while the row at `index` is over
/// `target`: up or down by the dragged row's height when it lies between.
fn aside(index: usize, target: usize, other: usize, height: f64) -> f64 {
    if index < other && other <= target {
        -height
    } else if target <= other && other < index {
        height
    } else {
        0.0
    }
}

impl QueueCard {
    /// A card whose rows act through `setup`, named `label` for screen
    /// readers.
    pub fn new(setup: &RowSetup, label: &str) -> Self {
        let card: Self = glib::Object::new();
        card.add_css_class("queue-card");
        card.update_property(&[gtk::accessible::Property::Label(label)]);
        let _ = card.imp().setup.set(setup.clone());
        card.connect_arrow_keys();
        card
    }

    /// Show `items`, in order. Rows already showing an episode keep it, so
    /// a reload after a drop changes nothing that moved.
    pub fn set(&self, items: &[EpisodeItem]) {
        let imp = self.imp();
        let Some(setup) = imp.setup.get() else {
            return;
        };
        if imp.drag.get().is_some() {
            return;
        }
        let mut old: Vec<EpisodeRow> = imp.rows.take();
        let mut rows = Vec::with_capacity(items.len());
        for item in items {
            let id = item.episode.id;
            let row = match old
                .iter()
                .position(|row| row.item().is_some_and(|i| i.episode.id == id))
            {
                Some(at) => old.remove(at),
                None => {
                    let row = EpisodeRow::new_in_card(setup);
                    row.set_parent(self);
                    self.connect_row(&row);
                    row
                }
            };
            rows.push(row);
        }
        for row in old {
            row.unparent();
        }
        let count = rows.len();
        for (index, (row, item)) in rows.iter().zip(items).enumerate() {
            row.bind(item);
            row.set_edges(index == 0, index + 1 == count);
            row.set_movable(true, index == 0, index + 1 == count);
            row.update_property(&[gtk::accessible::Property::Label(&row.accessible_name())]);
            self.connect_handle(row);
        }
        for slide in imp.slides.take().into_iter().flatten() {
            slide.skip();
        }
        imp.offsets.replace(vec![0.0; count]);
        imp.slides.replace(vec![None; count]);
        imp.rows.replace(rows);
        self.queue_resize();
    }

    /// The rows, for tests.
    #[cfg(test)]
    pub fn rows(&self) -> Vec<EpisodeRow> {
        self.imp().rows.borrow().clone()
    }

    /// Drag the row at `index` by `dy` without letting go, for tests:
    /// returns how far it went, which the card keeps inside itself.
    #[cfg(test)]
    pub fn drag_by(&self, index: usize, dy: f64) -> f64 {
        if self.imp().drag.get().is_none() {
            let row = self.imp().rows.borrow()[index].clone();
            self.begin_drag(&row);
        }
        self.update_drag(dy);
        self.imp().drag.get().map_or(0.0, |drag| drag.dy)
    }

    /// Let go of the row being dragged, for tests.
    #[cfg(test)]
    pub fn let_go(&self) {
        self.end_drag();
    }

    /// Activating a row, as the list view does for its own rows: a click,
    /// or Enter or Space while it has the focus.
    fn connect_row(&self, row: &EpisodeRow) {
        let Some(setup) = self.imp().setup.get().cloned() else {
            return;
        };
        let click = gtk::GestureClick::builder()
            .button(gdk::BUTTON_PRIMARY)
            .build();
        let (r, s) = (row.downgrade(), setup.clone());
        click.connect_released(move |gesture, presses, _, _| {
            if presses != 1 {
                return;
            }
            if let Some(item) = r.upgrade().and_then(|row| row.item()) {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                episode_list::activate(&s, &item);
            }
        });
        row.add_controller(click);
        let keys = gtk::EventControllerKey::new();
        let r = row.downgrade();
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(row) = r.upgrade().filter(|row| row.has_focus()) else {
                return glib::Propagation::Proceed;
            };
            match key {
                gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::space => {
                    if let Some(item) = row.item() {
                        episode_list::activate(&setup, &item);
                    }
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        row.add_controller(keys);
    }

    /// Up and Down move between the card's rows; past its ends they go on
    /// to the list's.
    fn connect_arrow_keys(&self) {
        let keys = gtk::EventControllerKey::new();
        let card = self.downgrade();
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(card) = card.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let step: isize = match key {
                gdk::Key::Up | gdk::Key::KP_Up => -1,
                gdk::Key::Down | gdk::Key::KP_Down => 1,
                _ => return glib::Propagation::Proceed,
            };
            let rows = card.imp().rows.borrow().clone();
            let Some(at) = rows.iter().position(|row| row.has_focus()) else {
                return glib::Propagation::Proceed;
            };
            match at.checked_add_signed(step).and_then(|to| rows.get(to)) {
                Some(next) => {
                    next.grab_focus();
                    glib::Propagation::Stop
                }
                None => glib::Propagation::Proceed,
            }
        });
        self.add_controller(keys);
    }

    /// Drag the row by its handle, once it has one.
    fn connect_handle(&self, row: &EpisodeRow) {
        let Some(handle) = row.handle() else {
            return;
        };
        // Once per handle: a handle keeps its gesture as the row is bound
        // to other episodes.
        if handle.observe_controllers().n_items() > 0 {
            return;
        }
        let gesture = gtk::GestureDrag::builder()
            .button(gdk::BUTTON_PRIMARY)
            .build();
        let (card, r) = (self.downgrade(), row.downgrade());
        gesture.connect_drag_begin(move |gesture, _, _| {
            let (Some(card), Some(row)) = (card.upgrade(), r.upgrade()) else {
                return;
            };
            // Claimed, so the press does not also open the details.
            gesture.set_state(gtk::EventSequenceState::Claimed);
            card.begin_drag(&row);
        });
        let card = self.downgrade();
        gesture.connect_drag_update(move |_, _, dy| {
            if let Some(card) = card.upgrade() {
                card.update_drag(dy);
            }
        });
        let card = self.downgrade();
        gesture.connect_drag_end(move |_, _, _| {
            if let Some(card) = card.upgrade() {
                card.end_drag();
            }
        });
        handle.add_controller(gesture);
    }

    fn begin_drag(&self, row: &EpisodeRow) {
        let imp = self.imp();
        let Some(index) = imp.rows.borrow().iter().position(|r| r == row) else {
            return;
        };
        imp.drag.set(Some(Drag {
            index,
            dy: 0.0,
            target: index,
        }));
        row.add_css_class("dragging");
        self.set_cursor_from_name(Some("grabbing"));
        self.queue_draw();
    }

    fn update_drag(&self, dy: f64) {
        let imp = self.imp();
        let Some(mut drag) = imp.drag.get() else {
            return;
        };
        let slots = slots(&imp.rows.borrow(), self.width());
        let Some(&(top, height)) = slots.get(drag.index) else {
            return;
        };
        // Never out of the card.
        let bottom = slots.last().map_or(0.0, |(top, height)| top + height);
        drag.dy = dy.clamp(-top, bottom - top - height);
        drag.target = landing(&slots, drag.index, drag.dy);
        imp.drag.set(Some(drag));
        for other in 0..slots.len() {
            if other != drag.index {
                self.slide(other, aside(drag.index, drag.target, other, height));
            }
        }
        // The card's rounded ends go to the rows now at its ends.
        let mut order: Vec<usize> = (0..slots.len()).collect();
        let dragged = order.remove(drag.index);
        order.insert(drag.target, dragged);
        let rows = imp.rows.borrow();
        let last = order.len().saturating_sub(1);
        for (place, index) in order.into_iter().enumerate() {
            rows[index].set_edges(place == 0, place == last);
        }
        drop(rows);
        self.queue_allocate();
    }

    /// Slide the row at `index` to `to` from its place, unless it is on
    /// its way there already.
    fn slide(&self, index: usize, to: f64) {
        let imp = self.imp();
        let from = imp.offsets.borrow().get(index).copied().unwrap_or_default();
        let heading = imp.slides.borrow().get(index).cloned().flatten();
        match heading {
            Some(slide) if (slide.value_to() - to).abs() < 0.5 => return,
            Some(slide) => slide.pause(),
            None if (from - to).abs() < 0.5 => return,
            None => {}
        }
        let card = self.downgrade();
        let target = adw::CallbackAnimationTarget::new(move |value| {
            let Some(card) = card.upgrade() else {
                return;
            };
            if let Some(offset) = card.imp().offsets.borrow_mut().get_mut(index) {
                *offset = value;
            }
            card.queue_allocate();
        });
        let slide = adw::TimedAnimation::builder()
            .widget(self)
            .value_from(from)
            .value_to(to)
            .duration(SLIDE_MS)
            .easing(adw::Easing::EaseOutCubic)
            .target(&target)
            .build();
        if let Some(slot) = imp.slides.borrow_mut().get_mut(index) {
            *slot = Some(slide.clone());
        }
        slide.play();
    }

    /// Let go: the dragged row settles into its new place, and the queue
    /// is saved in the new order (the dragged episode takes the place of
    /// the one it landed on).
    fn end_drag(&self) {
        let imp = self.imp();
        let Some(drag) = imp.drag.take() else {
            return;
        };
        self.set_cursor(None::<&gdk::Cursor>);
        let mut rows = imp.rows.borrow().clone();
        let Some(dragged) = rows.get(drag.index).cloned() else {
            return;
        };
        dragged.remove_css_class("dragging");
        let landed_on = rows.get(drag.target).and_then(EpisodeRow::item);
        let slots = slots(&rows, self.width());
        // Where the dragged row is drawn now, from the place it takes.
        let drawn = slots[drag.index].0 + drag.dy;
        let moved = rows.remove(drag.index);
        rows.insert(drag.target, moved);
        let new_slots = self::slots(&rows, self.width());
        for slide in imp.slides.take().into_iter().flatten() {
            slide.skip();
        }
        let count = rows.len();
        imp.offsets.replace(vec![0.0; count]);
        imp.slides.replace(vec![None; count]);
        for (index, row) in rows.iter().enumerate() {
            row.set_edges(index == 0, index + 1 == count);
            row.set_movable(true, index == 0, index + 1 == count);
        }
        imp.rows.replace(rows);
        if let Some(offset) = imp.offsets.borrow_mut().get_mut(drag.target) {
            *offset = drawn - new_slots[drag.target].0;
        }
        self.slide(drag.target, 0.0);
        self.queue_allocate();
        if drag.target != drag.index
            && let (Some(setup), Some(item), Some(landed_on)) =
                (imp.setup.get(), dragged.item(), landed_on)
        {
            (setup.on_action)(&landed_on, RowAction::MoveHere(item.episode.id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three rows of 10, 20 and 10 pixels.
    fn rows() -> Vec<(f64, f64)> {
        vec![(0.0, 10.0), (10.0, 20.0), (30.0, 10.0)]
    }

    #[test]
    fn a_row_lands_where_its_middle_is() {
        assert_eq!(landing(&rows(), 0, 0.0), 0);
        assert_eq!(landing(&rows(), 0, 6.0), 1);
        assert_eq!(landing(&rows(), 0, 26.0), 2);
        assert_eq!(landing(&rows(), 2, -26.0), 0);
        assert_eq!(landing(&rows(), 2, 100.0), 2, "past the end");
    }

    #[test]
    fn rows_between_move_aside() {
        // The first dragged over the last: the two after it move up.
        assert_eq!(aside(0, 2, 1, 10.0), -10.0);
        assert_eq!(aside(0, 2, 2, 10.0), -10.0);
        // The last dragged over the first: the two before it move down.
        assert_eq!(aside(2, 0, 0, 10.0), 10.0);
        assert_eq!(aside(2, 0, 1, 10.0), 10.0);
        // Rows outside stay.
        assert_eq!(aside(0, 1, 2, 10.0), 0.0);
    }
}
