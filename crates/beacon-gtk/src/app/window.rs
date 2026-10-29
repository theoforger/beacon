// SPDX-License-Identifier: GPL-3.0-or-later

//! The narrow layout and the remembered window size.

use relm4::adw::prelude::*;
use relm4::gtk::gio;
use relm4::{ComponentController, adw};

use super::{App, AppWidgets, NARROW_BREAKPOINT_SP};

/// The details pane's maximum width in the narrow layout: more than any
/// window, so the fraction of 1.0 decides. `f64::MAX` overflows when GTK
/// converts it to an integer.
const FULL_WIDTH: f64 = 100_000.0;

impl App {
    /// Narrow windows: the sidebar becomes a page with the content pushed on
    /// top, and the details pane covers the window with its own Back. It
    /// starts closed and its header toggle is hidden.
    pub(super) fn narrow_breakpoint(&self, widgets: &AppWidgets) -> adw::Breakpoint {
        let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
            adw::BreakpointConditionLengthType::MaxWidth,
            NARROW_BREAKPOINT_SP,
            adw::LengthUnit::Sp,
        ));
        breakpoint.add_setter(&widgets.split_view, "collapsed", Some(&true.to_value()));
        breakpoint.add_setter(&widgets.details_split, "collapsed", Some(&true.to_value()));
        breakpoint.add_setter(
            &widgets.details_split,
            "show-sidebar",
            Some(&false.to_value()),
        );
        breakpoint.add_setter(
            &widgets.details_split,
            "max-sidebar-width",
            Some(&FULL_WIDTH.to_value()),
        );
        breakpoint.add_setter(
            &widgets.details_split,
            "sidebar-width-fraction",
            Some(&1.0f64.to_value()),
        );
        breakpoint.add_setter(
            self.content.widgets().details_toggle(),
            "visible",
            Some(&false.to_value()),
        );
        breakpoint.add_setter(
            self.details.widgets().back_button(),
            "visible",
            Some(&true.to_value()),
        );
        breakpoint.add_setter(
            self.sidebar.widgets().library(),
            "mode",
            Some(&adw::SidebarMode::Page.to_value()),
        );
        breakpoint
    }

    pub(super) fn restore_window_state(&self, window: &adw::ApplicationWindow) {
        window.set_default_size(
            self.settings.int("window-width"),
            self.settings.int("window-height"),
        );
        if self.settings.boolean("is-maximized") {
            window.maximize();
        }
    }
}

pub(super) fn save_window_state(settings: &gio::Settings, window: &adw::ApplicationWindow) {
    let (width, height) = window.default_size();
    let result = settings
        .set_int("window-width", width)
        .and_then(|()| settings.set_int("window-height", height))
        .and_then(|()| settings.set_boolean("is-maximized", window.is_maximized()));
    if let Err(err) = result {
        tracing::warn!("could not save the window state: {err}");
    }
}
