// SPDX-License-Identifier: GPL-3.0-or-later

//! Import Subscriptions: the dialog, the file it reads, each feed's
//! preview, and the import itself (`docs/specs/opml.md`).

use adw::prelude::*;
use beacon_core::model::ShowId;
use beacon_core::services::library::{FeedPreview, ToImport};
use beacon_core::services::opml::ImportPlan;
use beacon_core::{LibraryError, OpmlError};
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};

use super::{Choosing, OpmlCommand, OpmlMsg, filters};
use crate::app::library::Parts;
use crate::app::{App, AppCommand, AppMsg, toast};
use crate::navigation::Location;
use crate::ui::content::ContentInput;
use crate::ui::dialogs::import::{self, ImportDialog, ImportInput, ImportOutput};
use crate::ui::errors;
use crate::ui::feed_picker::PickItem;

impl App {
    pub(super) fn open_import(&mut self, root: &adw::ApplicationWindow) {
        if let Some(dialog) = self.import_dialog() {
            dialog.present(Some(root));
            return;
        }
        let dialog = ImportDialog::default();
        let sender = self.sender.clone();
        dialog.connect_output(move |output| {
            sender.input(AppMsg::Opml(OpmlMsg::ImportDialog(output)));
        });
        self.opml.generation += 1;
        self.opml.choosing = Some(Choosing {
            dialog: dialog.downgrade(),
            generation: self.opml.generation,
            feeds: Vec::new(),
            previews: Vec::new(),
            loaded: 0,
            loading: None,
        });
        dialog.present(Some(root));
    }

    pub(super) fn import_dialog(&self) -> Option<ImportDialog> {
        self.opml.choosing.as_ref()?.dialog.upgrade()
    }

    pub(super) fn import_output(&mut self, output: ImportOutput, root: &adw::ApplicationWindow) {
        match output {
            ImportOutput::ChooseFile => {
                let file_dialog = gtk::FileDialog::builder()
                    .title(gettext("Import Subscriptions"))
                    .accept_label(gettext("_Open"))
                    .filters(&filters())
                    .modal(true)
                    .build();
                let (root, sender) = (root.clone(), self.sender.clone());
                glib::spawn_future_local(async move {
                    if let Ok(file) = file_dialog.open_future(Some(&root)).await {
                        sender.input(AppMsg::Opml(OpmlMsg::Read(file)));
                    }
                });
            }
            ImportOutput::Dropped(file) => self.read_file(&file),
            ImportOutput::Import(chosen) => self.start_import(&chosen),
            ImportOutput::Closed => {
                if let Some(loading) = self.opml.choosing.take().and_then(|c| c.loading) {
                    loading.abort();
                }
            }
        }
    }

    /// Read `file` in the background.
    pub(super) fn read_file(&mut self, file: &gio::File) {
        let (Some(library), Some(choosing)) = (self.library.clone(), &self.opml.choosing) else {
            return;
        };
        let generation = choosing.generation;
        let path = file.path();
        self.sender.spawn_blocking(move || {
            let result = match path {
                Some(path) => library.read_opml(&path),
                None => Err(OpmlError::Io(std::io::ErrorKind::Unsupported.into())),
            };
            AppCommand::Opml(OpmlCommand::Read(generation, result))
        });
    }

    /// The file's been read: say what is wrong with it, or list its new
    /// podcasts and load their previews.
    pub(super) fn file_read(&mut self, generation: u64, result: Result<ImportPlan, OpmlError>) {
        let Some(dialog) = self.import_dialog() else {
            return;
        };
        let Some(choosing) = self
            .opml
            .choosing
            .as_mut()
            .filter(|c| c.generation == generation)
        else {
            return;
        };
        let plan = match result {
            Ok(plan) if !plan.feeds.is_empty() => plan,
            Ok(plan) => {
                let message = if plan.subscribed + plan.repeated == 0 {
                    gettext("The file lists no podcasts")
                } else {
                    gettext("No new podcasts in the file")
                };
                dialog.send(ImportInput::Problem(message));
                return;
            }
            Err(err) => {
                tracing::warn!("could not read the OPML file: {err}");
                dialog.send(ImportInput::Problem(errors::opml(&err, false)));
                return;
            }
        };
        let items = plan
            .feeds
            .iter()
            .map(|feed| PickItem {
                title: feed.name().to_owned(),
                url: feed.url.to_string(),
                ..PickItem::default()
            })
            .collect();
        let note = (plan.subscribed > 0).then(|| {
            ngettext(
                "{n} podcast in the file is already in your library",
                "{n} podcasts in the file are already in your library",
                plan.subscribed as u32,
            )
            .replace("{n}", &plan.subscribed.to_string())
        });
        dialog.send(ImportInput::Feeds { items, note });
        let total = plan.feeds.len();
        dialog.send(ImportInput::Loading(Some((0, total))));
        choosing.previews = (0..total).map(|_| None).collect();
        choosing.loaded = 0;
        let urls = plan.feeds.iter().map(|f| f.url.clone()).collect();
        choosing.feeds = plan.feeds;
        if let Some(library) = self.library.clone() {
            let sender = self.sender.clone();
            choosing.loading = Some(self.sender.spawn(async move {
                library
                    .preview_feeds(urls, |i, result| {
                        sender.command(AppCommand::Opml(OpmlCommand::Previewed(
                            generation,
                            i,
                            Box::new(result),
                        )));
                    })
                    .await;
                AppCommand::Opml(OpmlCommand::PreviewsDone(generation))
            }));
        }
    }

    pub(super) fn feed_previewed(
        &mut self,
        generation: u64,
        i: usize,
        result: Result<FeedPreview, LibraryError>,
    ) {
        let Some(dialog) = self.import_dialog() else {
            return;
        };
        let Some(choosing) = self
            .opml
            .choosing
            .as_mut()
            .filter(|c| c.generation == generation)
        else {
            return;
        };
        let Some(feed) = choosing.feeds.get(i) else {
            return;
        };
        choosing.loaded += 1;
        let mut item = PickItem {
            title: feed.name().to_owned(),
            url: feed.url.to_string(),
            ..PickItem::default()
        };
        match result {
            Ok(preview) => {
                item.title = preview.feed.show.title.clone();
                item.cover = preview.cover.clone();
                if preview.subscribed.is_some() {
                    item.problem = Some(gettext("Already in your library"));
                }
                choosing.previews[i] = Some(preview);
            }
            Err(err) => {
                tracing::warn!("could not load {}: {err}", feed.url);
                item.problem = Some(errors::library(&err));
            }
        }
        dialog.send(ImportInput::Feed(i, item));
        let progress = (choosing.loaded < choosing.feeds.len())
            .then_some((choosing.loaded, choosing.feeds.len()));
        dialog.send(ImportInput::Loading(progress));
    }

    /// Import: close the dialog and subscribe to the chosen feeds, from
    /// their previews when loaded.
    fn start_import(&mut self, chosen: &[usize]) {
        let Some(library) = self.library.clone() else {
            return;
        };
        let Some(mut choosing) = self.opml.choosing.take() else {
            return;
        };
        if let Some(loading) = choosing.loading.take() {
            loading.abort();
        }
        let feeds: Vec<_> = chosen
            .iter()
            .filter_map(|&i| {
                let feed = choosing.feeds.get(i)?;
                Some(match choosing.previews.get_mut(i)?.take() {
                    Some(preview) => ToImport::Previewed(Box::new(preview)),
                    None => ToImport::Feed(feed.url.clone()),
                })
            })
            .collect();
        if let Some(dialog) = choosing.dialog.upgrade() {
            dialog.close();
        }
        if feeds.is_empty() {
            return;
        }
        let total = feeds.len();
        self.opml.running = Some((0, 0, total));
        self.content
            .send(ContentInput::ImportProgress(Some((0, total))));
        // Shows, where the new shows arrive under the bar.
        self.sender.input(AppMsg::Navigate(Location::Shows));
        let sender = self.sender.clone();
        self.sender.spawn(async move {
            library
                .import(feeds, |result| {
                    sender.command(AppCommand::Opml(OpmlCommand::Subscribed(result)));
                })
                .await;
            AppCommand::Opml(OpmlCommand::Finished)
        });
    }

    pub(super) fn feed_imported(&mut self, result: Result<ShowId, LibraryError>) {
        let Some((imported, done, total)) = self.opml.running.as_mut() else {
            return;
        };
        *done += 1;
        match result {
            // Subscribed to meanwhile, by sync or by hand: it is there.
            Ok(_) | Err(LibraryError::AlreadySubscribed(_)) => *imported += 1,
            Err(err) => tracing::warn!("could not import a feed: {err}"),
        }
        self.content
            .send(ContentInput::ImportProgress(Some((*done, *total))));
        // The Shows grid fills in as the import goes.
        self.reload(Parts {
            shows: true,
            counts: true,
            ..Parts::NONE
        });
    }

    pub(super) fn import_finished(&mut self) {
        let Some((imported, _, total)) = self.opml.running.take() else {
            return;
        };
        self.content.send(ContentInput::ImportProgress(None));
        toast(
            &self.widgets.toasts,
            &import::imported_message(imported, total),
        );
        self.reload(Parts::ALL);
        self.request_sync(false);
    }
}
