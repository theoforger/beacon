// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared plumbing for `GtkListView`s over a `gio::ListStore` of boxed Rust
//! values.

use relm4::gtk::{self, glib, prelude::*};

use crate::ui::widgets::episode_row::{EpisodeRow, OnRowAction};

/// The boxed value behind a list item, unwrapping tree list rows.
pub fn boxed(object: &glib::Object) -> Option<glib::BoxedAnyObject> {
    let object = match object.downcast_ref::<gtk::TreeListRow>() {
        Some(row) => row.item()?,
        None => object.clone(),
    };
    object.downcast::<glib::BoxedAnyObject>().ok()
}

/// The episode row inside a list item, creating it if the item holds
/// nothing or something else.
pub fn row_of(item: &gtk::ListItem, on_action: &OnRowAction) -> EpisodeRow {
    match item.child().and_downcast::<EpisodeRow>() {
        Some(row) => row,
        None => {
            let row = EpisodeRow::new(on_action.clone());
            item.set_child(Some(&row));
            row
        }
    }
}
