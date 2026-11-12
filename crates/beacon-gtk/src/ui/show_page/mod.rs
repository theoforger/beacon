// SPDX-License-Identifier: GPL-3.0-or-later

//! A podcast page, pushed over the current section: the hero, then the
//! episodes, all in one list view (`ui/episode_list.rs`) so that long feeds
//! scroll smoothly. When the feed has season tags, each season has its own
//! card under a `GtkExpander` heading, so it can be folded; the page
//! remembers which seasons are folded.
//!
//! The same page previews a show from Apple's directory before subscribing
//! (decided by the user 2026-11-12, replacing a page of its own): the feed
//! as it would look subscribed, with Subscribe where Unsubscribe goes, and
//! nothing that acts on the library, as its episodes are not in it.

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use beacon_core::model::{EpisodeId, ShowId, ShowPrefs};
use gtk::{gdk, gio, glib};
use url::Url;

use crate::ui::episode_list::{Entry, EpisodeList, Folding};
use crate::ui::format;
use crate::ui::messages::{Inbox, Output};
use crate::ui::models::{EpisodeItem, RowAction, ShowPageData};
use crate::ui::page_filters;
use crate::ui::selection_bar::Bulk;
use crate::ui::show_hero::ShowHero;
use crate::ui::show_search::ShowSearch;
use crate::ui::widgets::episode_row::RowSetup;

mod build;
mod menu;
mod preview;

pub use preview::PreviewStatus;

pub const SHOW_TAG: &str = "show";
pub const PREVIEW_TAG: &str = "preview";

/// A subscribed show's page, or a preview of one not in the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
    Library,
    Preview,
}

#[derive(Debug)]
pub enum ShowPageInput {
    Set(Box<ShowPageData>),
    /// Preview a show, from what the directory said, with its page on
    /// Apple Podcasts if known, while its feed loads.
    Preview(Box<ShowPageData>, Option<Url>),
    /// The previewed feed, or why it could not be loaded.
    PreviewLoaded(Result<Box<ShowPageData>, String>),
    /// A choice in the filter menu: the group's action name and the value.
    Choose(page_filters::Choice),
    /// Mark All as Played, or as Unplayed when every episode is played.
    MarkAll,
    DownloadAll,
    CopyFeedAddress,
    ViewInStore,
    Unsubscribe,
    Subscribe,
    /// The subscription finished, either way.
    SubscribeDone,
    /// A season was folded or unfolded.
    Refold,
}

#[derive(Debug)]
pub enum ShowPageOutput {
    Action(Box<EpisodeItem>, RowAction),
    PrefsChanged(ShowId, ShowPrefs),
    /// Mark every episode of the show played (`true`) or unplayed.
    MarkAll(ShowId, bool),
    Unsubscribe(ShowId),
    /// A preview's Subscribe, with the feed's address.
    Subscribe(Url),
    FeedAddressCopied,
    /// Download All Episodes: the episodes not downloaded yet.
    DownloadAll(Vec<EpisodeId>),
    /// A change to the selected episodes.
    Change(Vec<EpisodeId>, Bulk),
    /// What the search bar narrows the episodes to, or `None` when the
    /// search ended.
    Search(ShowId, Option<String>),
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct ShowPage {
        pub parts: OnceCell<super::Parts>,
        pub data: RefCell<Option<ShowPageData>>,
        /// A preview's store link and load state.
        pub preview: RefCell<Option<(Option<Url>, PreviewStatus)>>,
        pub subscribing: Cell<bool>,
        pub inbox: Inbox<ShowPageInput>,
        pub output: Output<ShowPageOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ShowPage {
        const NAME: &'static str = "BeaconShowPage";
        type Type = super::ShowPage;
        type ParentType = adw::NavigationPage;
    }

    impl ObjectImpl for ShowPage {}
    impl WidgetImpl for ShowPage {}
    impl NavigationPageImpl for ShowPage {}
}

glib::wrapper! {
    pub struct ShowPage(ObjectSubclass<imp::ShowPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

pub struct Parts {
    kind: PageKind,
    hero: ShowHero,
    /// The filter menu's radio groups, by action name; not on a preview.
    filter: Option<page_filters::FilterButton>,
    /// More Options: Mark All, Download All Episodes, Copy Feed Address;
    /// on a preview, View on Apple Podcasts and Copy Feed Address.
    more: gtk::MenuButton,
    /// Download All Episodes, unavailable when all are downloaded.
    download_all: gio::SimpleAction,
    view_in_store: gio::SimpleAction,
    rows: RowSetup,
    list: EpisodeList,
    /// Folded seasons, kept while the app runs.
    folding: Folding,
    /// Not on a preview: its episodes are not in the library to search.
    search: Option<ShowSearch>,
    /// Shows the header's title once the hero's has scrolled away.
    title_check: Rc<dyn Fn()>,
}

impl ShowPage {
    pub fn connect_output(&self, receive: impl Fn(ShowPageOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    pub fn send(&self, input: ShowPageInput) {
        self.imp()
            .inbox
            .deliver(input, |message| self.update(message));
    }

    fn parts(&self) -> &Parts {
        self.imp().parts.get().expect("set when made")
    }

    /// Ctrl+F: open or close the search bar.
    pub fn toggle_search(&self) {
        if let Some(search) = &self.parts().search {
            search.toggle();
        }
    }

    /// The page's search, for tests.
    #[cfg(test)]
    pub fn search(&self) -> &ShowSearch {
        self.parts().search.as_ref().expect("a library page")
    }

    /// The feed shown.
    pub fn feed(&self) -> Option<Url> {
        let data = self.imp().data.borrow();
        data.as_ref().map(|d| d.show.feed_url.clone())
    }

    fn update(&self, message: ShowPageInput) {
        let imp = self.imp();
        let parts = self.parts();
        match message {
            ShowPageInput::Set(data) => self.set(*data),
            ShowPageInput::Preview(data, store) => {
                imp.preview.replace(Some((store, PreviewStatus::Loading)));
                imp.subscribing.set(false);
                self.set(*data);
            }
            ShowPageInput::PreviewLoaded(result) => {
                let mut preview = imp.preview.borrow_mut();
                let Some((_, status)) = preview.as_mut() else {
                    return;
                };
                match result {
                    Ok(data) => {
                        *status = PreviewStatus::Loaded;
                        drop(preview);
                        self.set(*data);
                    }
                    Err(why) => {
                        *status = PreviewStatus::Failed(why);
                        drop(preview);
                        self.show_episodes();
                    }
                }
            }
            ShowPageInput::Choose(choice) => {
                self.change_prefs(|p| *p = choice.apply(*p));
            }
            ShowPageInput::MarkAll => {
                let all = imp
                    .data
                    .borrow()
                    .as_ref()
                    .map(|d| (d.show.id, d.all_played));
                if let Some((show, all_played)) = all {
                    imp.output.emit(ShowPageOutput::MarkAll(show, !all_played));
                }
            }
            ShowPageInput::DownloadAll => {
                let ids = imp.data.borrow().as_ref().map(|d| d.to_download.clone());
                if let Some(ids) = ids.filter(|ids| !ids.is_empty()) {
                    imp.output.emit(ShowPageOutput::DownloadAll(ids));
                }
            }
            ShowPageInput::CopyFeedAddress => {
                if let (Some(url), Some(display)) = (self.feed(), gdk::Display::default()) {
                    display.clipboard().set_text(url.as_str());
                    imp.output.emit(ShowPageOutput::FeedAddressCopied);
                }
            }
            ShowPageInput::ViewInStore => self.view_in_store(),
            ShowPageInput::Unsubscribe => {
                let show = imp.data.borrow().as_ref().map(|d| d.show.id);
                if let Some(show) = show.filter(|_| parts.kind == PageKind::Library) {
                    imp.output.emit(ShowPageOutput::Unsubscribe(show));
                }
            }
            ShowPageInput::Subscribe => self.subscribe(),
            ShowPageInput::SubscribeDone => {
                imp.subscribing.set(false);
                self.show_subscribe();
            }
            ShowPageInput::Refold => self.show_episodes(),
        }
    }

    /// Show `data`: a new show starts out of selection mode and search,
    /// and at the top of its list.
    fn set(&self, data: ShowPageData) {
        let imp = self.imp();
        let parts = self.parts();
        let shown = imp
            .data
            .borrow()
            .as_ref()
            .map(|d| (d.show.feed_url.clone(), d.all_played));
        if shown.as_ref().map(|(url, _)| url) != Some(&data.show.feed_url) {
            parts.rows.selection.set_active(false);
            if let Some(search) = &parts.search {
                search.close();
            }
            parts.list.set(Vec::new());
        }
        self.set_title(&data.show.info.title);
        crate::ui::show_title::check_later(&parts.title_check);
        parts.hero.update(&data);
        match parts.kind {
            PageKind::Library => {
                if shown.map(|(_, all)| all) != Some(data.all_played) {
                    parts
                        .more
                        .set_menu_model(Some(&menu::more_menu(data.all_played)));
                }
                if let Some(filter) = &parts.filter {
                    filter.set_prefs(&data.prefs);
                }
                parts.download_all.set_enabled(!data.to_download.is_empty());
            }
            PageKind::Preview => {
                let store = imp.preview.borrow().as_ref().map(|(s, _)| s.is_some());
                parts.view_in_store.set_enabled(store.unwrap_or(false));
                self.show_subscribe();
            }
        }
        imp.data.replace(Some(data));
        self.show_episodes();
    }

    fn change_prefs(&self, change: impl FnOnce(&mut ShowPrefs)) {
        let imp = self.imp();
        let Some((show, old)) = imp.data.borrow().as_ref().map(|d| (d.show.id, d.prefs)) else {
            return;
        };
        let mut prefs = old;
        change(&mut prefs);
        if prefs != old {
            imp.output.emit(ShowPageOutput::PrefsChanged(show, prefs));
        }
    }

    /// List the hero, then each season's heading and, unless folded, its
    /// episodes; or the episodes alone when the feed has no seasons. A
    /// preview whose feed has not loaded says so instead.
    fn show_episodes(&self) {
        let parts = self.parts();
        let shown = self.imp().data.borrow();
        let Some(data) = shown.as_ref() else {
            return;
        };
        let show = data.show.id;
        let mut all = Vec::new();
        let mut entries = vec![Entry::Header];
        match &data.seasons {
            Some(seasons) => {
                // Seasons 24px apart, as GNOME Podcasts spaces its groups.
                for (i, season) in seasons
                    .iter()
                    .filter(|s| !s.episodes.is_empty())
                    .enumerate()
                {
                    let key = (show, season.season.as_ref().map(|s| s.number));
                    entries.push(Entry::Heading {
                        title: format::season_title(season.season.as_ref()),
                        fold: Some(key),
                        top: if i == 0 { 12 } else { 24 },
                    });
                    if !parts.folding.folded.borrow().contains(&key) {
                        entries.extend(Entry::card(&season.episodes));
                    }
                    all.extend(season.episodes.iter().cloned());
                }
            }
            None if !data.episodes.is_empty() => {
                entries.push(Entry::Space(12));
                entries.extend(Entry::card(&data.episodes));
                all.extend(data.episodes.iter().cloned());
            }
            None => {}
        }
        if all.is_empty() {
            let message = self
                .preview_message()
                .unwrap_or_else(|| menu::empty_message(&data.prefs, data.query.is_some()));
            entries.push(Entry::Message(message));
        }
        entries.push(Entry::Space(24));
        drop(shown);
        parts.rows.selection.set_items(all);
        parts.list.set(entries);
    }
}
