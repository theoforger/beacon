// SPDX-License-Identifier: GPL-3.0-or-later

mod app;
mod config;
mod navigation;
mod player;
mod ui;

use gettextrs::{LocaleCategory, gettext};

use adw::prelude::*;
use gtk::{gio, glib};
use tracing_subscriber::EnvFilter;

use crate::app::Window;
use crate::config::{APP_ID, GETTEXT_PACKAGE, LOCALEDIR};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    setup_gettext();

    gio::resources_register_include!("beacon.gresource")
        .expect("the bundled GResource is always valid");

    glib::set_application_name(&gettext("Beacon"));

    let app = adw::Application::builder().application_id(APP_ID).build();
    setup_app_actions(&app);

    // One window: activating again brings it forward.
    app.connect_activate(|app| match app.active_window() {
        Some(window) => window.present(),
        None => Window::new(app).present(),
    });
    app.run();
}

fn setup_gettext() {
    // SAFETY: called once at startup, before any other thread exists.
    unsafe {
        gettextrs::setlocale(LocaleCategory::LcAll, "");
    }
    if let Err(err) = gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR) {
        tracing::warn!("could not bind the text domain: {err}");
    }
    if let Err(err) = gettextrs::bind_textdomain_codeset(GETTEXT_PACKAGE, "UTF-8") {
        tracing::warn!("could not set the text domain codeset: {err}");
    }
    if let Err(err) = gettextrs::textdomain(GETTEXT_PACKAGE) {
        tracing::warn!("could not switch to the text domain: {err}");
    }
}

fn setup_app_actions(app: &adw::Application) {
    let quit = gio::ActionEntry::builder("quit")
        // Closing each window, rather than quitting outright, saves the
        // window's state and lets it sync once more.
        .activate(|app: &adw::Application, _, _| {
            for window in app.windows() {
                window.close();
            }
        })
        .build();
    app.add_action_entries([quit]);

    app.set_accels_for_action("app.quit", &["<Control>q"]);
    app.set_accels_for_action("window.close", &["<Control>w"]);
    app.set_accels_for_action("win.search", &["<Control>f"]);
    app.set_accels_for_action("win.back", &["<Alt>Left", "Back"]);
    app.set_accels_for_action("win.refresh", &["<Control>r", "F5"]);
    app.set_accels_for_action("win.details", &["F9"]);
    app.set_accels_for_action("win.preferences", &["<Control>comma"]);
}
