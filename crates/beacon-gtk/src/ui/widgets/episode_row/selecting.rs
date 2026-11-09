// SPDX-License-Identifier: GPL-3.0-or-later

//! How a row takes part in selection mode.

use beacon_core::model::EpisodeId;
use gettextrs::gettext;
use gtk::subclass::prelude::*;
use gtk::{self, gdk, glib, prelude::*};

use super::EpisodeRow;

impl EpisodeRow {
    /// The check box, at the start of the row; made the first time
    /// selection mode needs it. libadwaita's round selection check, as in
    /// GNOME's own selection modes; the row's name tells a screen reader
    /// which episode it is.
    fn select_check(&self) -> Option<&gtk::CheckButton> {
        let parts = self.imp().parts.get()?;
        Some(parts.select.get_or_init(|| {
            let check = gtk::CheckButton::builder()
                .valign(gtk::Align::Center)
                .tooltip_text(gettext("Select"))
                .css_classes(["selection-mode"])
                .build();
            check.update_property(&[gtk::accessible::Property::Label(&gettext("Select"))]);
            let row = self.downgrade();
            check.connect_toggled(move |check| {
                let Some(row) = row.upgrade() else {
                    return;
                };
                let (Some(parts), Some(id)) = (row.imp().parts.get(), row.episode_id()) else {
                    return;
                };
                if parts.selection.contains(id) != check.is_active() {
                    parts.selection.set(id, check.is_active());
                }
            });
            parts.content.prepend(&check);
            check
        }))
    }

    /// Selecting by the check box, a secondary click or a long press. The
    /// check box shows only in selection mode, not on hover as the HIG has
    /// it; the row menu's Select starts selecting instead (asked by the
    /// user, 2026-11-04).
    pub(super) fn connect_selection(&self) {
        let toggle = |row: &glib::WeakRef<EpisodeRow>| {
            let row = row.upgrade()?;
            let id = row.episode_id()?;
            row.imp().parts.get()?.selection.toggle(id);
            Some(())
        };
        let secondary = gtk::GestureClick::builder()
            .button(gdk::BUTTON_SECONDARY)
            .build();
        let row = self.downgrade();
        secondary.connect_pressed(move |gesture, _, _, _| {
            if toggle(&row).is_some() {
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        });
        self.add_controller(secondary);
        let long_press = gtk::GestureLongPress::builder().touch_only(true).build();
        let row = self.downgrade();
        long_press.connect_pressed(move |gesture, _, _| {
            if toggle(&row).is_some() {
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        });
        self.add_controller(long_press);
    }

    pub(super) fn episode_id(&self) -> Option<EpisodeId> {
        let parts = self.imp().parts.get()?;
        parts.episode.borrow().as_ref().map(|item| item.episode.id)
    }

    /// The check box, ticked when selected and shown in selection mode; the
    /// play button and menu only outside it.
    pub fn show_selection(&self) {
        let (Some(parts), Some(id)) = (self.imp().parts.get(), self.episode_id()) else {
            return;
        };
        let active = parts.selection.is_active();
        let selected = parts.selection.contains(id);
        if active || selected {
            if let Some(check) = self.select_check() {
                check.set_active(selected);
                check.set_visible(true);
            }
        } else if let Some(check) = parts.select.get() {
            check.set_active(false);
            check.set_visible(false);
        }
        parts.menu.set_visible(!active);
        self.fit_narrow();
        self.show_handle();
    }
}
