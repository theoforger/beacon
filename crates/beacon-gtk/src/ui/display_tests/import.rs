// SPDX-License-Identifier: GPL-3.0-or-later

//! Import Subscriptions (M8): progress in the content's thin bar, the
//! banner while podcasts are not added, and the list it opens.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;

use super::{NARROWEST, minimum_width};
use crate::ui::content::{Content, ContentInput, ContentOutput};
use crate::ui::dialogs::import::{self, FailuresResponse};

pub(super) fn an_import_shows_in_the_bar_and_the_banner() {
    let content = Content::default();
    let shown = Rc::new(Cell::new(false));
    let s = shown.clone();
    content.connect_output(move |output| {
        if matches!(output, ContentOutput::ShowImportFailures) {
            s.set(true);
        }
    });
    assert!(!content.import_state().0.0, "the bar starts hidden");

    content.send(ContentInput::ImportProgress(Some((12, 48))));
    let ((visible, fraction, tooltip), _) = content.import_state();
    assert!(visible);
    assert!((fraction - 0.25).abs() < 1e-9, "{fraction}");
    assert_eq!(
        tooltip.as_deref(),
        Some("Importing podcasts: 12 of 48 podcasts")
    );

    // A refresh during the import leaves the bar filling, and pulses once
    // the import ends.
    content.send(ContentInput::Refreshing(true));
    assert!((content.import_state().0.1 - 0.25).abs() < 1e-9);
    content.send(ContentInput::ImportProgress(None));
    let ((visible, _, tooltip), _) = content.import_state();
    assert!(visible);
    assert_eq!(tooltip.as_deref(), Some("Fetching feeds…"));
    content.send(ContentInput::Refreshing(false));
    assert!(!content.import_state().0.0);

    content.send(ContentInput::ImportFailures(3));
    assert_eq!(
        content.import_state().1,
        (true, "3 podcasts could not be added".into())
    );
    content.press_import_banner();
    assert!(shown.get());
    content.send(ContentInput::ImportFailures(0));
    assert!(!content.import_state().1.0);
}

pub(super) fn the_failures_list_offers_try_again() {
    let chosen = Rc::new(RefCell::new(Vec::new()));
    let c = chosen.clone();
    let long = "A podcast whose name goes on far longer than any window is wide & then some";
    let dialog = import::failures(
        41,
        44,
        &[
            (long.into(), "Could not reach the feed".into()),
            (
                "Gone Quiet".into(),
                "The feed's server answered with error 404".into(),
            ),
        ],
        move |r| c.borrow_mut().push(r),
    );
    assert_eq!(
        dialog.heading().as_deref(),
        Some("2 Podcasts Could Not Be Added")
    );
    assert_eq!(dialog.body(), "Imported 41 of 44 podcasts");
    assert_eq!(dialog.default_response().as_deref(), Some("retry"));
    assert_eq!(dialog.close_response(), "dismiss");
    let extra = dialog.extra_child().expect("the list");
    assert!(
        minimum_width(&extra) <= NARROWEST,
        "the list needs {}px",
        minimum_width(&extra)
    );
    dialog.emit_by_name::<()>("response", &[&"retry"]);
    dialog.emit_by_name::<()>("response", &[&"dismiss"]);
    assert_eq!(
        *chosen.borrow(),
        [FailuresResponse::Retry, FailuresResponse::Dismiss]
    );
}
