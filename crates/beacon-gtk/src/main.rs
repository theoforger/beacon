// SPDX-License-Identifier: GPL-3.0-or-later

mod app;
mod config;
mod navigation;
mod ui;

use gettextrs::{LocaleCategory, gettext};
use relm4::RelmApp;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk::{gio, glib};
use tracing_subscriber::EnvFilter;

use crate::app::App;
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

    RelmApp::from_app(app).run::<App>(());
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
        .activate(|app: &adw::Application, _, _| app.quit())
        .build();
    app.add_action_entries([quit]);

    app.set_accels_for_action("app.quit", &["<Control>q"]);
    app.set_accels_for_action("window.close", &["<Control>w"]);
    app.set_accels_for_action("win.search", &["<Control>f"]);
    app.set_accels_for_action("win.back", &["<Alt>Left", "Back"]);
}
