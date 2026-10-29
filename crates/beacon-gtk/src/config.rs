// SPDX-License-Identifier: GPL-3.0-or-later

//! Build-time configuration. Meson sets these variables when it calls Cargo;
//! the defaults match the Flatpak install so a plain `cargo build` still works.

pub const APP_ID: &str = "com.theoforger.Beacon";
pub const GETTEXT_PACKAGE: &str = "beacon";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const LOCALEDIR: &str = match option_env!("BEACON_LOCALEDIR") {
    Some(dir) => dir,
    None => "/app/share/locale",
};
