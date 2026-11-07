// SPDX-License-Identifier: GPL-3.0-or-later

//! The narrow layout, keyboard focus and Back, and the remembered window
//! size.

use relm4::adw::prelude::*;
use relm4::gtk::{self, gdk, gio, glib};
use relm4::{ComponentController, adw};

use super::{App, AppWidgets, NARROW_BREAKPOINT_SP};
use crate::ui::widgets::episode_row;

/// The details pane's maximum width in the narrow layout: more than any
/// window, so the fraction of 1.0 decides. `f64::MAX` overflows when GTK
/// converts it to an integer.
const FULL_WIDTH: f64 = 100_000.0;

impl App {
    /// Alt+Left or the Back key: close the details page in the narrow
    /// layout, else leave a deeper page, else return from the content to
    /// the sidebar in the narrow layout, as Escape does there.
    pub(super) fn go_back(&self, widgets: &AppWidgets) {
        if widgets.details_split.is_collapsed() && widgets.details_split.shows_sidebar() {
            widgets.details_split.set_show_sidebar(false);
        } else if !self.content.widgets().go_back()
            && widgets.split_view.is_collapsed()
            && widgets.split_view.shows_content()
        {
            widgets.split_view.set_show_content(false);
        }
    }

    /// Remember where focus was before a row opens the details pane, and
    /// move it into the pane once its content is in place.
    pub(super) fn focus_details(&self, widgets: &AppWidgets) {
        let focus = widgets.details_split.root().and_then(|root| root.focus());
        self.details_origin
            .replace(focus.map(|widget| widget.downgrade()));
        // Its first action, Play: the description's links come after.
        let play = self.details.widgets().play_button().clone();
        glib::idle_add_local_once(move || {
            play.grab_focus();
        });
    }

    /// Start in the sidebar; Escape closes the details pane; closing it
    /// returns focus to where it was before the pane opened.
    pub(super) fn manage_focus(&self, widgets: &AppWidgets) {
        let library = self.sidebar.widgets().library().clone();
        glib::idle_add_local_once(move || {
            library.child_focus(gtk::DirectionType::TabForward);
        });

        let escape = gtk::EventControllerKey::new();
        let split = widgets.details_split.downgrade();
        escape.connect_key_pressed(move |_, key, _, _| {
            if key != gdk::Key::Escape {
                return glib::Propagation::Proceed;
            }
            if let Some(split) = split.upgrade() {
                split.set_show_sidebar(false);
            }
            glib::Propagation::Stop
        });
        self.details.widget().add_controller(escape);

        let pane = self.details.widget().clone();
        let origin = self.details_origin.clone();
        widgets
            .details_split
            .connect_show_sidebar_notify(move |split| {
                if split.shows_sidebar() {
                    return;
                }
                let Some(widget) = origin.take().and_then(|weak| weak.upgrade()) else {
                    return;
                };
                let inside = split
                    .root()
                    .and_then(|root| root.focus())
                    .is_none_or(|focus| focus.is_ancestor(&pane));
                if inside && widget.is_mapped() {
                    widget.grab_focus();
                }
            });
    }

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
        // The player bar stacks, and leaves out speed, sleep timer and
        // volume, as the mockup's narrow layout does.
        breakpoint.add_setter(
            self.bar.widgets().bar(),
            "orientation",
            Some(&gtk::Orientation::Vertical.to_value()),
        );
        breakpoint.add_setter(
            self.bar.widgets().extras(),
            "visible",
            Some(&false.to_value()),
        );
        breakpoint.connect_apply(|_| episode_row::set_narrow(true));
        breakpoint.connect_unapply(|_| episode_row::set_narrow(false));
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
