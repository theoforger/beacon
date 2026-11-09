// SPDX-License-Identifier: GPL-3.0-or-later

use adw::prelude::*;
use gettextrs::gettext;

use crate::config::{APP_ID, VERSION};

pub fn present(parent: &impl IsA<gtk::Widget>) {
    let dialog = adw::AboutDialog::builder()
        .application_name(gettext("Beacon"))
        .application_icon(APP_ID)
        .developer_name("TheoForger")
        .developers(["TheoForger"])
        .version(VERSION)
        .copyright("© 2026 TheoForger")
        .license_type(gtk::License::Gpl30)
        // Translators: Replace "translator-credits" with your name and email, one per line.
        .translator_credits(gettext("translator-credits"))
        .build();

    dialog.present(Some(parent));
}
