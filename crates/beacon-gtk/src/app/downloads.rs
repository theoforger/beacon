// SPDX-License-Identifier: GPL-3.0-or-later

//! Downloads, only ever when asked (from a row's menu, the details pane,
//! selection mode or a podcast page's Download All Episodes), and the
//! deletion of old ones that Preferences can turn on
//! (`docs/specs/auto-delete.md`): at startup and once a day.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use adw::prelude::*;
use beacon_core::DownloadError;
use beacon_core::model::EpisodeId;
use beacon_core::services::cleanup::CleanupPolicy;
use beacon_core::services::downloads::Progress;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};

use super::library::Parts;
use super::{App, AppCommand, AppMsg, toast};
use crate::ui::models::EpisodeItem;
use crate::ui::{downloads, errors};

/// How often the rows' download bars move on.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);
/// How often old downloads are looked for while Beacon is open.
const CLEANUP_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// The downloads under way, shared with the timer that shows them.
pub type Active = Rc<RefCell<HashMap<EpisodeId, Arc<Progress>>>>;

/// What the rows show of `active`.
fn publish(active: &Active) {
    downloads::set(
        active
            .borrow()
            .iter()
            .map(|(id, progress)| (*id, progress.fraction()))
            .collect(),
    );
}

/// Preferences' Downloads group, as the cleanup reads it.
pub fn cleanup_policy(settings: &gio::Settings) -> CleanupPolicy {
    CleanupPolicy {
        enabled: settings.boolean("delete-old-downloads"),
        after_days: settings.uint("delete-after-days"),
        only_played: settings.boolean("delete-only-played"),
    }
}

impl App {
    /// A row's or the details pane's Download, Cancel Download or Delete
    /// Download, as the episode is.
    pub(super) fn toggle_download(&mut self, item: &EpisodeItem) {
        let id = item.episode.id;
        let active = self.downloads.borrow().get(&id).cloned();
        if let Some(progress) = active {
            progress.cancel();
        } else if item.episode.state.download_path.is_some() {
            self.delete_downloads(vec![id]);
        } else {
            self.start_downloads(vec![id]);
        }
    }

    /// Download `ids`, those not already downloading; the library takes
    /// them a few at a time, in this order.
    pub(super) fn start_downloads(&mut self, ids: Vec<EpisodeId>) {
        let Some(library) = self.library.clone() else {
            return;
        };
        let was_idle = self.downloads.borrow().is_empty();
        for id in ids {
            if self.downloads.borrow().contains_key(&id) {
                continue;
            }
            let progress = Arc::new(Progress::default());
            self.downloads.borrow_mut().insert(id, progress.clone());
            let library = library.clone();
            self.sender.spawn(async move {
                let result = library.download(id, &progress).await;
                // The title, for a toast saying what failed.
                let title = match &result {
                    Ok(_) => String::new(),
                    Err(_) => library.with_store(|store| {
                        store
                            .episode(id)
                            .ok()
                            .flatten()
                            .map(|e| e.info.title)
                            .unwrap_or_default()
                    }),
                };
                AppCommand::Downloaded(id, title, result)
            });
        }
        publish(&self.downloads);
        // The details pane's menu is built when it shows an episode: build
        // it again, so it offers Cancel Download.
        self.reload(Parts {
            details: true,
            ..Parts::NONE
        });
        if was_idle && !self.downloads.borrow().is_empty() {
            // Until the last finishes.
            let active = self.downloads.clone();
            glib::timeout_add_local(PROGRESS_EVERY, move || {
                publish(&active);
                if active.borrow().is_empty() {
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            });
        }
    }

    pub(super) fn downloaded(
        &mut self,
        id: EpisodeId,
        title: &str,
        result: Result<PathBuf, DownloadError>,
    ) {
        self.downloads.borrow_mut().remove(&id);
        publish(&self.downloads);
        match result {
            Ok(path) => tracing::debug!(?path, "downloaded episode {}", id.0),
            Err(err) => {
                tracing::warn!("could not download episode {}: {err}", id.0);
                if let Some(message) = errors::download(&err, title) {
                    toast(&self.widgets.toasts, &message);
                }
            }
        }
        self.reload(Parts::EPISODE);
    }

    /// Download All Episodes, asked first: it can be many files.
    pub(super) fn confirm_download_all(&self, ids: Vec<EpisodeId>, root: &adw::ApplicationWindow) {
        if ids.is_empty() {
            return;
        }
        let count = ids.len();
        let dialog = adw::AlertDialog::new(
            Some(
                &ngettext(
                    "Download {n} Episode?",
                    "Download {n} Episodes?",
                    count as u32,
                )
                .replace("{n}", &count.to_string()),
            ),
            Some(&gettext(
                "Downloaded episodes take up space on this device until they are deleted",
            )),
        );
        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("download", &gettext("_Download"));
        dialog.set_response_appearance("download", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("download"));
        dialog.set_close_response("cancel");
        let input = self.sender.clone();
        dialog.connect_response(Some("download"), move |_, _| {
            input.input(AppMsg::Download(ids.clone()));
        });
        dialog.present(Some(root));
    }

    /// Preferences. Closing it looks for old downloads at once, so a
    /// change shows without waiting a day.
    pub(super) fn open_preferences(&self, root: &adw::ApplicationWindow) {
        let dialog = crate::ui::dialogs::preferences::dialog(&self.settings);
        let sender = self.sender.clone();
        dialog.connect_closed(move |_| sender.input(AppMsg::CleanDownloads));
        dialog.present(Some(root));
    }

    /// Look for old downloads now, and then once a day.
    pub(super) fn schedule_cleanup(&self) {
        self.sender.input(AppMsg::CleanDownloads);
        let sender = self.sender.clone();
        glib::timeout_add_local(CLEANUP_EVERY, move || {
            sender.input(AppMsg::CleanDownloads);
            glib::ControlFlow::Continue
        });
    }

    /// Delete the downloads Preferences finds old, never the queue's (the
    /// library leaves those) nor the one playing.
    pub(super) fn clean_downloads(&self) {
        let policy = cleanup_policy(&self.settings);
        let Some(library) = self.library.clone().filter(|_| policy.enabled) else {
            return;
        };
        let keep: Vec<EpisodeId> = self.now_playing.iter().map(|now| now.id).collect();
        self.sender
            .spawn_blocking(move || AppCommand::Cleaned(library.clean_downloads(policy, &keep)));
    }

    pub(super) fn cleaned(&mut self, result: Result<usize, beacon_core::StoreError>) {
        match result {
            Ok(0) => {}
            Ok(count) => {
                toast(
                    &self.widgets.toasts,
                    &ngettext(
                        "Deleted {n} old download",
                        "Deleted {n} old downloads",
                        count as u32,
                    )
                    .replace("{n}", &count.to_string()),
                );
                self.reload(Parts::EPISODE);
            }
            Err(err) => {
                tracing::warn!("could not delete old downloads: {err}");
                toast(&self.widgets.toasts, &errors::store(&err));
            }
        }
    }
}
