// SPDX-License-Identifier: GPL-3.0-or-later

//! How a queued row in the Queue view is reordered, as the HIG has it for
//! lists whose order matters: "Include drag handles at the beginning of
//! the rows, to allow moving them", and a menu "with items for “move up”,
//! “move down”" ("required for accessibility purposes"). The row has the
//! handle and the menu items; the queue's card does the dragging.

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

    /// The handle at the start of the row, made the first time the row is
    /// queued. The queue's card (`ui/widgets/queue_card.rs`) drags the row
    /// by it.
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
            parts.content.prepend(&handle);
            handle
        });
    }

    /// The drag handle, once the row has been queued.
    pub fn handle(&self) -> Option<gtk::Widget> {
        let parts = self.imp().parts.get()?;
        parts.handle.get().map(|h| h.clone().upcast())
    }
}
