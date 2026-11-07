// SPDX-License-Identifier: GPL-3.0-or-later

//! The content pane's section pages and header toggles.

use std::cell::RefCell;
use std::time::Duration;

use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::gtk::glib;
use relm4::{adw, gtk};

use crate::navigation::Location;
use crate::ui::labels;
use crate::ui::selection::Selection;
use crate::ui::selection_bar::{self, OnChange};

use super::ROOT_TAG;

pub(super) fn new_details_toggle() -> gtk::ToggleButton {
    let toggle = gtk::ToggleButton::builder()
        .icon_name("sidebar-show-right-symbolic")
        .tooltip_text(gettext("Episode Details"))
        .build();
    toggle.update_property(&[gtk::accessible::Property::Label(&gettext(
        "Episode Details",
    ))]);
    toggle
}

pub(super) fn section_id(location: Location) -> &'static str {
    match location {
        Location::Shows => "shows",
        Location::New => "new",
        Location::Queue => "queue",
        Location::Favorites => "favorites",
        Location::Downloaded => "downloaded",
    }
}

/// Sections whose views arrive later (M5) show their empty state.
pub(super) fn empty_state(location: Location) -> adw::StatusPage {
    let (title, description) = match location {
        Location::Queue => (
            gettext("Queue Is Empty"),
            gettext("Add episodes to the queue to play them next"),
        ),
        Location::Favorites => (
            gettext("No Favorites"),
            gettext("Mark an episode as a favorite to find it here"),
        ),
        Location::Downloaded => (
            gettext("No Downloads"),
            gettext("Download an episode to listen offline"),
        ),
        Location::Shows | Location::New => unreachable!("Shows and New have their own views"),
    };
    adw::StatusPage::builder()
        .icon_name(labels::icon(location))
        .title(title)
        .description(description)
        .build()
}

/// Show `page`: push it, or go back to it if it is already further down.
pub(super) fn bring_forward(
    navigation: &adw::NavigationView,
    page: &adw::NavigationPage,
    tag: &str,
) {
    if navigation.visible_page_tag().as_deref() == Some(tag) {
        return;
    }
    match navigation.find_page(tag) {
        Some(existing) => {
            navigation.pop_to_page(&existing);
        }
        None => navigation.push(page),
    }
}

/// New's selection mode, on the root page, entered from `select`. The
/// search bar closes and stops catching typing meanwhile.
pub(super) fn attach_new_selection(
    toolbar: &adw::ToolbarView,
    header: &adw::HeaderBar,
    select: &gtk::Button,
    selection: &Selection,
    search_bar: &gtk::SearchBar,
    navigation: &adw::NavigationView,
    on_change: OnChange,
) {
    selection_bar::attach(toolbar, header, select, selection, on_change);
    let search_bar = search_bar.clone();
    let navigation = navigation.clone();
    selection.connect_changed(move |selection| {
        if selection.is_active() {
            search_bar.set_search_mode(false);
            search_bar.set_key_capture_widget(None::<&gtk::Widget>);
        } else if navigation.visible_page_tag().as_deref() == Some(ROOT_TAG) {
            search_bar.set_key_capture_widget(navigation.root().as_ref());
        }
    });
}

/// While feeds refresh, a thin bar pulses along the top of the content,
/// under the header bar, as in GNOME Podcasts (`content_stack.rs`, an
/// overlaid progress bar pulsed every 100ms).
pub(super) struct RefreshBar {
    bar: gtk::ProgressBar,
    pulse: RefCell<Option<glib::SourceId>>,
}

impl RefreshBar {
    pub(super) fn new() -> Self {
        let bar = gtk::ProgressBar::builder()
            .valign(gtk::Align::Start)
            .visible(false)
            .tooltip_text(gettext("Fetching feeds…"))
            .build();
        // Added, not set: setting the classes would drop `horizontal`.
        bar.add_css_class("refresh-progress");
        bar.update_property(&[gtk::accessible::Property::Label(&gettext("Fetching feeds"))]);
        Self {
            bar,
            pulse: RefCell::default(),
        }
    }

    pub(super) fn widget(&self) -> &gtk::ProgressBar {
        &self.bar
    }

    pub(super) fn set_running(&self, running: bool) {
        let mut pulse = self.pulse.borrow_mut();
        if running == pulse.is_some() {
            return;
        }
        self.bar.set_visible(running);
        match pulse.take() {
            Some(source) => source.remove(),
            None => {
                let bar = self.bar.downgrade();
                *pulse = Some(glib::timeout_add_local(
                    Duration::from_millis(100),
                    move || match bar.upgrade() {
                        Some(bar) => {
                            bar.pulse();
                            glib::ControlFlow::Continue
                        }
                        None => glib::ControlFlow::Break,
                    },
                ));
            }
        }
    }
}
