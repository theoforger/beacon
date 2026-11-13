// SPDX-License-Identifier: GPL-3.0-or-later

//! The podcasts an OPML import could not add (`docs/specs/opml.md`),
//! opened from the banner that says some failed: each with its reason,
//! then Dismiss or Try Again. The import's progress is the content's
//! refresh bar, not a dialog: the HIG's Progress Bars page says "In
//! general, progress windows are not recommended".

use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::glib;

/// What was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailuresResponse {
    /// Forget the failures; the banner goes.
    Dismiss,
    /// Try the failed feeds again.
    Retry,
}

const DISMISS: &str = "dismiss";
const RETRY: &str = "retry";

/// The dialog listing `failures` (name, reason), `imported` of `total`
/// having gone in, reporting the choice to `respond`.
pub fn failures(
    imported: usize,
    total: usize,
    failures: &[(String, String)],
    respond: impl Fn(FailuresResponse) + 'static,
) -> adw::AlertDialog {
    let n = failures.len() as u32;
    let dialog = adw::AlertDialog::new(
        Some(
            &ngettext(
                "{n} Podcast Could Not Be Added",
                "{n} Podcasts Could Not Be Added",
                n,
            )
            .replace("{n}", &n.to_string()),
        ),
        Some(&imported_message(imported, total)),
    );
    let list = gtk::ListBox::builder()
        .css_classes(["boxed-list"])
        .selection_mode(gtk::SelectionMode::None)
        .build();
    // Translators: how a screen reader names the list of podcasts an
    // import could not add.
    list.update_property(&[gtk::accessible::Property::Label(&gettext(
        "Podcasts Not Added",
    ))]);
    for (name, reason) in failures {
        list.append(
            &adw::ActionRow::builder()
                .title(glib::markup_escape_text(name))
                .subtitle(glib::markup_escape_text(reason))
                .build(),
        );
    }
    // A long list scrolls rather than pushing the buttons off the screen.
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(320)
        .child(&list)
        .build();
    dialog.set_extra_child(Some(&scrolled));
    dialog.add_response(DISMISS, &gettext("_Dismiss"));
    dialog.add_response(RETRY, &gettext("_Try Again"));
    dialog.set_response_appearance(RETRY, adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some(RETRY));
    dialog.set_close_response(DISMISS);
    dialog.connect_response(None, move |_, response| {
        respond(if response == RETRY {
            FailuresResponse::Retry
        } else {
            FailuresResponse::Dismiss
        });
    });
    dialog
}

/// What the banner says while some podcasts of an import are not added.
pub fn banner_title(failed: usize) -> String {
    ngettext(
        "{n} podcast could not be added",
        "{n} podcasts could not be added",
        failed as u32,
    )
    .replace("{n}", &failed.to_string())
}

/// How far an import has got, for the refresh bar's label: the HIG's
/// Progress Bars page asks for one naming what is counted.
pub fn progress_message(done: usize, total: usize) -> String {
    // Translators: how far an import has got, such as "12 of 44 podcasts".
    ngettext(
        "{done} of {total} podcast",
        "{done} of {total} podcasts",
        total as u32,
    )
    .replace("{done}", &done.to_string())
    .replace("{total}", &total.to_string())
}

/// How many of the file's new podcasts are in the library now.
pub fn imported_message(imported: usize, total: usize) -> String {
    if imported == total {
        ngettext(
            "Imported {n} podcast",
            "Imported {n} podcasts",
            total as u32,
        )
    } else {
        // Translators: {n} podcasts of {total} were imported.
        ngettext(
            "Imported {n} of {total} podcast",
            "Imported {n} of {total} podcasts",
            total as u32,
        )
    }
    .replace("{n}", &imported.to_string())
    .replace("{total}", &total.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_says_how_many_were_imported() {
        assert_eq!(imported_message(1, 1), "Imported 1 podcast");
        assert_eq!(imported_message(44, 44), "Imported 44 podcasts");
        assert_eq!(imported_message(41, 44), "Imported 41 of 44 podcasts");
        assert_eq!(imported_message(0, 3), "Imported 0 of 3 podcasts");
    }

    #[test]
    fn progress_and_the_banner_count_podcasts() {
        assert_eq!(progress_message(12, 44), "12 of 44 podcasts");
        assert_eq!(progress_message(0, 1), "0 of 1 podcast");
        assert_eq!(banner_title(1), "1 podcast could not be added");
        assert_eq!(banner_title(3), "3 podcasts could not be added");
    }
}
