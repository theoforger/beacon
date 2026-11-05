// SPDX-License-Identifier: GPL-3.0-or-later

//! How a row takes part in selection mode.

use beacon_core::model::EpisodeId;
use relm4::gtk::subclass::prelude::*;
use relm4::gtk::{self, gdk, glib, prelude::*};

use super::EpisodeRow;

impl EpisodeRow {
    /// Selecting by the check box, a secondary click or a long press. The
    /// check box shows only in selection mode, not on hover as the HIG has
    /// it; the row menu's Select starts selecting instead (asked by the
    /// user, 2026-11-04).
    pub(super) fn connect_selection(&self) {
        let Some(parts) = self.imp().parts.get() else {
            return;
        };
        let row = self.downgrade();
        parts.select.connect_toggled(move |check| {
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

    fn episode_id(&self) -> Option<EpisodeId> {
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
        parts.select.set_active(selected);
        parts.select.set_visible(active || selected);
        parts.play.set_visible(!active);
        parts.menu.set_visible(!active);
    }
}
