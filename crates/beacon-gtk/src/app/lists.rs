// SPDX-License-Identifier: GPL-3.0-or-later

//! The Clear buttons of Queue and Downloaded (asked by the user
//! 2026-11-08). Clearing the queue can be undone from its toast, as the
//! HIG's Toasts page has toasts offer; deleting downloads deletes files, so
//! it asks first, as Unsubscribe does.

use beacon_core::model::EpisodeId;
use gettextrs::{gettext, ngettext};
use relm4::adw::prelude::*;
use relm4::{ComponentSender, adw};

use super::library::Parts;
use super::{App, AppCommand, AppMsg, AppWidgets, toast};
use crate::ui::errors;

impl App {
    /// Empty the queue, with Undo putting `queued` back in its order.
    pub(super) fn clear_queue(
        &mut self,
        queued: Vec<EpisodeId>,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
    ) {
        if queued.is_empty() {
            return;
        }
        self.job(sender, Parts::EPISODE, false, |store| store.clear_queue());
        let toast = adw::Toast::builder()
            .title(gettext("Queue cleared"))
            .button_label(gettext("_Undo"))
            .build();
        let sender = sender.clone();
        toast.connect_button_clicked(move |_| {
            sender.input(AppMsg::Requeue(queued.clone()));
        });
        widgets.toasts.add_toast(toast);
    }

    /// Undo of Clear Queue: the episodes join the queue again, in order.
    pub(super) fn requeue(&mut self, ids: Vec<EpisodeId>, sender: &ComponentSender<Self>) {
        self.job(sender, Parts::EPISODE, false, move |store| {
            ids.iter().try_for_each(|id| store.enqueue(*id))
        });
    }

    /// Ask before deleting every download.
    pub(super) fn confirm_delete_downloads(
        &self,
        ids: Vec<EpisodeId>,
        sender: &ComponentSender<Self>,
        root: &adw::ApplicationWindow,
    ) {
        if ids.is_empty() {
            return;
        }
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Delete All Downloads?")),
            Some(
                &ngettext(
                    "The downloaded episode will be removed from this device",
                    "The {n} downloaded episodes will be removed from this device",
                    ids.len() as u32,
                )
                .replace("{n}", &ids.len().to_string()),
            ),
        );
        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("delete", &gettext("_Delete"));
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        // No default: the HIG keeps Return off destructive buttons, and
        // Escape cancels.
        dialog.set_close_response("cancel");
        let sender = sender.clone();
        dialog.connect_response(Some("delete"), move |_, _| {
            sender.input(AppMsg::DeleteDownloads(ids.clone()));
        });
        dialog.present(Some(root));
    }

    pub(super) fn delete_downloads(&self, ids: Vec<EpisodeId>, sender: &ComponentSender<Self>) {
        let Some(library) = self.library.clone() else {
            return;
        };
        sender.spawn_oneshot_command(move || {
            AppCommand::DownloadsDeleted(library.delete_downloads(&ids))
        });
    }

    pub(super) fn downloads_deleted(
        &mut self,
        result: Result<usize, beacon_core::StoreError>,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
    ) {
        match result {
            Ok(count) => toast(
                &widgets.toasts,
                &ngettext(
                    "Deleted {n} download",
                    "Deleted {n} downloads",
                    count as u32,
                )
                .replace("{n}", &count.to_string()),
            ),
            Err(err) => {
                tracing::warn!("could not delete downloads: {err}");
                toast(&widgets.toasts, &errors::store(&err));
            }
        }
        self.reload(sender, Parts::EPISODE);
    }
}
