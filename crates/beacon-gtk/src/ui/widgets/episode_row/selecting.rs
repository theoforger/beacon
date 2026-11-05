// SPDX-License-Identifier: GPL-3.0-or-later

//! How a row takes part in selection mode.

use beacon_core::model::EpisodeId;
use relm4::gtk::subclass::prelude::*;
use relm4::gtk::{self, gdk, glib, prelude::*};

use super::EpisodeRow;

impl EpisodeRow {
    /// Selecting by the check box, a secondary click or a long press, and
    /// the check box shown on hover (HIG Selection & Edit Modes: "In normal
    /// mode, a check box should be shown over each content item on hover").
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

        let hover = gtk::EventControllerMotion::new();
        let row = self.downgrade();
        hover.connect_enter(move |_, _, _| {
            if let Some(row) = row.upgrade() {
                row.set_hovered(true);
            }
        });
        let row = self.downgrade();
        hover.connect_leave(move |_| {
            if let Some(row) = row.upgrade() {
                row.set_hovered(false);
            }
        });
        self.add_controller(hover);

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

    fn set_hovered(&self, hovered: bool) {
        if let Some(parts) = self.imp().parts.get() {
            parts.hovered.set(hovered);
            self.show_selection();
        }
    }

    fn episode_id(&self) -> Option<EpisodeId> {
        let parts = self.imp().parts.get()?;
        parts.episode.borrow().as_ref().map(|item| item.episode.id)
    }

    /// The check box, ticked when selected and shown in selection mode or
    /// on hover; the play button and menu only outside selection mode.
    pub fn show_selection(&self) {
        let (Some(parts), Some(id)) = (self.imp().parts.get(), self.episode_id()) else {
            return;
        };
        let active = parts.selection.is_active();
        let selected = parts.selection.contains(id);
        parts.select.set_active(selected);
        parts
            .select
            .set_visible(active || selected || parts.hovered.get());
        parts.play.set_visible(!active);
        parts.menu.set_visible(!active);
    }
}
