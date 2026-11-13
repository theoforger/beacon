// SPDX-License-Identifier: GPL-3.0-or-later

//! The content pane's section pages and header toggles.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::glib;

use crate::navigation::Location;
use crate::ui::dialogs::import;
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

/// Put `page` where `old` is in the navigation stack, without a slide,
/// leaving out any other place `page` had further down.
pub(super) fn swap_in(
    navigation: &adw::NavigationView,
    old: &impl IsA<adw::NavigationPage>,
    page: &impl IsA<adw::NavigationPage>,
) {
    let stack = navigation.navigation_stack();
    let pages: Vec<adw::NavigationPage> = (0..stack.n_items())
        .filter_map(|i| stack.item(i).and_downcast::<adw::NavigationPage>())
        .filter(|p| p != page.upcast_ref())
        .map(|p| {
            if p == *old.upcast_ref() {
                page.clone().upcast()
            } else {
                p
            }
        })
        .collect();
    navigation.replace(&pages);
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
/// overlaid progress bar pulsed every 100ms). While an OPML import runs
/// it fills instead, as far as the import has got: the HIG's Progress
/// Bars page, "For situations where task progress happens in the
/// background, and accompanying text isn't as important … a thin progress
/// bar can be used".
pub(super) struct RefreshBar {
    bar: gtk::ProgressBar,
    pulse: RefCell<Option<glib::SourceId>>,
    refreshing: Cell<bool>,
    /// The import's progress: done of total.
    import: Cell<Option<(usize, usize)>>,
}

impl RefreshBar {
    pub(super) fn new() -> Self {
        let bar = gtk::ProgressBar::builder()
            .valign(gtk::Align::Start)
            .visible(false)
            .build();
        // Added, not set: setting the classes would drop `horizontal`.
        bar.add_css_class("refresh-progress");
        let this = Self {
            bar,
            pulse: RefCell::default(),
            refreshing: Cell::new(false),
            import: Cell::new(None),
        };
        this.update();
        this
    }

    pub(super) fn widget(&self) -> &gtk::ProgressBar {
        &self.bar
    }

    pub(super) fn set_running(&self, running: bool) {
        self.refreshing.set(running);
        self.update();
    }

    /// An import's progress, or `None` once it has ended.
    pub(super) fn set_import(&self, progress: Option<(usize, usize)>) {
        self.import.set(progress);
        self.update();
    }

    fn update(&self) {
        let bar = &self.bar;
        let mut pulse = self.pulse.borrow_mut();
        let pulsing = self.import.get().is_none() && self.refreshing.get();
        if let Some((done, total)) = self.import.get() {
            let label = gettext("Importing podcasts");
            let count = import::progress_message(done, total);
            bar.set_fraction(if total == 0 {
                0.0
            } else {
                done as f64 / total as f64
            });
            bar.set_tooltip_text(Some(&format!("{label}: {count}")));
            bar.update_property(&[
                gtk::accessible::Property::Label(&label),
                gtk::accessible::Property::ValueText(&count),
            ]);
        } else {
            bar.set_tooltip_text(Some(&gettext("Fetching feeds…")));
            bar.update_property(&[gtk::accessible::Property::Label(&gettext("Fetching feeds"))]);
            gtk::prelude::AccessibleExt::reset_property(bar, gtk::AccessibleProperty::ValueText);
        }
        bar.set_visible(pulsing || self.import.get().is_some());
        match (pulsing, pulse.is_some()) {
            (true, false) => {
                let weak = bar.downgrade();
                *pulse = Some(glib::timeout_add_local(
                    Duration::from_millis(100),
                    move || match weak.upgrade() {
                        Some(bar) => {
                            bar.pulse();
                            glib::ControlFlow::Continue
                        }
                        None => glib::ControlFlow::Break,
                    },
                ));
            }
            (false, true) => {
                if let Some(source) = pulse.take() {
                    source.remove();
                }
            }
            _ => {}
        }
    }

    /// Whether the bar shows, its fraction and its label, for tests.
    #[cfg(test)]
    pub(super) fn state(&self) -> (bool, f64, Option<String>) {
        (
            self.bar.is_visible(),
            self.bar.fraction(),
            self.bar.tooltip_text().map(|t| t.to_string()),
        )
    }
}
