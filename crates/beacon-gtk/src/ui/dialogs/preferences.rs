// SPDX-License-Identifier: GPL-3.0-or-later

//! Preferences: General, a Downloads group of stock rows each bound to its
//! setting (`docs/specs/auto-delete.md`; Delete Old Downloads turns the
//! others on, and while it is off they are unavailable) and the country or
//! region of Apple's directory that Add searches (`docs/specs/
//! add-and-itunes.md`); and Sync (`sync_page.rs`).

use adw::prelude::*;
use beacon_core::services::itunes::{self, STOREFRONTS};
use gettextrs::gettext;
use gtk::{gio, glib};

/// Where the directory's country is remembered; empty for automatic.
const COUNTRY_KEY: &str = "directory-country";

/// A country's name in the listener's language, as iso-codes translates it.
fn country_name(english: &str) -> String {
    gettextrs::dgettext("iso_3166-1", english)
}

/// The directories, by code and translated name, in the order of their
/// names; the empty code, first, is Automatic.
pub fn countries() -> Vec<(String, String)> {
    let mut named: Vec<_> = STOREFRONTS
        .iter()
        .map(|(code, name)| ((*code).to_owned(), country_name(name)))
        .collect();
    named.sort_by_cached_key(|(_, name)| glib::casefold(name).to_string());
    let automatic = itunes::resolve_country("", itunes::system_locale().as_deref());
    let automatic = STOREFRONTS
        .iter()
        .find(|(code, _)| *code == automatic)
        .map(|(_, name)| country_name(name))
        .unwrap_or(automatic);
    // Translators: the first choice of Country or Region; the name is the
    // country found from the system's region.
    let first = gettext("Automatic ({country})").replace("{country}", &automatic);
    std::iter::once((String::new(), first))
        .chain(named)
        .collect()
}

/// The directory's country or region, bound to its setting.
fn country_row(settings: &gio::Settings) -> adw::ComboRow {
    let countries = countries();
    let names: Vec<&str> = countries.iter().map(|(_, name)| name.as_str()).collect();
    let row = adw::ComboRow::builder()
        .title(gettext("_Country or Region"))
        .subtitle(gettext("Whose Apple Podcasts directory Add searches"))
        .use_underline(true)
        .model(&gtk::StringList::new(&names))
        .enable_search(true)
        .expression(gtk::PropertyExpression::new(
            gtk::StringObject::static_type(),
            None::<&gtk::Expression>,
            "string",
        ))
        .build();
    let chosen = settings.string(COUNTRY_KEY);
    let at = countries
        .iter()
        .position(|(code, _)| code.eq_ignore_ascii_case(&chosen))
        .unwrap_or(0);
    row.set_selected(at as u32);
    let s = settings.clone();
    row.connect_selected_notify(move |row| {
        let code = countries
            .get(row.selected() as usize)
            .map(|(code, _)| code.as_str())
            .unwrap_or_default();
        if s.string(COUNTRY_KEY) != code {
            let _ = s.set_string(COUNTRY_KEY, code);
        }
    });
    row
}

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
    let directory = adw::PreferencesGroup::builder()
        .title(gettext("Podcast Directory"))
        .build();
    directory.add(&country_row(settings));
    let page = adw::PreferencesPage::builder()
        .title(gettext("General"))
        .icon_name("preferences-system-symbolic")
        .build();
    page.add(&downloads);
    page.add(&directory);
    let dialog = adw::PreferencesDialog::new();
    dialog.add(&page);
    dialog.add(sync);
    dialog
}
