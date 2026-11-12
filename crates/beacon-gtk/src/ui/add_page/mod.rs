// SPDX-License-Identifier: GPL-3.0-or-later

//! The Add page, pushed over the current section (`docs/specs/
//! add-and-itunes.md`). One rounded search field with a Search button, as
//! in GNOME Podcasts: words search Apple's directory only on Enter or
//! Search, never while typing (asked by the user 2026-11-12); a feed
//! address shows an RSS Feed row with Add Feed as it is typed; an Apple
//! Podcasts link finds its show, on Enter or Search too. Results are a boxed list, as in GNOME Podcasts; each
//! opens a preview of the show.

use std::cell::{Cell, OnceCell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use beacon_core::ItunesError;
use beacon_core::model::ShowId;
use gettextrs::gettext;
use gtk::glib;
use url::Url;

use crate::ui::covers::Cover;
use crate::ui::directory_row::{self, DirectoryItem};
use crate::ui::errors;
use crate::ui::messages::Output;

mod build;
mod typed;
pub use typed::{Typed, classify, request_key};

pub const ADD_TAG: &str = "add";

#[derive(Debug)]
pub enum AddPageInput {
    TextChanged,
    /// Enter in the field: add the feed, or search.
    Activate,
    /// The Search button.
    SearchPressed,
    AddFeed,
    /// From the window: the subscription finished, either way.
    Done,
    /// Search Online: fill the field with this and search.
    Search(String),
    /// The directory's answer to a request (`request_key`).
    Results(String, Result<Vec<DirectoryItem>, ItunesError>),
    /// A result's thumbnail arrived.
    Art(Url, std::path::PathBuf),
    /// The feed was subscribed to (as this show) or unsubscribed from
    /// elsewhere: its result says so.
    Subscribed(Url, Option<ShowId>),
}

#[derive(Debug)]
pub enum AddPageOutput {
    Subscribe(Url),
    /// Search Apple's directory for these words.
    Search(String),
    /// Look up the show with this Apple Podcasts ID.
    Lookup(u64),
    /// Preview a result.
    Open(Box<DirectoryItem>),
    /// A result already subscribed to: open its page.
    OpenShow(ShowId),
    /// Escape in the empty search field: leave the page.
    Back,
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct AddPage {
        pub parts: OnceCell<super::Parts>,
        pub typed: RefCell<Option<Typed>>,
        pub adding: Cell<bool>,
        /// The page is setting the field's text itself.
        pub setting: Cell<bool>,
        /// The request whose answer the page waits for, or last showed.
        pub pending: RefCell<Option<String>>,
        pub items: RefCell<Vec<DirectoryItem>>,
        /// Each result row's cover, by its thumbnail's address.
        pub covers: RefCell<Vec<(Option<Url>, Cover)>>,
        pub output: Output<AddPageOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AddPage {
        const NAME: &'static str = "BeaconAddPage";
        type Type = super::AddPage;
        type ParentType = adw::NavigationPage;
    }

    impl ObjectImpl for AddPage {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }
    impl WidgetImpl for AddPage {}
    impl NavigationPageImpl for AddPage {}
}

glib::wrapper! {
    pub struct AddPage(ObjectSubclass<imp::AddPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

pub struct Parts {
    header: adw::HeaderBar,
    entry: gtk::SearchEntry,
    search: gtk::Button,
    note: gtk::Label,
    feed_list: gtk::ListBox,
    feed_row: adw::ActionRow,
    add: gtk::Button,
    stack: gtk::Stack,
    results: gtk::ListBox,
    error: adw::StatusPage,
}

impl Default for AddPage {
    fn default() -> Self {
        glib::Object::new()
    }
}

/// The stack's pages.
const START: &str = "start";
const SEARCHING: &str = "searching";
const RESULTS: &str = "results";
const NONE: &str = "none";
const ERROR: &str = "error";
const FEED: &str = "feed";

impl AddPage {
    pub fn connect_output(&self, receive: impl Fn(AddPageOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    pub fn send(&self, input: AddPageInput) {
        let imp = self.imp();
        match input {
            AddPageInput::TextChanged if imp.setting.get() => return,
            // Typing only says what the field holds: nothing goes online
            // before Enter or Search.
            AddPageInput::TextChanged => {
                let typed = classify(&self.parts().entry.text());
                if typed == Typed::Empty {
                    imp.pending.replace(None);
                }
                imp.typed.replace(Some(typed));
            }
            AddPageInput::SearchPressed => {
                let typed = classify(&self.parts().entry.text());
                imp.typed.replace(Some(typed.clone()));
                self.request(&typed);
            }
            AddPageInput::Activate => {
                let typed = classify(&self.parts().entry.text());
                imp.typed.replace(Some(typed.clone()));
                if matches!(typed, Typed::Feed(_)) {
                    self.send(AddPageInput::AddFeed);
                    return;
                }
                self.request(&typed);
            }
            AddPageInput::AddFeed => {
                let feed = match imp.typed.borrow().as_ref() {
                    Some(Typed::Feed(url)) => Some(url.clone()),
                    _ => None,
                };
                if let (Some(url), false) = (feed, imp.adding.get()) {
                    imp.adding.set(true);
                    imp.output.emit(AddPageOutput::Subscribe(url));
                }
            }
            AddPageInput::Done => imp.adding.set(false),
            AddPageInput::Search(words) => {
                // Set quietly: the search goes out now, not after the pause,
                // and the field's emptying on the way is not a change.
                imp.setting.set(true);
                self.parts().entry.set_text(&words);
                imp.setting.set(false);
                let typed = classify(&words);
                imp.typed.replace(Some(typed.clone()));
                self.request(&typed);
            }
            AddPageInput::Results(key, result) => {
                if imp.pending.borrow().as_deref() == Some(key.as_str()) {
                    self.show_results(result);
                }
                return;
            }
            AddPageInput::Subscribed(feed, show) => {
                self.mark_subscribed(&feed, show);
                return;
            }
            AddPageInput::Art(url, path) => {
                for item in imp.items.borrow_mut().iter_mut() {
                    if item.show.artwork.as_ref() == Some(&url) {
                        item.art = Some(path.clone());
                    }
                }
                for (art, cover) in imp.covers.borrow().iter() {
                    if art.as_ref() == Some(&url) {
                        cover.set(Some(&path));
                    }
                }
                return;
            }
        }
        self.refresh();
    }

    /// Ask the directory about `typed`, unless that is what the page
    /// already shows or waits for.
    fn request(&self, typed: &Typed) {
        let imp = self.imp();
        let key = request_key(typed);
        if key.is_none() || *imp.pending.borrow() == key {
            if key.is_none() {
                imp.pending.replace(None);
            }
            return;
        }
        imp.pending.replace(key);
        self.parts().stack.set_visible_child_name(SEARCHING);
        imp.output.emit(match typed {
            Typed::Apple(id) => AddPageOutput::Lookup(*id),
            Typed::Words(words) => AddPageOutput::Search(words.clone()),
            Typed::Empty | Typed::Feed(_) => return,
        });
    }

    fn show_results(&self, result: Result<Vec<DirectoryItem>, ItunesError>) {
        let imp = self.imp();
        let parts = self.parts();
        parts.results.remove_all();
        imp.covers.borrow_mut().clear();
        match result {
            Ok(items) if items.is_empty() => {
                imp.items.replace(Vec::new());
                parts.stack.set_visible_child_name(NONE);
            }
            Ok(items) => {
                let mut covers = imp.covers.borrow_mut();
                for item in &items {
                    let (row, cover) = directory_row::row(item);
                    parts.results.append(&row);
                    covers.push((item.show.artwork.clone(), cover));
                }
                imp.items.replace(items);
                parts.stack.set_visible_child_name(RESULTS);
            }
            Err(err) => {
                // Try again with the same words once it has passed.
                imp.pending.replace(None);
                imp.items.replace(Vec::new());
                parts.error.set_description(Some(&errors::itunes(&err)));
                parts.stack.set_visible_child_name(ERROR);
            }
        }
    }

    /// Redraw the result for `feed` as subscribed (`show`) or not.
    fn mark_subscribed(&self, feed: &Url, show: Option<ShowId>) {
        let imp = self.imp();
        let results = &self.parts().results;
        let mut items = imp.items.borrow_mut();
        let mut covers = imp.covers.borrow_mut();
        for (index, item) in items.iter_mut().enumerate() {
            if item.show.feed_url != *feed || item.subscribed == show {
                continue;
            }
            item.subscribed = show;
            let (row, cover) = directory_row::row(item);
            if let Some(old) = results.row_at_index(index as i32) {
                results.remove(&old);
            }
            results.insert(&row, index as i32);
            if let Some(slot) = covers.get_mut(index) {
                slot.1 = cover;
            }
        }
    }

    /// Whether the result for `feed` says Subscribed, for tests.
    #[cfg(test)]
    pub fn says_subscribed(&self, feed: &Url) -> Option<bool> {
        let items = self.imp().items.borrow();
        let item = items.iter().find(|i| i.show.feed_url == *feed)?;
        Some(item.subscribed.is_some())
    }

    fn open(&self, index: i32) {
        let imp = self.imp();
        let Some(item) = imp.items.borrow().get(index as usize).cloned() else {
            return;
        };
        imp.output.emit(match item.subscribed {
            Some(id) => AddPageOutput::OpenShow(id),
            None => AddPageOutput::Open(Box::new(item)),
        });
    }

    /// The feed row, the note and the stack's page, as things stand.
    fn refresh(&self) {
        let imp = self.imp();
        let parts = self.parts();
        let typed = imp.typed.borrow().clone().unwrap_or(Typed::Empty);
        let adding = imp.adding.get();
        let feed = match &typed {
            Typed::Feed(url) => Some(url.clone()),
            _ => None,
        };
        parts.feed_list.set_visible(feed.is_some());
        parts.note.set_visible(feed.is_none());
        parts
            .feed_row
            .set_subtitle(feed.as_ref().map(Url::as_str).unwrap_or_default());
        parts.add.set_label(&if adding {
            gettext("Adding…")
        } else {
            gettext("Add Feed")
        });
        parts.add.set_sensitive(!adding);
        parts
            .search
            .set_sensitive(matches!(typed, Typed::Apple(_) | Typed::Words(_)));
        match typed {
            Typed::Feed(_) => parts.stack.set_visible_child_name(FEED),
            Typed::Empty => parts.stack.set_visible_child_name(START),
            // Words not searched yet leave the last results, or say how
            // to search.
            Typed::Apple(_) | Typed::Words(_) => {
                if parts.stack.visible_child_name().as_deref() == Some(FEED) {
                    parts.stack.set_visible_child_name(START);
                }
            }
        }
    }

    fn parts(&self) -> &Parts {
        self.imp().parts.get().expect("built when constructed")
    }

    pub fn header(&self) -> &adw::HeaderBar {
        &self.parts().header
    }

    pub fn clear(&self) {
        let imp = self.imp();
        self.parts().entry.set_text("");
        imp.typed.replace(None);
        imp.pending.replace(None);
        self.refresh();
    }

    /// Type into the field, for tests.
    #[cfg(test)]
    pub fn type_text(&self, text: &str) {
        self.parts().entry.set_text(text);
    }

    /// Press Search, for tests.
    #[cfg(test)]
    pub fn press_search(&self) -> bool {
        let search = &self.parts().search;
        let sensitive = search.is_sensitive();
        search.emit_clicked();
        sensitive
    }

    /// The stack's page and the result titles, for tests.
    #[cfg(test)]
    pub fn showing(&self) -> (String, Vec<String>) {
        let parts = self.parts();
        let titles = self
            .imp()
            .items
            .borrow()
            .iter()
            .map(|i| i.show.title.clone())
            .collect();
        (
            parts
                .stack
                .visible_child_name()
                .map(|n| n.to_string())
                .unwrap_or_default(),
            titles,
        )
    }
}
