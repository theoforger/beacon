// SPDX-License-Identifier: GPL-3.0-or-later

//! The content pane's widgets: the navigation view and its root page, with
//! the header, the search bar and the section stack.

use adw::prelude::*;
use gettextrs::gettext;

use super::sections::new_details_toggle;
use super::{ROOT_TAG, sections};
use crate::navigation::Location;
use crate::ui::labels;

pub struct ContentWidgets {
    pub(super) navigation: adw::NavigationView,
    pub(super) root_page: adw::NavigationPage,
    pub(super) root_toolbar: adw::ToolbarView,
    pub(super) root_header: adw::HeaderBar,
    pub(super) refresh_holder: adw::Bin,
    /// The header's start: Refresh Feeds, or the sections' Clear.
    pub(super) start: gtk::Box,
    pub(super) refresh: gtk::Button,
    pub(super) details_toggle: gtk::ToggleButton,
    pub(super) search_toggle: gtk::ToggleButton,
    pub(super) search_bar: gtk::SearchBar,
    pub(super) search_entry: gtk::SearchEntry,
    pub(super) stack: gtk::Stack,
}

/// Build the pane, with New's filter button in the header and the refresh
/// bar over the sections.
pub(super) fn widgets(
    new_filter: &gtk::Overlay,
    refresh_bar: &sections::RefreshBar,
) -> ContentWidgets {
    // Only in Shows and New, where new episodes appear; its holder is
    // hidden in narrow windows, where the main menu's Refresh Feeds is one
    // Back away.
    let refresh = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .action_name("win.refresh")
        .tooltip_text(gettext("Refresh Feeds"))
        .build();
    refresh.update_property(&[gtk::accessible::Property::Label(&gettext("Refresh Feeds"))]);
    let refresh_holder = adw::Bin::builder().child(&refresh).build();
    let details_toggle = new_details_toggle();
    let search_toggle = gtk::ToggleButton::builder()
        .icon_name("system-search-symbolic")
        .tooltip_text(gettext("Search"))
        .build();
    search_toggle.update_property(&[gtk::accessible::Property::Label(&gettext("Search"))]);

    let root_header = adw::HeaderBar::new();
    // One box without spacing, so whichever of Refresh Feeds and Clear a
    // section shows sits at the very start: Refresh's holder stays even
    // where the button is hidden, and the header would space Clear after
    // it (found by the user).
    let start = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    start.append(&refresh_holder);
    root_header.pack_start(&start);
    root_header.pack_end(&details_toggle);
    // Sort and filter, only in New, as on podcast pages.
    root_header.pack_end(new_filter);
    root_header.pack_end(&search_toggle);

    // Results arrive with M7.
    let search_entry = gtk::SearchEntry::builder()
        .hexpand(true)
        .placeholder_text(gettext("Search"))
        .build();
    let search_bar = gtk::SearchBar::builder()
        .child(
            &adw::Clamp::builder()
                .maximum_size(400)
                .child(&search_entry)
                .build(),
        )
        .build();

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .build();
    let overlay = gtk::Overlay::builder().child(&stack).build();
    overlay.add_overlay(refresh_bar.widget());

    let root_toolbar = adw::ToolbarView::new();
    root_toolbar.add_top_bar(&root_header);
    root_toolbar.add_top_bar(&search_bar);
    root_toolbar.set_content(Some(&overlay));
    let root_page = adw::NavigationPage::builder()
        .tag(ROOT_TAG)
        .title(labels::title(Location::Shows))
        .child(&root_toolbar)
        .build();

    let navigation = adw::NavigationView::new();
    navigation.add(&root_page);
    // Type to search only where the search bar is: the root page.
    let bar = search_bar.clone();
    navigation.connect_visible_page_notify(move |navigation| {
        let on_root = navigation.visible_page_tag().as_deref() == Some(ROOT_TAG);
        let window = on_root.then(|| bar.root()).flatten();
        bar.set_key_capture_widget(window.as_ref());
        if !on_root {
            bar.set_search_mode(false);
        }
    });

    ContentWidgets {
        navigation,
        root_page,
        root_toolbar,
        root_header,
        refresh_holder,
        start,
        refresh,
        details_toggle,
        search_toggle,
        search_bar,
        search_entry,
        stack,
    }
}

impl ContentWidgets {
    /// The root page's details toggle. The toggles on deeper pages follow it.
    pub fn details_toggle(&self) -> &gtk::ToggleButton {
        &self.details_toggle
    }

    /// What holds Refresh Feeds, hidden in narrow windows so the header
    /// fits 360px even with minimise, maximise and close beside it.
    pub fn refresh_holder(&self) -> &adw::Bin {
        &self.refresh_holder
    }

    /// Type to search: typing anywhere in `window` opens the search bar
    /// while the root page shows.
    pub fn capture_typing(&self, window: &impl IsA<gtk::Widget>) {
        self.search_bar.set_key_capture_widget(Some(window));
    }

    /// Leave the deeper page showing, if there is one.
    pub fn go_back(&self) -> bool {
        self.navigation.pop()
    }

    /// Ctrl+F: open the search bar, or close it if it is open. Only the root
    /// page has one.
    pub fn toggle_search(&self) {
        if self.navigation.visible_page_tag().as_deref() != Some(ROOT_TAG)
            || !self.root_header.is_visible()
        {
            return;
        }
        self.search_toggle
            .set_active(!self.search_toggle.is_active());
    }
}
