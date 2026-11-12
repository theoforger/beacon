// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::RefCell;
use std::rc::Rc;

use chrono::Utc;

use super::*;

/// Called from `display_tests`, which owns GTK's one thread.
pub(crate) fn the_sync_page_shows_each_state() {
    let page = SyncPage::default();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    page.connect_output(move |output| s.borrow_mut().push(format!("{output:?}")));
    let w = page.widgets();

    // Signed out, the browser way: the server alone enables Log In.
    page.send(SyncPageInput::Show(SyncView::SignedOut));
    assert!(w.signed_out.is_visible() && !w.signed_in.is_visible());
    assert!(!w.user.is_visible() && !w.password.is_visible());
    assert!(!w.log_in.is_sensitive());
    w.server.set_text("cloud.example.com");
    assert!(w.log_in.is_sensitive());

    // The password way needs a username and password too.
    w.method.set_active_name(Some(PASSWORD));
    assert!(w.user.is_visible() && w.password.is_visible());
    assert!(!w.log_in.is_sensitive());
    w.user.set_text("ada");
    w.password.set_text("secret");
    assert!(w.log_in.is_sensitive());
    w.log_in.emit_clicked();
    assert_eq!(
        seen.borrow().last().map(String::as_str),
        Some(
            r#"LogInWithPassword { server: "cloud.example.com", user: "ada", password: "secret" }"#
        )
    );

    // Waiting for the browser: a spinner and Cancel instead of Log In.
    page.send(SyncPageInput::Show(SyncView::LoggingIn { browser: true }));
    assert!(w.waiting.is_visible() && w.cancel.is_visible() && !w.log_in.is_visible());
    assert!(!w.server.is_sensitive());
    w.cancel.emit_clicked();
    assert_eq!(
        seen.borrow().last().map(String::as_str),
        Some("CancelLogIn")
    );
    page.send(SyncPageInput::Show(SyncView::SignedOut));
    assert!(!w.waiting.is_visible() && w.log_in.is_visible());
    assert!(w.password.text().is_empty(), "the password is not kept");

    // Signed in: the account, the switch, Last Synced and Sync Now.
    let count = seen.borrow().len();
    page.send(SyncPageInput::Show(SyncView::SignedIn {
        user: "ada".into(),
        server: "https://cloud.example.com/".into(),
        active: true,
        last_synced: None,
        syncing: false,
    }));
    assert!(!w.signed_out.is_visible() && w.signed_in.is_visible());
    // Log Out is the signed-in group's one coloured button, a warning.
    assert!(w.log_out.has_css_class("destructive-action"));
    // Added to the row's own classes, not in place of them: replacing them
    // drew Log Out plain and cramped (found in the M7 review).
    for class in adw::ButtonRow::new().css_classes() {
        assert!(w.log_out.has_css_class(&class), "Log Out lost .{class}");
    }
    assert_eq!(w.account.title(), "ada");
    assert_eq!(
        w.last_synced.subtitle().as_deref(),
        Some(gettext("Never").as_str())
    );
    assert!(w.active.is_active() && w.sync_now.is_sensitive());
    assert_eq!(seen.borrow().len(), count, "showing state emits nothing");
    w.active.set_active(false);
    assert_eq!(
        seen.borrow().last().map(String::as_str),
        Some("SetActive(false)")
    );
    page.send(SyncPageInput::Show(SyncView::SignedIn {
        user: "ada".into(),
        server: "https://cloud.example.com/".into(),
        active: true,
        last_synced: Some(Utc::now()),
        syncing: true,
    }));
    assert_eq!(
        w.last_synced.subtitle().as_deref(),
        Some(gettext("Syncing…").as_str())
    );
    assert!(!w.sync_now.is_sensitive(), "one sync at a time");
}
