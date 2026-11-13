// SPDX-License-Identifier: GPL-3.0-or-later

//! The data side of the window. Every read and write of the library runs on
//! a blocking worker, never on the UI thread; a job changes the store if
//! asked, then loads only the parts of the views it affects, and the window
//! applies the result when it arrives. Each part remembers its latest
//! request, so a slow result never overwrites a newer one.

use adw::prelude::*;
use beacon_core::StoreError;
use beacon_core::browse::RecencyGroup;
use beacon_core::model::{EpisodeId, LibraryCounts, ShowId, ShowPrefs};
use beacon_core::queue::Source;
use beacon_core::store::Store;
use chrono::{DateTime, Utc};
use gettextrs::gettext;

use super::{App, AppCommand, toast};
use crate::navigation::Location;
use crate::ui::content::ContentInput;
use crate::ui::details_pane::DetailsPaneInput;
use crate::ui::errors;
use crate::ui::models::{self, EpisodeItem, ShowCard, ShowPageData};
use crate::ui::page_filters;
use crate::ui::player_bar::PlayerBarInput;
use crate::ui::queue_data::{self, Lists, QueueData};
use crate::ui::search_data::{self, SearchResults};
use crate::ui::sidebar::{Counts, SidebarInput};

/// Where New's filter menu choices are remembered, by menu group.
pub(super) const NEW_KEYS: [(&str, &str); 3] = [
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
    /// The Queue view, and whether Next has anything to play.
    pub queue: bool,
    pub favorites: bool,
    pub downloaded: bool,
    /// The library search's results, when one shows.
    pub search: bool,
}

impl Parts {
    pub const ALL: Parts = Parts {
        shows: true,
        new: true,
        page: true,
        details: true,
        counts: true,
        queue: true,
        favorites: true,
        downloaded: true,
        search: true,
    };
    pub(super) const NONE: Parts = Parts {
        shows: false,
        new: false,
        page: false,
        details: false,
        counts: false,
        queue: false,
        favorites: false,
        downloaded: false,
        search: false,
    };
    /// Everything an episode's state shows up in.
    pub(super) const EPISODE: Parts = Parts::ALL;
    /// Where an episode got to: its progress bar in New and on its page,
    /// and the details. Not the Shows grid or the counts.
    pub(super) const PROGRESS: Parts = Parts {
        new: true,
        page: true,
        details: true,
        queue: true,
        favorites: true,
        downloaded: true,
        search: true,
        ..Parts::NONE
    };
    /// New's filters decide what follows an episode started from it.
    pub(super) const NEW: Parts = Parts {
        new: true,
        queue: true,
        ..Parts::NONE
    };
    pub(super) const PAGE: Parts = Parts {
        page: true,
        ..Parts::NONE
    };
    pub(super) const SEARCH: Parts = Parts {
        search: true,
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
    playing: Option<EpisodeId>,
    after: Option<EpisodeId>,
    source: Option<Source>,
    search: Option<(String, Location)>,
    show_search: Option<String>,
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
    queue: u64,
    favorites: u64,
    downloaded: u64,
    search: u64,
}

#[derive(Debug, Default)]
pub struct Loaded {
    generation: u64,
    cards: Option<Vec<ShowCard>>,
    new: Option<(Vec<(RecencyGroup, EpisodeItem)>, ShowPrefs)>,
    page: Option<Option<ShowPageData>>,
    details: Option<Option<EpisodeItem>>,
    counts: Option<LibraryCounts>,
    queue: Option<QueueData>,
    favorites: Option<Vec<EpisodeItem>>,
    downloaded: Option<Vec<EpisodeItem>>,
    search: Option<SearchResults>,
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
            Some(id) => models::show_page(store, id, context.show_search.as_deref())?,
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
    if parts.favorites {
        loaded.favorites = Some(models::favorite_items(store)?);
    }
    if parts.downloaded {
        loaded.downloaded = Some(models::downloaded_items(store)?);
    }
    if parts.search
        && let Some((query, section)) = &context.search
    {
        let episodes = section_episodes(store, *section, context)?;
        loaded.search = Some(search_data::search(store, query, *section, episodes)?);
    }
    if parts.queue {
        let lists = Lists {
            now: context.now,
            new_prefs: context.new_prefs,
        };
        loaded.queue = Some(queue_data::queue_data(
            store,
            context.playing,
            context.playing.or(context.after),
            context.source,
            lists,
        )?);
    }
    Ok(loaded)
}

/// A section's episodes, in the order its view shows them, for its search.
fn section_episodes(
    store: &Store,
    section: Location,
    context: &Context,
) -> Result<Vec<EpisodeItem>, StoreError> {
    Ok(match section {
        Location::Shows => Vec::new(),
        Location::New => models::new_items(store, context.now, context.new_prefs)?
            .into_iter()
            .map(|(_, item)| item)
            .collect(),
        Location::Favorites => models::favorite_items(store)?,
        Location::Downloaded => models::downloaded_items(store)?,
        Location::Queue => {
            let lists = Lists {
                now: context.now,
                new_prefs: context.new_prefs,
            };
            let queue = queue_data::queue_data(
                store,
                context.playing,
                context.playing.or(context.after),
                context.source,
                lists,
            )?;
            queue
                .now_playing
                .into_iter()
                .chain(queue.queued)
                .chain(queue.from_source)
                .collect()
        }
    })
}

impl App {
    fn context(&self) -> Option<Context> {
        Some(Context {
            now: self.library.as_ref()?.now(),
            new_prefs: self.new_prefs(),
            page: self.last_show,
            details: self.details_episode.map(|(episode, _)| episode),
            playing: self.now_playing.as_ref().map(|now| now.id),
            after: self.after,
            source: self.source,
            search: self.search.clone(),
            show_search: self.show_search.clone(),
        })
    }

    /// Where a list's order comes from when it is read again.
    pub(super) fn lists(&self) -> Option<Lists> {
        Some(Lists {
            now: self.library.as_ref()?.now(),
            new_prefs: self.new_prefs(),
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
            (parts.queue, &mut requests.queue),
            (parts.favorites, &mut requests.favorites),
            (parts.downloaded, &mut requests.downloaded),
            (parts.search, &mut requests.search),
        ] {
            if wanted {
                *slot = generation;
            }
        }
        self.sender.spawn_blocking(move || {
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
    pub(super) fn reload(&mut self, parts: Parts) {
        self.job(parts, false, |_| Ok(()));
    }

    pub(super) fn loaded(&mut self, result: Result<Loaded, StoreError>) {
        let loaded = match result {
            Ok(loaded) => loaded,
            Err(err) => {
                tracing::error!("could not read the library: {err}");
                toast(&self.widgets.toasts, &errors::store(&err));
                return;
            }
        };
        let generation = loaded.generation;
        let current = |latest: u64| latest == generation;

        if let Some(cards) = loaded.cards.filter(|_| current(self.requests.shows)) {
            self.has_shows = !cards.is_empty();
            self.export.set_enabled(self.has_shows);
            self.titles = cards.iter().map(|c| (c.id, c.title.clone())).collect();
            self.content.send(ContentInput::SetShows(cards));
        }
        if let Some((items, prefs)) = loaded.new.filter(|_| current(self.requests.new)) {
            self.content.send(ContentInput::SetNew {
                items,
                prefs,
                has_shows: self.has_shows,
            });
        }
        if let Some(Some(page)) = loaded.page.filter(|_| current(self.requests.page)) {
            if self.subscribed_toast == Some(page.show.id) {
                self.subscribed_toast = None;
                toast(
                    &self.widgets.toasts,
                    &gettext("Subscribed to {show}").replace("{show}", &page.show.info.title),
                );
            }
            if loaded.open_page {
                self.content.send(ContentInput::OpenShow(Box::new(page)));
                self.widgets.split_view.set_show_content(true);
            } else {
                self.content.send(ContentInput::UpdateShow(Box::new(page)));
            }
        }
        if let Some(details) = loaded.details.filter(|_| current(self.requests.details)) {
            match details {
                Some(item) => self.details.send(DetailsPaneInput::Show(Box::new(item))),
                None => self.details.send(DetailsPaneInput::Clear),
            }
        }
        if let Some(counts) = loaded.counts.filter(|_| current(self.requests.counts)) {
            self.sidebar.send(SidebarInput::SetCounts(Counts {
                shows: counts.shows_with_new,
                new: counts.new_episodes,
                queue: counts.queue,
            }));
        }
        if let Some(items) = loaded
            .favorites
            .filter(|_| current(self.requests.favorites))
        {
            self.content.send(ContentInput::SetFavorites(items));
        }
        if let Some(items) = loaded
            .downloaded
            .filter(|_| current(self.requests.downloaded))
        {
            self.content.send(ContentInput::SetDownloaded(items));
        }
        if let Some(results) = loaded.search.filter(|_| current(self.requests.search)) {
            // A search that ended meanwhile shows nothing.
            if self.search.as_ref() == Some(&(results.query.clone(), results.section)) {
                self.content
                    .send(ContentInput::SetSearch(Box::new(results)));
            }
        }
        if let Some(queue) = loaded.queue.filter(|_| current(self.requests.queue)) {
            self.bar.send(PlayerBarInput::HasNext(queue.next.is_some()));
            self.up_next = queue.next.as_ref().map(|item| item.episode.clone());
            self.preload_next();
            self.content.send(ContentInput::SetQueue(Box::new(queue)));
        }
    }
}
