// SPDX-License-Identifier: GPL-3.0-or-later

//! Search on a podcast page: the header's Search toggle and Ctrl+F open a
//! search bar under the header that narrows the page's own episodes in
//! place. Every search field searches only its own page (decided by the
//! user 2026-11-11, replacing the mockup's This Show / Everywhere toggle).

use adw::prelude::*;
use gettextrs::gettext;

use crate::ui::content::search;

pub struct ShowSearch {
    pub toggle: gtk::ToggleButton,
    pub bar: gtk::SearchBar,
    entry: gtk::SearchEntry,
}

impl ShowSearch {
    /// `changed` hears every change of the query, `None` when the bar
    /// closes or the field is emptied.
    pub fn new(changed: impl Fn(Option<String>) + 'static) -> Self {
        let toggle = gtk::ToggleButton::builder()
            .icon_name("system-search-symbolic")
            .tooltip_text(gettext("Search"))
            .build();
        toggle.update_property(&[gtk::accessible::Property::Label(&gettext("Search"))]);
        let entry = gtk::SearchEntry::builder()
            .hexpand(true)
            .placeholder_text(gettext("Search This Show"))
            .build();
        let bar = gtk::SearchBar::builder()
            .child(
                &adw::Clamp::builder()
                    .maximum_size(400)
                    .child(&entry)
                    .build(),
            )
            .build();
        bar.connect_entry(&entry);
        toggle
            .bind_property("active", &bar, "search-mode-enabled")
            .bidirectional()
            .sync_create()
            .build();
        search::connect_query(&bar, &entry, changed);
        Self { toggle, bar, entry }
    }

    /// Close the bar, ending any search, and empty it for the next show.
    pub fn close(&self) {
        self.bar.set_search_mode(false);
        self.entry.set_text("");
    }

    /// Ctrl+F.
    pub fn toggle(&self) {
        self.toggle.set_active(!self.toggle.is_active());
    }

    /// Type into the field, for tests.
    #[cfg(test)]
    pub fn type_text(&self, text: &str) {
        self.bar.set_search_mode(true);
        self.entry.set_text(text);
    }
}
