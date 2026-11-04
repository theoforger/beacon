// SPDX-License-Identifier: GPL-3.0-or-later

//! The data side of the window. Every read and write of the library runs on
//! a blocking worker, never on the UI thread; a job changes the store if
//! asked, then loads only the parts of the views it affects, and the window
//! applies the result when it arrives. Each part remembers its latest
//! request, so a slow result never overwrites a newer one.

use beacon_core::LibraryError;
use beacon_core::StoreError;
use beacon_core::browse::RecencyGroup;
use beacon_core::model::{EpisodeId, LibraryCounts, ShowId, ShowPrefs};
use beacon_core::services::library::RefreshSummary;
use beacon_core::store::Store;
use chrono::{DateTime, Utc};
use gettextrs::{gettext, ngettext};
use relm4::adw::prelude::*;
use relm4::{ComponentController, ComponentSender, adw};
use url::Url;

use super::{App, AppCommand, AppMsg, AppWidgets, toast};
use crate::ui::content::{ContentInput, ContentOutput};
use crate::ui::details_pane::DetailsPaneInput;
use crate::ui::errors;
use crate::ui::models::{self, EpisodeItem, RowAction, ShowCard, ShowPageData};
use crate::ui::page_filters;
use crate::ui::player_bar::PlayerBarInput;
use crate::ui::sidebar::{Counts, SidebarInput};

/// Where New's filter menu choices are remembered, by menu group.
const NEW_KEYS: [(&str, &str); 3] = [
    (page_filters::SORT, "new-sort"),
    (page_filters::PLAYED, "new-filter"),
    (page_filters::DOWNLOADED, "new-downloaded"),
];

/// Which parts of the views a job reloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parts {
    pub shows: bool,
    pub new: bool,
    pub page: bool,
    pub details: bool,
    pub counts: bool,
}

impl Parts {
    pub const ALL: Parts = Parts {
        shows: true,
        new: true,
        page: true,
        details: true,
        counts: true,
    };
    pub(super) const NONE: Parts = Parts {
        shows: false,
        new: false,
        page: false,
        details: false,
        counts: false,
    };
    /// Everything an episode's state shows up in.
    pub(super) const EPISODE: Parts = Parts::ALL;
    const NEW: Parts = Parts {
        new: true,
        ..Parts::NONE
    };
    const PAGE: Parts = Parts {
        page: true,
        ..Parts::NONE
    };
}

/// What a job needs to know about the window, captured when it starts.
#[derive(Debug, Clone)]
struct Context {
    now: DateTime<Utc>,
    new_prefs: ShowPrefs,
    page: Option<ShowId>,
    details: Option<EpisodeId>,
}

/// Generation numbers: the latest request for each part.
#[derive(Debug, Default)]
pub struct Requests {
    next: u64,
    shows: u64,
    new: u64,
    page: u64,
    details: u64,
    counts: u64,
}

#[derive(Debug, Default)]
pub struct Loaded {
    generation: u64,
    cards: Option<Vec<ShowCard>>,
    new: Option<(Vec<(RecencyGroup, EpisodeItem)>, ShowPrefs)>,
    page: Option<Option<ShowPageData>>,
    details: Option<Option<EpisodeItem>>,
    counts: Option<LibraryCounts>,
    /// Push the loaded podcast page rather than only refresh it.
    open_page: bool,
}

fn load(store: &Store, parts: Parts, context: &Context) -> Result<Loaded, StoreError> {
    let mut loaded = Loaded::default();
    if parts.shows {
        loaded.cards = Some(models::show_cards(store)?);
    }
    if parts.new {
        let items = models::new_items(store, context.now, context.new_prefs)?;
        loaded.new = Some((items, context.new_prefs));
    }
    if parts.page {
        loaded.page = Some(match context.page {
            Some(id) => models::show_page(store, id)?,
            None => None,
        });
    }
    if parts.details {
        loaded.details = Some(match context.details {
            Some(id) => models::episode_item(store, id)?,
            None => None,
        });
    }
    if parts.counts {
        loaded.counts = Some(store.counts()?);
    }
    Ok(loaded)
}

impl App {
    fn context(&self) -> Option<Context> {
        Some(Context {
            now: self.library.as_ref()?.now(),
            new_prefs: self.new_prefs(),
            page: self.last_show,
            details: self.details_episode.map(|(episode, _)| episode),
        })
    }

    /// New's sort and filters, as remembered in settings.
    fn new_prefs(&self) -> ShowPrefs {
        NEW_KEYS
            .iter()
            .fold(ShowPrefs::default(), |prefs, (group, key)| {
                page_filters::apply(prefs, group, self.settings.string(key).as_str())
            })
    }

    /// Run `change` on the store in the background, then reload `parts`.
    pub(super) fn job(
        &mut self,
        sender: &ComponentSender<Self>,
        parts: Parts,
        open_page: bool,
        change: impl FnOnce(&mut Store) -> Result<(), StoreError> + Send + 'static,
    ) {
        let (Some(library), Some(context)) = (self.library.clone(), self.context()) else {
            return;
        };
        let requests = &mut self.requests;
        requests.next += 1;
        let generation = requests.next;
        for (wanted, slot) in [
            (parts.shows, &mut requests.shows),
            (parts.new, &mut requests.new),
            (parts.page, &mut requests.page),
            (parts.details, &mut requests.details),
            (parts.counts, &mut requests.counts),
        ] {
            if wanted {
                *slot = generation;
            }
        }
        sender.spawn_oneshot_command(move || {
            let result = library.with_store(|store| {
                change(store)?;
                load(store, parts, &context)
            });
            AppCommand::Loaded(Box::new(result.map(|mut loaded| {
                loaded.generation = generation;
                loaded.open_page = open_page;
                loaded
            })))
        });
    }

    /// Reload `parts` without changing anything.
    pub(super) fn reload(&mut self, sender: &ComponentSender<Self>, parts: Parts) {
        self.job(sender, parts, false, |_| Ok(()));
    }

    pub(super) fn loaded(&mut self, result: Result<Loaded, StoreError>, widgets: &AppWidgets) {
        let loaded = match result {
            Ok(loaded) => loaded,
            Err(err) => {
                tracing::error!("could not read the library: {err}");
                toast(&widgets.toasts, &errors::store(&err));
                return;
            }
        };
        let generation = loaded.generation;
        let current = |latest: u64| latest == generation;

        if let Some(cards) = loaded.cards.filter(|_| current(self.requests.shows)) {
            self.has_shows = !cards.is_empty();
            self.titles = cards.iter().map(|c| (c.id, c.title.clone())).collect();
            self.content.emit(ContentInput::SetShows(cards));
        }
        if let Some((items, prefs)) = loaded.new.filter(|_| current(self.requests.new)) {
            self.content.emit(ContentInput::SetNew {
                items,
                prefs,
                has_shows: self.has_shows,
            });
        }
        if let Some(Some(page)) = loaded.page.filter(|_| current(self.requests.page)) {
            if self.subscribed_toast == Some(page.show.id) {
                self.subscribed_toast = None;
                toast(
                    &widgets.toasts,
                    &gettext("Subscribed to {show}").replace("{show}", &page.show.info.title),
                );
            }
            if loaded.open_page {
                self.content.emit(ContentInput::OpenShow(Box::new(page)));
                widgets.split_view.set_show_content(true);
            } else {
                self.content.emit(ContentInput::UpdateShow(Box::new(page)));
            }
        }
        if let Some(details) = loaded.details.filter(|_| current(self.requests.details)) {
            match details {
                Some(item) => self.details.emit(DetailsPaneInput::Show(Box::new(item))),
                None => self.details.emit(DetailsPaneInput::Clear),
            }
        }
        if let Some(counts) = loaded.counts.filter(|_| current(self.requests.counts)) {
            self.sidebar.emit(SidebarInput::SetCounts(Counts {
                shows: counts.shows_with_new,
                new: counts.new_episodes,
                queue: counts.queue,
            }));
            self.set_queue_length(counts.queue);
        }
    }

    pub(super) fn handle_content(
        &mut self,
        output: ContentOutput,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
        root: &adw::ApplicationWindow,
    ) {
        match output {
            ContentOutput::Action(item, action) => self.row_action(*item, action, widgets, sender),
            ContentOutput::OpenShow(id) => self.open_show(id, sender),
            ContentOutput::GoBack => self.go_back(widgets),
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

    fn row_action(
        &mut self,
        item: EpisodeItem,
        action: RowAction,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
    ) {
        let Some(now) = self.library.as_ref().map(|l| l.now()) else {
            return;
        };
        let id = item.episode.id;
        let state = item.episode.state.clone();
        match action {
            RowAction::Play => self.play_episode(&item, widgets, sender),
            RowAction::ShowDetails => {
                // Show what the row already knows at once; viewing clears
                // the blue dot for good, which the reload then shows.
                self.details_episode = Some((id, item.episode.show_id));
                self.details.emit(DetailsPaneInput::Show(Box::new(item)));
                // The episode's own cover, as GNOME Podcasts shows it in its
                // details; rows keep the show's.
                self.fetch_episode_cover(id, sender);
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
                // The player bar's star follows when it is the playing one.
                if let Some(now) = &mut self.now_playing
                    && now.id == id
                {
                    now.favorite = !state.favorite;
                    self.bar.emit(PlayerBarInput::Favorite(!state.favorite));
                }
            }
            // Never touches the queue.
            RowAction::TogglePlayed => {
                self.job(sender, Parts::EPISODE, false, move |store| {
                    store.set_played(id, !state.played, now)
                });
            }
        }
    }

    fn open_show(&mut self, id: ShowId, sender: &ComponentSender<Self>) {
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

    pub(super) fn start_refresh(&self, sender: &ComponentSender<Self>) {
        let Some(library) = self.library.clone() else {
            return;
        };
        self.refresh.set_enabled(false);
        sender.oneshot_command(async move { AppCommand::Refreshed(library.refresh_all().await) });
    }

    pub(super) fn refreshed(
        &mut self,
        summary: &RefreshSummary,
        widgets: &AppWidgets,
        sender: &ComponentSender<Self>,
    ) {
        self.refresh.set_enabled(true);
        self.reload(sender, Parts::ALL);
        for (show, err) in &summary.failed {
            tracing::warn!("could not refresh show {show:?}: {err}");
        }
        toast(&widgets.toasts, &refresh_message(summary));
    }
}

/// What a finished refresh says: failures first, then what was found,
/// including that nothing was.
fn refresh_message(summary: &RefreshSummary) -> String {
    if !summary.failed.is_empty() {
        let n = summary.failed.len() as u32;
        return ngettext(
            "Could not refresh {n} podcast",
            "Could not refresh {n} podcasts",
            n,
        )
        .replace("{n}", &n.to_string());
    }
    match summary.added {
        0 => gettext("No new episodes"),
        n => {
            ngettext("{n} new episode", "{n} new episodes", n as u32).replace("{n}", &n.to_string())
        }
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
    fn a_refresh_always_says_what_it_found() {
        assert_eq!(refresh_message(&summary(0)), "No new episodes");
        assert_eq!(refresh_message(&summary(1)), "1 new episode");
        assert_eq!(refresh_message(&summary(3)), "3 new episodes");
    }
}
