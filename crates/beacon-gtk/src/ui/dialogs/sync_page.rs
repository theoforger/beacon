// SPDX-License-Identifier: GPL-3.0-or-later

//! The Sync page of Preferences (`docs/specs/sync.md`, Preferences), with
//! GNOME Podcasts' rows (`sync_preferences.ui`). Signed out: the server,
//! Log In With (Browser or Password), the username and password, and Log
//! In. Signed in: the account, the Sync switch, Last Synced with Sync Now,
//! and a destructive Log Out row. The page only shows state; the window does the work.

use std::cell::Cell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use chrono::{DateTime, Utc};
use gettextrs::gettext;
use gtk::glib;

use crate::ui::format;
use crate::ui::messages::Output;

/// What the page shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncView {
    SignedOut,
    /// Logging in: in the browser (which can be cancelled), or with a
    /// password.
    LoggingIn {
        browser: bool,
    },
    SignedIn {
        user: String,
        server: String,
        active: bool,
        last_synced: Option<DateTime<Utc>>,
        syncing: bool,
    },
}

#[derive(Debug)]
pub enum SyncPageInput {
    Show(SyncView),
}

#[derive(Debug)]
pub enum SyncPageOutput {
    LogInWithBrowser {
        server: String,
    },
    LogInWithPassword {
        server: String,
        user: String,
        password: String,
    },
    CancelLogIn,
    LogOut,
    SetActive(bool),
    SyncNow,
}

const BROWSER: &str = "browser";
const PASSWORD: &str = "password";

mod imp {
    use std::cell::OnceCell;

    use super::*;

    #[derive(Default)]
    pub struct SyncPage {
        pub widgets: OnceCell<super::Widgets>,
        /// Set while the page itself changes the switch.
        pub updating: Cell<bool>,
        pub output: Output<SyncPageOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SyncPage {
        const NAME: &'static str = "BeaconSyncPage";
        type Type = super::SyncPage;
        type ParentType = adw::PreferencesPage;
    }

    impl ObjectImpl for SyncPage {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }
    impl WidgetImpl for SyncPage {}
    impl PreferencesPageImpl for SyncPage {}
}

glib::wrapper! {
    pub struct SyncPage(ObjectSubclass<imp::SyncPage>)
        @extends adw::PreferencesPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for SyncPage {
    fn default() -> Self {
        glib::Object::new()
    }
}

pub struct Widgets {
    signed_out: adw::PreferencesGroup,
    server: adw::EntryRow,
    method: adw::ToggleGroup,
    user: adw::EntryRow,
    password: adw::PasswordEntryRow,
    log_in: gtk::Button,
    waiting: gtk::Box,
    waiting_label: gtk::Label,
    cancel: gtk::Button,
    signed_in: adw::PreferencesGroup,
    account: adw::ActionRow,
    active: adw::SwitchRow,
    last_synced: adw::ActionRow,
    sync_now: gtk::Button,
    log_out: adw::ButtonRow,
}

impl SyncPage {
    fn build(&self) {
        self.set_title(&gettext("Sync"));
        self.set_icon_name(Some("network-server-symbolic"));
        let description = gettext(
            "Sync subscriptions, play positions and played episodes with a Nextcloud \
             server that has the GPodder Sync app",
        );

        let server = adw::EntryRow::builder()
            .title(gettext("Server address"))
            .input_purpose(gtk::InputPurpose::Url)
            .build();
        let method = adw::ToggleGroup::builder()
            .valign(gtk::Align::Center)
            .build();
        for (name, label) in [
            (BROWSER, gettext("Browser")),
            (PASSWORD, gettext("Password")),
        ] {
            method.add(adw::Toggle::builder().name(name).label(label).build());
        }
        method.set_active_name(Some(BROWSER));
        // A field label, so sentence capitalization (Writing Style).
        let method_row = adw::ActionRow::builder()
            .title(gettext("Log in with"))
            .build();
        method
            .upcast_ref::<gtk::Widget>()
            .update_property(&[gtk::accessible::Property::Label(&gettext("Log in with"))]);
        method_row.add_suffix(&method);
        let user = adw::EntryRow::builder()
            .title(gettext("Username"))
            .input_purpose(gtk::InputPurpose::FreeForm)
            .visible(false)
            .build();
        let password = adw::PasswordEntryRow::builder()
            .title(gettext("Password"))
            .visible(false)
            .build();
        let log_in = gtk::Button::builder()
            .label(gettext("_Log In"))
            .use_underline(true)
            .halign(gtk::Align::Center)
            .margin_top(24)
            .css_classes(["suggested-action", "pill"])
            .sensitive(false)
            .build();
        let waiting_label = gtk::Label::builder().wrap(true).build();
        let cancel = gtk::Button::builder()
            .label(gettext("_Cancel"))
            .use_underline(true)
            .halign(gtk::Align::Center)
            .build();
        // The label says what it waits for.
        let spinner = adw::Spinner::builder()
            .width_request(24)
            .accessible_role(gtk::AccessibleRole::Presentation)
            .build();
        let status = gtk::Box::builder()
            .spacing(12)
            .halign(gtk::Align::Center)
            .build();
        status.append(&spinner);
        status.append(&waiting_label);
        let waiting = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(24)
            .visible(false)
            .build();
        waiting.append(&status);
        waiting.append(&cancel);

        let signed_out = adw::PreferencesGroup::builder()
            .title(gettext("Nextcloud"))
            .description(&description)
            .build();
        signed_out.add(&server);
        signed_out.add(&method_row);
        signed_out.add(&user);
        signed_out.add(&password);
        signed_out.add(&log_in);
        signed_out.add(&waiting);

        // A button row, as Boxed Lists asks for buttons in a list, in
        // destructive colours: it drops the changes not yet synced, and
        // logging in again goes through the server (Buttons: "Destructive
        // action is a warning").
        let log_out = adw::ButtonRow::builder()
            .title(gettext("Log _Out"))
            .use_underline(true)
            .css_classes(["destructive-action"])
            .build();
        let account = adw::ActionRow::builder().subtitle_selectable(true).build();
        let active = adw::SwitchRow::builder()
            .title(gettext("_Sync"))
            .use_underline(true)
            .build();
        let sync_now = gtk::Button::builder()
            .label(gettext("Sync _Now"))
            .use_underline(true)
            .valign(gtk::Align::Center)
            .build();
        let last_synced = adw::ActionRow::builder()
            .title(gettext("Last synced"))
            .build();
        last_synced.add_suffix(&sync_now);
        // Clicking the row presses its one control (Boxed Lists).
        last_synced.set_activatable_widget(Some(&sync_now));
        let signed_in = adw::PreferencesGroup::builder()
            .title(gettext("Nextcloud"))
            .description(&description)
            .visible(false)
            .build();
        signed_in.add(&account);
        signed_in.add(&active);
        signed_in.add(&last_synced);
        signed_in.add(&log_out);

        self.add(&signed_out);
        self.add(&signed_in);

        let output = self.imp().output.clone();
        let widgets = Widgets {
            signed_out,
            server,
            method,
            user,
            password,
            log_in,
            waiting,
            waiting_label,
            cancel,
            signed_in,
            account,
            active,
            last_synced,
            sync_now,
            log_out,
        };
        self.connect(&widgets, &output);
        let _ = self.imp().widgets.set(widgets);
    }

    fn connect(&self, w: &Widgets, output: &Output<SyncPageOutput>) {
        let page = self.downgrade();
        let changed = move || {
            if let Some(page) = page.upgrade() {
                page.update_log_in();
            }
        };
        w.method.connect_active_name_notify({
            let changed = changed.clone();
            move |_| changed()
        });
        for entry in [
            w.server.upcast_ref::<gtk::Editable>(),
            w.user.upcast_ref(),
            w.password.upcast_ref(),
        ] {
            let changed = changed.clone();
            entry.connect_changed(move |_| changed());
        }
        let page = self.downgrade();
        let log_in = {
            let output = output.clone();
            move || {
                let Some(page) = page.upgrade() else { return };
                let w = page.widgets();
                if !w.log_in.is_sensitive() || !w.log_in.is_visible() {
                    return;
                }
                let server = w.server.text().to_string();
                output.emit(if page.browser() {
                    SyncPageOutput::LogInWithBrowser { server }
                } else {
                    SyncPageOutput::LogInWithPassword {
                        server,
                        user: w.user.text().to_string(),
                        password: w.password.text().to_string(),
                    }
                });
            }
        };
        w.log_in.connect_clicked({
            let log_in = log_in.clone();
            move |_| log_in()
        });
        w.server.connect_entry_activated({
            let log_in = log_in.clone();
            move |_| log_in()
        });
        w.user.connect_entry_activated({
            let log_in = log_in.clone();
            move |_| log_in()
        });
        w.password.connect_entry_activated(move |_| log_in());
        let emit = |message: fn() -> SyncPageOutput| {
            let output = output.clone();
            move |_: &gtk::Button| output.emit(message())
        };
        w.cancel
            .connect_clicked(emit(|| SyncPageOutput::CancelLogIn));
        w.log_out.connect_activated({
            let output = output.clone();
            move |_| output.emit(SyncPageOutput::LogOut)
        });
        w.sync_now.connect_clicked(emit(|| SyncPageOutput::SyncNow));
        let page = self.downgrade();
        let output = output.clone();
        w.active.connect_active_notify(move |row| {
            if page.upgrade().is_some_and(|p| !p.imp().updating.get()) {
                output.emit(SyncPageOutput::SetActive(row.is_active()));
            }
        });
    }

    fn widgets(&self) -> &Widgets {
        self.imp().widgets.get().expect("built when constructed")
    }

    fn browser(&self) -> bool {
        self.widgets().method.active_name().as_deref() != Some(PASSWORD)
    }

    /// Show the rows the chosen way of logging in needs, and make Log In
    /// available once they are filled in.
    fn update_log_in(&self) {
        let browser = self.browser();
        let w = self.widgets();
        w.user.set_visible(!browser);
        w.password.set_visible(!browser);
        let filled = !w.server.text().trim().is_empty()
            && (browser || (!w.user.text().trim().is_empty() && !w.password.text().is_empty()));
        w.log_in.set_sensitive(filled);
    }

    pub fn connect_output(&self, receive: impl Fn(SyncPageOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    pub fn send(&self, input: SyncPageInput) {
        let SyncPageInput::Show(view) = input;
        let w = self.widgets();
        let signed_in = matches!(view, SyncView::SignedIn { .. });
        w.signed_out.set_visible(!signed_in);
        w.signed_in.set_visible(signed_in);
        match view {
            SyncView::SignedOut => {
                w.waiting.set_visible(false);
                w.log_in.set_visible(true);
                self.set_form_sensitive(true);
                // The password is not kept once it has been used.
                w.password.set_text("");
                self.update_log_in();
            }
            SyncView::LoggingIn { browser } => {
                w.waiting_label.set_label(&if browser {
                    gettext("Waiting for approval in the browser…")
                } else {
                    gettext("Logging in…")
                });
                w.cancel.set_visible(browser);
                w.waiting.set_visible(true);
                w.log_in.set_visible(false);
                self.set_form_sensitive(false);
            }
            SyncView::SignedIn {
                user,
                server,
                active,
                last_synced,
                syncing,
            } => {
                w.account.set_title(&glib::markup_escape_text(&user));
                w.account.set_subtitle(&glib::markup_escape_text(&server));
                self.imp().updating.set(true);
                w.active.set_active(active);
                self.imp().updating.set(false);
                let when = if syncing {
                    gettext("Syncing…")
                } else {
                    last_synced.map_or_else(|| gettext("Never"), format::synced_at)
                };
                w.last_synced.set_subtitle(&when);
                w.last_synced.set_sensitive(active);
                w.sync_now.set_sensitive(active && !syncing);
            }
        }
    }

    fn set_form_sensitive(&self, sensitive: bool) {
        let w = self.widgets();
        w.server.set_sensitive(sensitive);
        w.method.set_sensitive(sensitive);
        w.user.set_sensitive(sensitive);
        w.password.set_sensitive(sensitive);
    }
}

#[cfg(test)]
#[path = "sync_page_tests.rs"]
mod tests;
#[cfg(test)]
pub(crate) use tests::the_sync_page_shows_each_state;
