// SPDX-License-Identifier: GPL-3.0-or-later

//! What the views ask for: row actions, opening and leaving shows,
//! subscribing and unsubscribing, and refreshing, each done as a background
//! job on the store (`library.rs`).

use beacon_core::LibraryError;
use beacon_core::model::{EpisodeChange, EpisodeId, ShowId};
use beacon_core::queue::Source;
use beacon_core::services::library::RefreshSummary;
use gettextrs::{gettext, ngettext};
use relm4::adw::prelude::*;
use relm4::{ComponentController, ComponentSender, adw};
use url::Url;

use super::{App, AppCommand, AppMsg, AppWidgets, toast};
use crate::ui::content::{ContentInput, ContentOutput};
use crate::ui::details_pane::DetailsPaneInput;
use crate::ui::errors;
use crate::ui::models::{EpisodeItem, RowAction};
use crate::ui::page_filters;

/// Where New's filter menu choices are remembered, by menu group.
use super::library::{NEW_KEYS, Parts};

impl App {
    pub(super) fn handle_content(
        &mut self,
        output: ContentOutput,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
        root: &adw::ApplicationWindow,
    ) {
        match output {
            ContentOutput::Action(item, action, source) => {
                self.row_action(*item, action, source, widgets, sender);
            }
            ContentOutput::OpenShow(id) => self.open_show(id, sender),
            ContentOutput::GoBack => self.go_back(widgets),
            ContentOutput::Change(ids, change) => self.change_episodes(ids, change, sender),
            ContentOutput::NewPrefsChanged(prefs) => {
                for (group, value) in page_filters::states(&prefs) {
                    let Some((_, key)) = NEW_KEYS.iter().find(|(g, _)| *g == group) else {
                        continue;
                    };
                    if let Err(err) = self.settings.set_string(key, value) {
                        tracing::warn!("could not remember the New filters: {err}");
                    }
                }
                self.reload(sender, Parts::NEW);
            }
            ContentOutput::ShowPrefsChanged(id, prefs) => {
                self.job(sender, Parts::PAGE, false, move |store| {
                    store.set_show_prefs(id, prefs)
                });
            }
            ContentOutput::MarkAll(id, played) => {
                let now = self.library.as_ref().map(|l| l.now());
                if let Some(now) = now {
                    self.job(sender, Parts::EPISODE, false, move |store| {
                        if played {
                            store.mark_all_played(id, now).map(|_| ())
                        } else {
                            store.mark_all_unplayed(id).map(|_| ())
                        }
                    });
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

    /// Selection mode's actions: one change to every selected episode.
    fn change_episodes(
        &mut self,
        ids: Vec<EpisodeId>,
        change: EpisodeChange,
        sender: &ComponentSender<Self>,
    ) {
        let Some(now) = self.library.as_ref().map(|l| l.now()) else {
            return;
        };
        self.job(sender, Parts::EPISODE, false, move |store| {
            store.change_episodes(&ids, change, now)
        });
    }

    /// `source` is the list the row is in; `None` (the Queue's own rows)
    /// keeps what plays after the playing episode as it was.
    pub(super) fn row_action(
        &mut self,
        item: EpisodeItem,
        action: RowAction,
        source: Option<Source>,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
    ) {
        let Some(now) = self.library.as_ref().map(|l| l.now()) else {
            return;
        };
        let id = item.episode.id;
        let state = item.episode.state.clone();
        match action {
            RowAction::Play => {
                self.take_source(id, source);
                self.play_episode(&item, widgets, sender);
            }
            RowAction::ShowDetails => {
                if source.is_some() || self.details_episode.map(|(e, _)| e) != Some(id) {
                    self.details_source = source;
                }
                // Show what the row already knows at once; viewing clears
                // the blue dot for good, which the reload then shows.
                self.details_episode = Some((id, item.episode.show_id));
                self.details.emit(DetailsPaneInput::Show(Box::new(item)));
                // The episode's own cover, as GNOME Podcasts shows it in its
                // details (rows keep the show's), and its chapters.
                self.load_details(id, sender);
                self.focus_details(widgets);
                widgets.details_split.set_show_sidebar(true);
                self.job(sender, Parts::EPISODE, false, move |store| {
                    store.mark_seen(id)
                });
            }
            RowAction::ToggleQueue => {
                let queued = item.in_queue;
                self.job(sender, Parts::EPISODE, false, move |store| {
                    if queued {
                        store.dequeue(id)
                    } else {
                        store.enqueue(id)
                    }
                });
            }
            RowAction::ToggleFavorite => {
                self.job(sender, Parts::EPISODE, false, move |store| {
                    store.set_favorite(id, !state.favorite)
                });
            }
            // Never touches the queue.
            RowAction::TogglePlayed => {
                self.job(sender, Parts::EPISODE, false, move |store| {
                    store.set_played(id, !state.played, now)
                });
            }
            // The playing episode goes back to its start, which is then
            // saved as it plays; any other has its saved position cleared.
            RowAction::ResetProgress => {
                if self.now_playing.as_ref().is_some_and(|now| now.id == id) {
                    self.seek(std::time::Duration::ZERO);
                }
                self.job(sender, Parts::EPISODE, false, move |store| {
                    store.set_position(id, std::time::Duration::ZERO)
                });
            }
        }
    }

    /// An episode about to start from a list: what follows it comes from
    /// that list. Pausing or resuming the playing one changes nothing.
    pub(super) fn take_source(&mut self, id: EpisodeId, source: Option<Source>) {
        let playing = self.now_playing.as_ref().map(|now| now.id);
        if playing != Some(id) && source.is_some() {
            self.source = source;
        }
    }

    pub(super) fn open_show(&mut self, id: ShowId, sender: &ComponentSender<Self>) {
        self.last_show = Some(id);
        self.job(sender, Parts::PAGE, true, |_| Ok(()));
    }

    fn confirm_unsubscribe(
        &self,
        id: ShowId,
        sender: &ComponentSender<Self>,
        root: &adw::ApplicationWindow,
    ) {
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Unsubscribe From {show}?").replace("{show}", &self.show_title(id))),
            Some(&gettext(
                "Its episodes, your progress in them and your favorites from it are removed from your library",
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

    /// A show's title as last loaded, without touching the store.
    fn show_title(&self, id: ShowId) -> String {
        self.titles.get(&id).cloned().unwrap_or_default()
    }

    pub(super) fn unsubscribe(
        &mut self,
        id: ShowId,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
    ) {
        if self.details_episode.is_some_and(|(_, show)| show == id) {
            self.details_episode = None;
        }
        if self.last_show == Some(id) {
            self.last_show = None;
        }
        self.content.emit(ContentInput::CloseShow);
        let title = self.show_title(id);
        self.job(sender, Parts::ALL, false, move |store| {
            store.delete_show(id)
        });
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
        sender: &ComponentSender<Self>,
    ) {
        self.content.emit(ContentInput::SubscribeDone);
        match result {
            Ok(id) => {
                self.last_show = Some(id);
                // The toast names the show once its page has loaded.
                self.subscribed_toast = Some(id);
                self.job(sender, Parts::ALL, true, |_| Ok(()));
            }
            Err(LibraryError::AlreadySubscribed(id)) => {
                self.open_show(id, sender);
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

    /// Refresh every feed; `at_startup` for the one when the app opens.
    pub(super) fn start_refresh(&self, sender: &ComponentSender<Self>, at_startup: bool) {
        let Some(library) = self.library.clone() else {
            return;
        };
        self.refresh.set_enabled(false);
        self.content.emit(ContentInput::Refreshing(true));
        sender.oneshot_command(async move {
            AppCommand::Refreshed(library.refresh_all().await, at_startup)
        });
    }

    pub(super) fn refreshed(
        &mut self,
        summary: &RefreshSummary,
        at_startup: bool,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
    ) {
        self.refresh.set_enabled(true);
        self.content.emit(ContentInput::Refreshing(false));
        self.reload(sender, Parts::ALL);
        for (show, err) in &summary.failed {
            tracing::warn!("could not refresh show {show:?}: {err}");
        }
        if let Some(message) = refresh_message(summary, at_startup) {
            toast(&widgets.toasts, &message);
        }
    }
}

/// What a finished refresh says: failures first, then what was found. One
/// the listener asked for also says that nothing was found; the one at
/// startup then says nothing, rather than at every launch.
fn refresh_message(summary: &RefreshSummary, at_startup: bool) -> Option<String> {
    if !summary.failed.is_empty() {
        let n = summary.failed.len() as u32;
        return Some(
            ngettext(
                "Could not refresh {n} podcast",
                "Could not refresh {n} podcasts",
                n,
            )
            .replace("{n}", &n.to_string()),
        );
    }
    match summary.added {
        0 if at_startup => None,
        0 => Some(gettext("No new episodes")),
        n => Some(
            ngettext("{n} new episode", "{n} new episodes", n as u32)
                .replace("{n}", &n.to_string()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(added: usize) -> RefreshSummary {
        RefreshSummary {
            added,
            failed: Vec::new(),
        }
    }

    #[test]
    fn a_refresh_says_what_it_found() {
        assert_eq!(
            refresh_message(&summary(0), false).unwrap(),
            "No new episodes"
        );
        assert_eq!(
            refresh_message(&summary(1), false).unwrap(),
            "1 new episode"
        );
        assert_eq!(
            refresh_message(&summary(3), false).unwrap(),
            "3 new episodes"
        );
    }

    #[test]
    fn the_refresh_at_startup_is_quiet_unless_something_happened() {
        assert_eq!(refresh_message(&summary(0), true), None);
        assert_eq!(
            refresh_message(&summary(2), true).unwrap(),
            "2 new episodes"
        );
        let failed = RefreshSummary {
            added: 0,
            failed: vec![(
                beacon_core::model::ShowId(1),
                LibraryError::ShowNotFound(beacon_core::model::ShowId(1)),
            )],
        };
        assert_eq!(
            refresh_message(&failed, true).unwrap(),
            "Could not refresh 1 podcast"
        );
    }
}
