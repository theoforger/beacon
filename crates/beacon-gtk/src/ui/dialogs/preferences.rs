// SPDX-License-Identifier: GPL-3.0-or-later

//! Preferences: General, a Downloads group of stock rows each bound to its
//! setting (`docs/specs/auto-delete.md`; Delete Old Downloads turns the
//! others on, and while it is off they are unavailable); and Sync
//! (`sync_page.rs`).

use adw::prelude::*;
use gettextrs::gettext;
use gtk::gio;

use super::sync_page::SyncPage;

/// The dialog, its rows bound to `settings`, with the Sync page after
/// General.
pub fn dialog(settings: &gio::Settings, sync: &SyncPage) -> adw::PreferencesDialog {
    let enabled = adw::SwitchRow::builder()
        .title(gettext("Delete _Old Downloads"))
        .subtitle(gettext("Queued episodes are never deleted"))
        .use_underline(true)
        .build();
    settings
        .bind("delete-old-downloads", &enabled, "active")
        .build();

    let after = adw::SpinRow::builder()
        // Sentence capitalization, as the HIG's Spin Buttons page asks.
        .title(gettext("Delete _after"))
        // Translators: the unit of the Delete After row's number.
        .subtitle(gettext("Days"))
        .use_underline(true)
        .adjustment(&gtk::Adjustment::new(7.0, 1.0, 365.0, 1.0, 7.0, 0.0))
        .numeric(true)
        .build();
    // A `u` setting and a double property: bound by hand, as GSettings
    // maps numbers of the same kind only.
    after.set_value(f64::from(settings.uint("delete-after-days")));
    let s = settings.clone();
    after.connect_value_notify(move |row| {
        // Whole days from 1 to 365, as the adjustment allows.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let days = row.value().round().clamp(1.0, 365.0) as u32;
        if s.uint("delete-after-days") != days {
            let _ = s.set_uint("delete-after-days", days);
        }
    });

    let only_played = adw::SwitchRow::builder()
        .title(gettext("Only Delete _Played Episodes"))
        .subtitle(gettext(
            "Days count from when an episode was played, not when it was downloaded",
        ))
        .use_underline(true)
        .build();
    settings
        .bind("delete-only-played", &only_played, "active")
        .build();

    for row in [after.upcast_ref::<gtk::Widget>(), only_played.upcast_ref()] {
        enabled
            .bind_property("active", row, "sensitive")
            .sync_create()
            .build();
    }

    let downloads = adw::PreferencesGroup::builder()
        .title(gettext("Downloads"))
        .build();
    downloads.add(&enabled);
    downloads.add(&after);
    downloads.add(&only_played);
    let page = adw::PreferencesPage::builder()
        .title(gettext("General"))
        .icon_name("preferences-system-symbolic")
        .build();
    page.add(&downloads);
    let dialog = adw::PreferencesDialog::new();
    dialog.add(&page);
    dialog.add(sync);
    dialog
}
