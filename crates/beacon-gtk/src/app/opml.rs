// SPDX-License-Identifier: GPL-3.0-or-later

//! Import and Export Subscriptions (`docs/specs/opml.md`): choosing the
//! file through `GtkFileDialog` (the portal, in Flatpak), reading or
//! writing it in the core, and what an import shows: its progress in the
//! content's thin bar, over Shows, where the new shows arrive; a toast when
//! it ends; and, while some podcasts are not added, a banner whose Show
//! lists them with Try Again (decided by the user 2026-11-12, replacing a
//! progress dialog the HIG advises against).

use adw::prelude::*;
use beacon_core::LibraryError;
use beacon_core::OpmlError;
use beacon_core::model::ShowId;
use beacon_core::services::opml::{ImportPlan, OpmlFeed};
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};

use super::library::Parts;
use super::{App, AppCommand, AppMsg, toast};
use crate::navigation::Location;
use crate::ui::content::ContentInput;
use crate::ui::dialogs::import::{self, FailuresResponse};
use crate::ui::errors;

/// The import under way, or the last one while its failures stand.
#[derive(Default)]
pub(super) struct Import {
    /// New feeds the file listed.
    total: usize,
    /// Of those, how many are in the library now.
    imported: usize,
    /// How many of this run's feeds have finished, of `running`.
    done: usize,
    running: Option<usize>,
    failures: Vec<(OpmlFeed, LibraryError)>,
}

#[derive(Debug)]
pub enum OpmlMsg {
    /// Import Subscriptions in the main menu: choose a file.
    ChooseImport,
    /// Import this file, chosen or dropped on the Add page.
    Import(gio::File),
    /// The banner's Show: list the podcasts not added.
    ShowFailures,
    /// The list's Try Again or Dismiss.
    Failures(FailuresResponse),
    /// Export Subscriptions in the main menu.
    Export,
    /// A feed address may hold a secret: warn first.
    WarnExport,
    /// Warned, or nothing to warn about: choose where to save.
    ChooseExport,
    ExportTo(gio::File),
}

#[derive(Debug)]
pub enum OpmlCommand {
    Read(Result<ImportPlan, OpmlError>),
    /// One feed of the import finished.
    Subscribed(OpmlFeed, Result<ShowId, LibraryError>),
    /// Every feed of the run finished.
    Finished,
    /// Whether a feed address may hold a secret.
    Checked(Result<bool, beacon_core::StoreError>),
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
            OpmlMsg::ChooseImport => {
                let dialog = gtk::FileDialog::builder()
                    .title(gettext("Import Subscriptions"))
                    .accept_label(gettext("_Import"))
                    .filters(&filters())
                    .modal(true)
                    .build();
                let (root, sender) = (root.clone(), self.sender.clone());
                glib::spawn_future_local(async move {
                    if let Ok(file) = dialog.open_future(Some(&root)).await {
                        sender.input(AppMsg::Opml(OpmlMsg::Import(file)));
                    }
                });
            }
            OpmlMsg::Import(file) => self.read_file(&file),
            OpmlMsg::ShowFailures => self.show_failures(root),
            OpmlMsg::Failures(response) => {
                if self.import.as_ref().is_some_and(|i| i.running.is_some()) {
                    return;
                }
                let failures = self
                    .import
                    .as_mut()
                    .map(|i| std::mem::take(&mut i.failures))
                    .unwrap_or_default();
                self.content.send(ContentInput::ImportFailures(0));
                if response == FailuresResponse::Retry && !failures.is_empty() {
                    self.start_import(failures.into_iter().map(|(feed, _)| feed).collect());
                }
            }
            OpmlMsg::Export => {
                let Some(library) = self.library.clone() else {
                    return;
                };
                self.sender.spawn_blocking(move || {
                    AppCommand::Opml(OpmlCommand::Checked(library.has_secret_feeds()))
                });
            }
            OpmlMsg::WarnExport => self.warn_before_export(root),
            OpmlMsg::ChooseExport => {
                let dialog = gtk::FileDialog::builder()
                    .title(gettext("Export Subscriptions"))
                    .accept_label(gettext("_Export"))
                    // Translators: the suggested name of an exported file.
                    .initial_name(format!("{}.opml", gettext("podcasts")))
                    .filters(&filters())
                    .modal(true)
                    .build();
                let (root, sender) = (root.clone(), self.sender.clone());
                glib::spawn_future_local(async move {
                    if let Ok(file) = dialog.save_future(Some(&root)).await {
                        sender.input(AppMsg::Opml(OpmlMsg::ExportTo(file)));
                    }
                });
            }
            OpmlMsg::ExportTo(file) => {
                let Some(library) = self.library.clone() else {
                    return;
                };
                let Some(path) = file.path() else {
                    let err = OpmlError::Io(std::io::ErrorKind::Unsupported.into());
                    toast(&self.widgets.toasts, &errors::opml(&err, true));
                    return;
                };
                self.sender.spawn_blocking(move || {
                    AppCommand::Opml(OpmlCommand::Exported(library.export_opml(&path)))
                });
            }
        }
    }

    pub(super) fn opml_command(&mut self, command: OpmlCommand) {
        match command {
            OpmlCommand::Read(result) => self.file_read(result),
            OpmlCommand::Subscribed(feed, result) => self.feed_imported(feed, result),
            OpmlCommand::Finished => self.import_finished(),
            OpmlCommand::Checked(Ok(warn)) => self.sender.input(AppMsg::Opml(if warn {
                OpmlMsg::WarnExport
            } else {
                OpmlMsg::ChooseExport
            })),
            OpmlCommand::Checked(Err(err)) => toast(&self.widgets.toasts, &errors::store(&err)),
            OpmlCommand::Exported(Ok(n)) => toast(
                &self.widgets.toasts,
                &ngettext("Exported {n} podcast", "Exported {n} podcasts", n as u32)
                    .replace("{n}", &n.to_string()),
            ),
            OpmlCommand::Exported(Err(err)) => {
                tracing::warn!("could not export subscriptions: {err}");
                toast(&self.widgets.toasts, &errors::opml(&err, true));
            }
        }
    }

    fn importing(&self) -> bool {
        self.import.as_ref().is_some_and(|i| i.running.is_some())
    }

    /// Read `file` in the background; one import at a time.
    fn read_file(&mut self, file: &gio::File) {
        let Some(library) = self.library.clone() else {
            return;
        };
        if self.importing() {
            toast(
                &self.widgets.toasts,
                &gettext("Wait for the import under way to finish"),
            );
            return;
        }
        let path = file.path();
        self.sender.spawn_blocking(move || {
            let result = match path {
                Some(path) => library.read_opml(&path),
                None => Err(OpmlError::Io(std::io::ErrorKind::Unsupported.into())),
            };
            AppCommand::Opml(OpmlCommand::Read(result))
        });
    }

    /// The file's been read: nothing to do, or import what is new. A file
    /// that cannot be read, or holds nothing new, is a toast: the HIG's
    /// Dialogs page, "For simple non-critical errors, toasts can be a good
    /// alternative".
    fn file_read(&mut self, result: Result<ImportPlan, OpmlError>) {
        match result {
            Err(err) => {
                tracing::warn!("could not read the OPML file: {err}");
                toast(&self.widgets.toasts, &errors::opml(&err, false));
            }
            Ok(plan) if plan.feeds.is_empty() => {
                let listed = plan.subscribed + plan.repeated;
                toast(
                    &self.widgets.toasts,
                    &if listed == 0 {
                        gettext("The file lists no podcasts")
                    } else {
                        gettext("Every podcast in the file is already in your library")
                    },
                );
            }
            Ok(plan) if self.importing() => drop(plan),
            Ok(plan) => {
                // A new import replaces what the last one left out.
                self.import = Some(Import {
                    total: plan.feeds.len(),
                    ..Import::default()
                });
                self.content.send(ContentInput::ImportFailures(0));
                // Shows, where the new shows arrive under the bar.
                self.sender.input(AppMsg::Navigate(Location::Shows));
                self.start_import(plan.feeds);
            }
        }
    }

    /// Subscribe to `feeds` in the background, the bar following.
    fn start_import(&mut self, feeds: Vec<OpmlFeed>) {
        let (Some(library), Some(import)) = (self.library.clone(), self.import.as_mut()) else {
            return;
        };
        let total = feeds.len();
        import.running = Some(total);
        import.done = 0;
        self.content
            .send(ContentInput::ImportProgress(Some((0, total))));
        let sender = self.sender.clone();
        self.sender.spawn(async move {
            library
                .import(feeds, |feed, result| {
                    sender.command(AppCommand::Opml(OpmlCommand::Subscribed(feed, result)));
                })
                .await;
            AppCommand::Opml(OpmlCommand::Finished)
        });
    }

    fn feed_imported(&mut self, feed: OpmlFeed, result: Result<ShowId, LibraryError>) {
        let Some(import) = self.import.as_mut() else {
            return;
        };
        import.done += 1;
        match result {
            // Subscribed to meanwhile, by sync or by hand: it is there.
            Ok(_) | Err(LibraryError::AlreadySubscribed(_)) => import.imported += 1,
            Err(err) => {
                tracing::warn!("could not import {}: {err}", feed.url);
                import.failures.push((feed, err));
            }
        }
        if let Some(total) = import.running {
            self.content
                .send(ContentInput::ImportProgress(Some((import.done, total))));
        }
        // The Shows grid fills in as the import goes.
        self.reload(Parts {
            shows: true,
            counts: true,
            ..Parts::NONE
        });
    }

    fn import_finished(&mut self) {
        let Some(import) = self.import.as_mut() else {
            return;
        };
        import.running = None;
        self.content.send(ContentInput::ImportProgress(None));
        self.content
            .send(ContentInput::ImportFailures(import.failures.len()));
        toast(
            &self.widgets.toasts,
            &import::imported_message(import.imported, import.total),
        );
        self.reload(Parts::ALL);
        self.request_sync(false);
    }

    /// The banner's Show: the podcasts not added, with Try Again.
    fn show_failures(&self, root: &adw::ApplicationWindow) {
        let Some(import) = self.import.as_ref().filter(|i| !i.failures.is_empty()) else {
            return;
        };
        let failures: Vec<_> = import
            .failures
            .iter()
            .map(|(feed, err)| (feed.name().to_owned(), errors::library(err)))
            .collect();
        let s = self.sender.clone();
        let dialog = import::failures(import.imported, import.total, &failures, move |r| {
            s.input(AppMsg::Opml(OpmlMsg::Failures(r)));
        });
        dialog.present(Some(root));
    }

    /// Exported feed addresses can give away a paid feed's key: say so
    /// before the file is saved, when one looks like it holds one.
    fn warn_before_export(&self, root: &adw::ApplicationWindow) {
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Export Private Feed Addresses?")),
            Some(&gettext(
                "Some feed addresses look like they contain a private key, as those of paid podcasts do. Anyone with the file can listen to those podcasts as you, so share it only with people you trust",
            )),
        );
        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("export", &gettext("_Export"));
        dialog.set_response_appearance("export", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("export"));
        dialog.set_close_response("cancel");
        let s = self.sender.clone();
        dialog.connect_response(Some("export"), move |_, _| {
            s.input(AppMsg::Opml(OpmlMsg::ChooseExport));
        });
        dialog.present(Some(root));
    }
}
