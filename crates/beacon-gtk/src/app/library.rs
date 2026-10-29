// SPDX-License-Identifier: GPL-3.0-or-later

//! The data side of the window: reload the views from the library, apply
//! what they ask for, subscribe and refresh in the background, and report
//! the outcome in toasts.

use beacon_core::LibraryError;
use beacon_core::StoreError;
use beacon_core::model::{EpisodeId, ShowId};
use beacon_core::services::library::RefreshSummary;
use beacon_core::store::Store;
use gettextrs::{gettext, ngettext};
use relm4::adw::prelude::*;
use relm4::{ComponentController, ComponentSender, adw};
use url::Url;

use super::{App, AppCommand, AppMsg, AppWidgets, toast};
use crate::ui::content::{ContentInput, ContentOutput};
use crate::ui::details_pane::DetailsPaneInput;
use crate::ui::errors;
use crate::ui::models::{self, RowAction};
use crate::ui::new_view::filter_from;
use crate::ui::sidebar::{Counts, SidebarInput};

const NEW_FILTER_KEY: &str = "new-filter";

impl App {
    /// Rebuild every view from the library: the Shows grid, New, the open
    /// podcast page, the details pane and the sidebar counts.
    pub(super) fn reload(&self, widgets: &AppWidgets) {
        let Some(library) = &self.library else {
            return;
        };
        let now = library.now();
        let filter = filter_from(Some(self.settings.string(NEW_FILTER_KEY).as_str()));
        let loaded = library.with_store(|store| -> Result<_, StoreError> {
            let cards = models::show_cards(store)?;
            let new = models::new_items(store, now, filter)?;
            let counts = store.counts()?;
            let page = match self.last_show {
                Some(id) => models::show_page(store, id)?,
                None => None,
            };
            let details = match self.details_episode {
                Some(id) => models::episode_item(store, id)?,
                None => None,
            };
            Ok((cards, new, counts, page, details))
        });
        let (cards, new, counts, page, details) = match loaded {
            Ok(loaded) => loaded,
            Err(err) => {
                tracing::error!("could not read the library: {err}");
                toast(&widgets.toasts, &errors::store(&err));
                return;
            }
        };

        let has_shows = !cards.is_empty();
        self.content.emit(ContentInput::SetShows(cards));
        self.content.emit(ContentInput::SetNew {
            items: new,
            filter,
            has_shows,
        });
        if let Some(page) = page {
            self.content.emit(ContentInput::UpdateShow(Box::new(page)));
        }
        match details {
            Some(item) => self.details.emit(DetailsPaneInput::Show(Box::new(item))),
            None => self.details.emit(DetailsPaneInput::Clear),
        }
        self.sidebar.emit(SidebarInput::SetCounts(Counts {
            shows: counts.shows_with_new,
            new: counts.new_episodes,
            queue: counts.queue,
        }));
    }

    pub(super) fn handle_content(
        &mut self,
        output: ContentOutput,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
        root: &adw::ApplicationWindow,
    ) {
        match output {
            ContentOutput::Action(id, action) => self.row_action(id, action, widgets),
            ContentOutput::OpenShow(id) => self.open_show(id, widgets),
            ContentOutput::NewFilterChanged(filter) => {
                let name = match filter {
                    beacon_core::model::EpisodeFilter::All => "all",
                    beacon_core::model::EpisodeFilter::Unplayed => "unplayed",
                };
                if let Err(err) = self.settings.set_string(NEW_FILTER_KEY, name) {
                    tracing::warn!("could not remember the New filter: {err}");
                }
                self.reload(widgets);
            }
            ContentOutput::ShowPrefsChanged(id, prefs) => {
                self.change(widgets, |store| store.set_show_prefs(id, prefs));
            }
            ContentOutput::MarkAllPlayed(id) => {
                let now = self.library.as_ref().map(|l| l.now());
                if let Some(now) = now {
                    self.change(widgets, |store| store.mark_all_played(id, now).map(|_| ()));
                }
            }
            ContentOutput::Unsubscribe(id) => self.confirm_unsubscribe(id, sender, root),
            ContentOutput::FeedAddressCopied => {
                toast(&widgets.toasts, &gettext("Feed address copied"));
            }
            ContentOutput::Subscribe(url) => {
                let Some(library) = self.library.clone() else {
                    return;
                };
                sender.oneshot_command(async move {
                    let result = library.subscribe(&url).await;
                    AppCommand::Subscribed(url, result)
                });
            }
        }
    }

    /// Apply a change to the store, then show the result everywhere.
    fn change(&self, widgets: &AppWidgets, f: impl FnOnce(&mut Store) -> Result<(), StoreError>) {
        let Some(library) = &self.library else {
            return;
        };
        if let Err(err) = library.with_store(f) {
            tracing::error!("could not change the library: {err}");
            toast(&widgets.toasts, &errors::store(&err));
        }
        self.reload(widgets);
    }

    fn row_action(&mut self, id: EpisodeId, action: RowAction, widgets: &AppWidgets) {
        let Some(library) = &self.library else {
            return;
        };
        let now = library.now();
        if action == RowAction::ShowDetails {
            // Viewing the details clears the blue dot for good.
            self.details_episode = Some(id);
            widgets.details_split.set_show_sidebar(true);
            self.change(widgets, |store| store.mark_seen(id));
            return;
        }
        self.change(widgets, |store| {
            let Some(episode) = store.episode(id)? else {
                return Ok(());
            };
            match action {
                RowAction::ToggleQueue => {
                    if store.queue()?.contains(&id) {
                        store.dequeue(id)
                    } else {
                        store.enqueue(id)
                    }
                }
                RowAction::ToggleFavorite => store.set_favorite(id, !episode.state.favorite),
                // Never touches the queue.
                RowAction::TogglePlayed => store.set_played(id, !episode.state.played, now),
                RowAction::ShowDetails => Ok(()),
            }
        });
    }

    fn open_show(&mut self, id: ShowId, widgets: &AppWidgets) {
        let Some(library) = &self.library else {
            return;
        };
        match library.with_store(|store| models::show_page(store, id)) {
            Ok(Some(page)) => {
                self.last_show = Some(id);
                self.content.emit(ContentInput::OpenShow(Box::new(page)));
                widgets.split_view.set_show_content(true);
            }
            Ok(None) => {}
            Err(err) => toast(&widgets.toasts, &errors::store(&err)),
        }
    }

    fn confirm_unsubscribe(
        &self,
        id: ShowId,
        sender: &ComponentSender<Self>,
        root: &adw::ApplicationWindow,
    ) {
        let title = self
            .library
            .as_ref()
            .and_then(|l| l.with_store(|store| store.show(id)).ok().flatten())
            .map(|show| show.info.title)
            .unwrap_or_default();
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Unsubscribe From {show}?").replace("{show}", &title)),
            Some(&gettext(
                "Its episodes, your progress in them and your favorites from it are removed from your library.",
            )),
        );
        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("unsubscribe", &gettext("_Unsubscribe"));
        dialog.set_response_appearance("unsubscribe", adw::ResponseAppearance::Destructive);
        // No default: the HIG keeps Return off destructive buttons, and
        // Escape cancels.
        dialog.set_close_response("cancel");
        let sender = sender.clone();
        dialog.connect_response(Some("unsubscribe"), move |_, _| {
            sender.input(AppMsg::ConfirmUnsubscribe(id));
        });
        dialog.present(Some(root));
    }

    pub(super) fn unsubscribe(&mut self, id: ShowId, widgets: &AppWidgets) {
        let Some(library) = &self.library else {
            return;
        };
        let title = library
            .with_store(|store| store.show(id))
            .ok()
            .flatten()
            .map(|show| show.info.title)
            .unwrap_or_default();
        let shown = self
            .details_episode
            .and_then(|e| library.with_store(|store| store.episode(e)).ok().flatten());
        if shown.is_some_and(|e| e.show_id == id) {
            self.details_episode = None;
        }
        if self.last_show == Some(id) {
            self.last_show = None;
        }
        self.content.emit(ContentInput::CloseShow);
        self.change(widgets, |store| store.delete_show(id));
        toast(
            &widgets.toasts,
            &gettext("Unsubscribed from {show}").replace("{show}", &title),
        );
    }

    pub(super) fn subscribed(
        &mut self,
        url: &Url,
        result: Result<ShowId, LibraryError>,
        widgets: &AppWidgets,
    ) {
        self.content.emit(ContentInput::SubscribeDone);
        match result {
            Ok(id) => {
                self.reload(widgets);
                self.open_show(id, widgets);
                let title = self
                    .library
                    .as_ref()
                    .and_then(|l| l.with_store(|store| store.show(id)).ok().flatten())
                    .map(|show| show.info.title)
                    .unwrap_or_default();
                toast(
                    &widgets.toasts,
                    &gettext("Subscribed to {show}").replace("{show}", &title),
                );
            }
            Err(LibraryError::AlreadySubscribed(id)) => {
                self.open_show(id, widgets);
                toast(
                    &widgets.toasts,
                    &errors::library(&LibraryError::AlreadySubscribed(id)),
                );
            }
            Err(err) => {
                tracing::warn!("could not subscribe to {url}: {err}");
                toast(&widgets.toasts, &errors::library(&err));
            }
        }
    }

    pub(super) fn start_refresh(&self, sender: &ComponentSender<Self>) {
        let Some(library) = self.library.clone() else {
            return;
        };
        self.refresh.set_enabled(false);
        sender.oneshot_command(async move { AppCommand::Refreshed(library.refresh_all().await) });
    }

    pub(super) fn refreshed(&self, summary: &RefreshSummary, widgets: &AppWidgets) {
        self.refresh.set_enabled(true);
        self.reload(widgets);
        for (show, err) in &summary.failed {
            tracing::warn!("could not refresh show {show:?}: {err}");
        }
        let message = if !summary.failed.is_empty() {
            let n = summary.failed.len() as u32;
            Some(
                ngettext(
                    "Could not refresh {n} podcast",
                    "Could not refresh {n} podcasts",
                    n,
                )
                .replace("{n}", &n.to_string()),
            )
        } else if summary.added > 0 {
            let n = summary.added as u32;
            Some(ngettext("{n} new episode", "{n} new episodes", n).replace("{n}", &n.to_string()))
        } else {
            None
        };
        if let Some(message) = message {
            toast(&widgets.toasts, &message);
        }
    }
}
