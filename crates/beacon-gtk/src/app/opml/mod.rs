// SPDX-License-Identifier: GPL-3.0-or-later

//! Import and Export Subscriptions (`docs/specs/opml.md`). Each opens a
//! dialog listing the podcasts it would take, to choose from (decided by
//! the user 2026-11-13). Import first asks for the file, dropped on the
//! dialog or chosen through `GtkFileDialog` (the portal, in Flatpak), then
//! loads each new feed's preview, four at a time, so its row shows the
//! feed's own title and cover; importing subscribes from those previews
//! without fetching them again. The chosen feeds go in with their progress
//! in the content's thin bar, and a toast when done.

use adw::prelude::*;
use beacon_core::model::{Show, ShowId};
use beacon_core::services::library::FeedPreview;
use beacon_core::services::opml::{ImportPlan, OpmlFeed};
use beacon_core::{LibraryError, OpmlError, StoreError};
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};

mod import;

use super::{App, AppCommand, AppMsg, toast};
use crate::ui::dialogs::export::{ExportDialog, ExportInput, ExportOutput};
use crate::ui::dialogs::import::{ImportDialog, ImportInput, ImportOutput};
use crate::ui::errors;
use crate::ui::feed_picker::PickItem;

/// The dialogs open and the import under way.
#[derive(Default)]
pub(super) struct Opml {
    choosing: Option<Choosing>,
    exporting: Option<Exporting>,
    /// The import under way: imported and finished, of how many.
    running: Option<(usize, usize, usize)>,
    /// Tells a file's previews from those of a file read before it.
    generation: u64,
}

/// The Import dialog, and what it lists.
struct Choosing {
    dialog: glib::WeakRef<ImportDialog>,
    generation: u64,
    feeds: Vec<OpmlFeed>,
    /// Each feed's preview, once loaded.
    previews: Vec<Option<FeedPreview>>,
    loaded: usize,
    loading: Option<tokio::task::AbortHandle>,
}

struct Exporting {
    dialog: glib::WeakRef<ExportDialog>,
    shows: Vec<ShowId>,
}

#[derive(Debug)]
pub enum OpmlMsg {
    /// Import Subscriptions in the main menu.
    Import,
    ImportDialog(ImportOutput),
    /// Read this file, chosen or dropped.
    Read(gio::File),
    /// Export Subscriptions in the main menu.
    Export,
    ExportDialog(ExportOutput),
    /// Write these shows to this file.
    ExportTo(gio::File, Vec<ShowId>),
}

#[derive(Debug)]
pub enum OpmlCommand {
    Read(u64, Result<ImportPlan, OpmlError>),
    /// One feed's preview, by its place in the list.
    Previewed(u64, usize, Box<Result<FeedPreview, LibraryError>>),
    /// Every preview of the file has loaded.
    PreviewsDone(u64),
    /// One feed of the import finished.
    Subscribed(Result<ShowId, LibraryError>),
    /// Every feed of the import finished.
    Finished,
    Shows(Result<Vec<Show>, StoreError>),
    Exported(Result<usize, OpmlError>),
}

/// The file chooser's filter, as GNOME Podcasts' (`utils.rs`), with the
/// extension for systems that do not know OPML's type.
fn filters() -> gio::ListStore {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some(&gettext("OPML Files")));
    for mime in [
        "text/x-opml",
        "text/x-opml+xml",
        "application/xml",
        "text/xml",
    ] {
        filter.add_mime_type(mime);
    }
    filter.add_suffix("opml");
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    filters
}

impl App {
    pub(super) fn opml_message(&mut self, message: OpmlMsg, root: &adw::ApplicationWindow) {
        match message {
            OpmlMsg::Import => self.open_import(root),
            OpmlMsg::ImportDialog(output) => self.import_output(output, root),
            OpmlMsg::Read(file) => self.read_file(&file),
            OpmlMsg::Export => {
                let Some(library) = self.library.clone() else {
                    return;
                };
                self.sender
                    .spawn_blocking(move || AppCommand::Opml(OpmlCommand::Shows(library.shows())));
            }
            OpmlMsg::ExportDialog(ExportOutput::Export(chosen)) => {
                let Some(exporting) = &self.opml.exporting else {
                    return;
                };
                let ids: Vec<_> = chosen
                    .iter()
                    .filter_map(|&i| exporting.shows.get(i).copied())
                    .collect();
                let file_dialog = gtk::FileDialog::builder()
                    .title(gettext("Export Subscriptions"))
                    .accept_label(gettext("_Export"))
                    // Translators: the suggested name of an exported file.
                    .initial_name(format!("{}.opml", gettext("podcasts")))
                    .filters(&filters())
                    .modal(true)
                    .build();
                let (root, sender) = (root.clone(), self.sender.clone());
                glib::spawn_future_local(async move {
                    if let Ok(file) = file_dialog.save_future(Some(&root)).await {
                        sender.input(AppMsg::Opml(OpmlMsg::ExportTo(file, ids)));
                    }
                });
            }
            OpmlMsg::ExportTo(file, ids) => {
                let Some(library) = self.library.clone() else {
                    return;
                };
                let Some(path) = file.path() else {
                    let err = OpmlError::Io(std::io::ErrorKind::Unsupported.into());
                    self.export_problem(&err);
                    return;
                };
                self.sender.spawn_blocking(move || {
                    AppCommand::Opml(OpmlCommand::Exported(library.export_opml(&path, &ids)))
                });
            }
        }
    }

    pub(super) fn opml_command(&mut self, command: OpmlCommand) {
        match command {
            OpmlCommand::Read(generation, result) => self.file_read(generation, result),
            OpmlCommand::Previewed(generation, i, result) => {
                self.feed_previewed(generation, i, *result)
            }
            OpmlCommand::PreviewsDone(generation) => {
                if let Some(choosing) = self.opml.choosing.as_mut()
                    && choosing.generation == generation
                {
                    choosing.loading = None;
                    if let Some(dialog) = choosing.dialog.upgrade() {
                        dialog.send(ImportInput::Loading(None));
                    }
                }
            }
            OpmlCommand::Subscribed(result) => self.feed_imported(result),
            OpmlCommand::Finished => self.import_finished(),
            OpmlCommand::Shows(Ok(shows)) => self.open_export(shows),
            OpmlCommand::Shows(Err(err)) => toast(&self.widgets.toasts, &errors::store(&err)),
            OpmlCommand::Exported(Ok(n)) => {
                if let Some(dialog) = self.opml.exporting.take().and_then(|e| e.dialog.upgrade()) {
                    dialog.close();
                }
                toast(
                    &self.widgets.toasts,
                    &ngettext("Exported {n} podcast", "Exported {n} podcasts", n as u32)
                        .replace("{n}", &n.to_string()),
                );
            }
            OpmlCommand::Exported(Err(err)) => {
                tracing::warn!("could not export subscriptions: {err}");
                self.export_problem(&err);
            }
        }
    }

    fn open_export(&mut self, shows: Vec<Show>) {
        let dialog = ExportDialog::default();
        let sender = self.sender.clone();
        dialog.connect_output(move |output| {
            sender.input(AppMsg::Opml(OpmlMsg::ExportDialog(output)));
        });
        let items = shows
            .iter()
            .map(|show| PickItem {
                title: show.info.title.clone(),
                url: show.feed_url.to_string(),
                cover: show.cover_path.clone(),
                problem: None,
            })
            .collect();
        dialog.send(ExportInput::Shows(items));
        self.opml.exporting = Some(Exporting {
            dialog: dialog.downgrade(),
            shows: shows.iter().map(|s| s.id).collect(),
        });
        dialog.present(Some(&self.widgets.toasts));
    }

    fn export_problem(&self, err: &OpmlError) {
        let message = errors::opml(err, true);
        match self
            .opml
            .exporting
            .as_ref()
            .and_then(|e| e.dialog.upgrade())
        {
            Some(dialog) => dialog.send(ExportInput::Problem(message)),
            None => toast(&self.widgets.toasts, &message),
        }
    }
}
