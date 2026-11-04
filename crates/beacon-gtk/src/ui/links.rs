// SPDX-License-Identifier: GPL-3.0-or-later

//! Keyboard movement between the links of a label. GTK moves between them
//! with Tab and Shift+Tab only (`gtk_label_focus`); descriptions are full of
//! links, so the arrow keys do it too.

use relm4::gtk::{self, gdk, glib, prelude::*};

/// Let Left and Up go to the previous link of `label`, Right and Down to the
/// next. Past the first or last link the arrow does what it would anyway.
pub fn arrow_keys_between_links(label: &gtk::Label) {
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(|keys, key, _, modifiers| {
        if !modifiers.is_empty() {
            return glib::Propagation::Proceed;
        }
        let direction = match key {
            gdk::Key::Left | gdk::Key::Up => gtk::DirectionType::TabBackward,
            gdk::Key::Right | gdk::Key::Down => gtk::DirectionType::TabForward,
            _ => return glib::Propagation::Proceed,
        };
        match keys.widget() {
            Some(label) if label.child_focus(direction) => glib::Propagation::Stop,
            _ => glib::Propagation::Proceed,
        }
    });
    label.add_controller(keys);
}

/// Make a description selectable, for copying, without its whole text being
/// selected each time it takes keyboard focus: the desktop's
/// `gtk-label-select-on-focus` does that, and descriptions take focus for
/// their links. The arrow keys move between links (see above); Shift with
/// the arrows, and the pointer, select.
pub fn selectable_description(label: &gtk::Label) {
    label.set_selectable(true);
    let focus = gtk::EventControllerFocus::new();
    let weak = label.downgrade();
    focus.connect_enter(move |_| {
        let weak = weak.clone();
        // The label selects everything after focus arrives.
        glib::idle_add_local_once(move || {
            if let Some(label) = weak.upgrade()
                && let Some((start, end)) = label.selection_bounds()
                && start == 0
                && end == label.text().chars().count() as i32
            {
                label.select_region(0, 0);
            }
        });
    });
    label.add_controller(focus);
    arrow_keys_between_links(label);
}
