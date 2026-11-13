// SPDX-License-Identifier: GPL-3.0-or-later

//! Import and Export Subscriptions (M8): the dialogs that ask for a file
//! and list the podcasts to choose, and the import's progress in the
//! content's thin bar.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

use super::{NARROWEST, minimum_width};
use crate::ui::content::{Content, ContentInput};
use crate::ui::dialogs::export::{ExportDialog, ExportInput, ExportOutput};
use crate::ui::dialogs::import::{ImportDialog, ImportInput, ImportOutput};
use crate::ui::feed_picker::PickItem;

fn item(title: &str, url: &str) -> PickItem {
    PickItem {
        title: title.into(),
        url: url.into(),
        ..PickItem::default()
    }
}

const LONG: &str = "A podcast whose name goes on far longer than any window is wide & then some";

pub(super) fn an_import_shows_its_progress_in_the_bar() {
    let content = Content::default();
    assert!(!content.import_state().0, "the bar starts hidden");
    content.send(ContentInput::ImportProgress(Some((12, 48))));
    let (visible, fraction, tooltip) = content.import_state();
    assert!(visible);
    assert!((fraction - 0.25).abs() < 1e-9, "{fraction}");
    assert_eq!(
        tooltip.as_deref(),
        Some("Importing podcasts: 12 of 48 podcasts")
    );
    // A refresh meanwhile leaves the import's progress showing.
    content.send(ContentInput::Refreshing(true));
    assert!((content.import_state().1 - 0.25).abs() < 1e-9);
    content.send(ContentInput::ImportProgress(None));
    let (visible, _, tooltip) = content.import_state();
    assert!(visible);
    assert_eq!(tooltip.as_deref(), Some("Fetching feeds…"));
    content.send(ContentInput::Refreshing(false));
    assert!(!content.import_state().0);
}

/// The file page, then the list: rows fill in as feeds load, one that
/// fails is unchecked, and Import sends what is checked.
pub(super) fn the_import_dialog_asks_for_a_file_then_lists_its_podcasts() {
    let outputs: Rc<RefCell<Vec<ImportOutput>>> = Rc::default();
    let o = outputs.clone();
    let dialog = ImportDialog::default();
    dialog.connect_output(move |output| o.borrow_mut().push(output));

    let (choosing, can_import, _, loading) = dialog.state();
    assert!(choosing, "it starts on the file page");
    assert!(!can_import, "nothing to import yet");
    assert!(!loading);
    assert!(dialog.drop_area_has_target(), "a file can be dropped on it");
    dialog.press_choose();
    assert!(matches!(
        outputs.borrow().as_slice(),
        [ImportOutput::ChooseFile]
    ));

    dialog.send(ImportInput::Feeds {
        items: vec![
            item("Small Hours", "https://a.example/small.xml"),
            item(LONG, "https://a.example/long.xml"),
            item("Gone Quiet", "https://a.example/gone.xml"),
        ],
        note: Some("1 podcast in the file is already in your library".into()),
    });
    dialog.send(ImportInput::Loading(Some((0, 3))));
    let (choosing, can_import, subtitle, loading) = dialog.state();
    assert!(!choosing, "the list shows");
    assert!(can_import);
    assert_eq!(subtitle, "3 selected");
    assert!(loading);

    // Previews: the feed's own title, and one that failed.
    dialog.send(ImportInput::Feed(
        0,
        item("Small Hours (Official)", "https://a.example/small.xml"),
    ));
    dialog.send(ImportInput::Feed(
        2,
        PickItem {
            problem: Some("The feed's server answered with error 404".into()),
            ..item("Gone Quiet", "https://a.example/gone.xml")
        },
    ));
    dialog.send(ImportInput::Loading(None));
    let picker = dialog.picker();
    assert_eq!(
        picker.rows(),
        [
            (
                "Small Hours (Official)".into(),
                "https://a.example/small.xml".into(),
                true,
                false
            ),
            (
                LONG.into(),
                "https://a.example/long.xml".into(),
                true,
                false
            ),
            (
                "Gone Quiet".into(),
                "https://a.example/gone.xml".into(),
                false,
                true
            ),
        ]
    );
    let (_, _, subtitle, loading) = dialog.state();
    assert_eq!(subtitle, "2 selected");
    assert!(!loading);

    // A row's background toggles its check box.
    picker.activate_row(0);
    assert_eq!(dialog.state().2, "1 selected");
    assert_eq!(picker.press_toggle_all(), "Select _All");
    assert_eq!(dialog.state().2, "3 selected");
    assert_eq!(picker.press_toggle_all(), "Select _None");
    let (_, can_import, subtitle, _) = dialog.state();
    assert!(!can_import, "nothing chosen");
    assert_eq!(subtitle, "None selected");
    picker.activate_row(1);
    dialog.press_import();
    assert!(matches!(
        outputs.borrow().last(),
        Some(ImportOutput::Import(chosen)) if chosen == &[1]
    ));
    assert!(
        minimum_width(&dialog.child().unwrap()) <= NARROWEST,
        "the dialog needs {}px",
        minimum_width(&dialog.child().unwrap())
    );
}

pub(super) fn the_export_dialog_lists_every_show_checked() {
    let outputs: Rc<RefCell<Vec<ExportOutput>>> = Rc::default();
    let o = outputs.clone();
    let dialog = ExportDialog::default();
    dialog.connect_output(move |output| o.borrow_mut().push(output));
    dialog.send(ExportInput::Shows(vec![
        item("Small Hours", "https://a.example/small.xml"),
        item(LONG, "https://a.example/long.xml?key=private"),
    ]));
    assert!(dialog.can_export());
    let picker = dialog.picker();
    assert!(picker.rows().iter().all(|r| r.2 && !r.3), "all checked");
    picker.activate_row(1);
    dialog.press_export();
    assert!(matches!(
        outputs.borrow().as_slice(),
        [ExportOutput::Export(chosen)] if chosen == &[0]
    ));
    picker.activate_row(0);
    assert!(!dialog.can_export(), "nothing chosen");
    assert!(
        minimum_width(&dialog.child().unwrap()) <= NARROWEST,
        "the dialog needs {}px",
        minimum_width(&dialog.child().unwrap())
    );
}
