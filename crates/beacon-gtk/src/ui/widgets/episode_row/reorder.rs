// SPDX-License-Identifier: GPL-3.0-or-later

//! How a queued row in the Queue view is reordered, as the HIG has it for
//! lists whose order matters: "Include drag handles at the beginning of
//! the rows, to allow moving them", and a menu "with items for “move up”,
//! “move down”" ("required for accessibility purposes"). The handle drags
//! the row's episode; dropped on another queued row, it takes that row's
//! place.

use beacon_core::model::EpisodeId;
use gtk::subclass::prelude::*;
use gtk::{self, gdk, prelude::*};

use super::EpisodeRow;

impl EpisodeRow {
    /// Whether the row is queued (`movable`), with a handle and Move Up and
    /// Move Down; Move Up unavailable on the `first`, Move Down on the
    /// `last`.
    pub fn set_movable(&self, movable: bool, first: bool, last: bool) {
        let Some(parts) = self.imp().parts.get() else {
            return;
        };
        let changed = parts.movable.replace(movable) != movable;
        parts.move_up.set_enabled(movable && !first);
        parts.move_down.set_enabled(movable && !last);
        if movable {
            self.drag_handle();
        }
        self.show_handle();
        // The menu is built when opened (`bind`): drop one built without
        // the moves, or with them.
        if changed && parts.menu.menu_model().is_some() {
            parts.menu.set_menu_model(None::<&gtk::gio::MenuModel>);
        }
    }

    /// The handle shows on a queued row outside selection mode, where the
    /// check box takes the start of the row.
    pub(super) fn show_handle(&self) {
        let Some(parts) = self.imp().parts.get() else {
            return;
        };
        if let Some(handle) = parts.handle.get() {
            handle.set_visible(parts.movable.get() && !parts.selection.is_active());
        }
    }

    /// The handle at the start of the row, made with the row's drop target
    /// the first time the row is queued.
    fn drag_handle(&self) {
        let Some(parts) = self.imp().parts.get() else {
            return;
        };
        parts.handle.get_or_init(|| {
            // Left out of the accessibility tree: Move Up and Move Down in
            // the menu do what it does, without a pointer.
            let handle = gtk::Image::builder()
                .icon_name("list-drag-handle-symbolic")
                .valign(gtk::Align::Center)
                .css_classes(["dim-label"])
                .cursor(&gdk::Cursor::from_name("grab", None).expect("a named cursor"))
                .accessible_role(gtk::AccessibleRole::Presentation)
                .build();
            let source = gtk::DragSource::builder()
                .actions(gdk::DragAction::MOVE)
                .build();
            let row = self.downgrade();
            source.connect_prepare(move |source, x, y| {
                let row = row.upgrade()?;
                let id = row.episode_id()?;
                // The row follows the pointer, held where it was grabbed.
                let handle = source.widget()?;
                let at =
                    handle.compute_point(&row, &gtk::graphene::Point::new(x as f32, y as f32))?;
                source.set_icon(
                    Some(&gtk::WidgetPaintable::new(Some(&row))),
                    at.x() as i32,
                    at.y() as i32,
                );
                Some(gdk::ContentProvider::for_value(&id.0.to_value()))
            });
            handle.add_controller(source);
            parts.content.prepend(&handle);
            self.accept_drops();
            handle
        });
    }

    /// Take a queued episode dropped on the row, if it is queued too.
    fn accept_drops(&self) {
        let target = gtk::DropTarget::new(i64::static_type(), gdk::DragAction::MOVE);
        let row = self.downgrade();
        target.connect_accept(move |_, _| row.upgrade().is_some_and(|row| row.is_movable()));
        let row = self.downgrade();
        target.connect_drop(move |_, value, _, _| {
            let (Some(row), Ok(dropped)) = (row.upgrade(), value.get::<i64>()) else {
                return false;
            };
            if !row.is_movable() || row.episode_id() == Some(EpisodeId(dropped)) {
                return false;
            }
            row.activate_action("row.move-here", Some(&dropped.to_variant()))
                .is_ok()
        });
        self.add_controller(target);
    }

    fn is_movable(&self) -> bool {
        self.imp()
            .parts
            .get()
            .is_some_and(|parts| parts.movable.get())
    }
}
