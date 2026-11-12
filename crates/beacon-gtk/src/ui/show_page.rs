// SPDX-License-Identifier: GPL-3.0-or-later

//! A podcast page, pushed over the current section: the hero, then the
//! episodes, all in one list view (`ui/episode_list.rs`) so that long feeds
//! scroll smoothly. When the feed has season tags, each season has its own
//! card under a `GtkExpander` heading, so it can be folded; the page
//! remembers which seasons are folded.

use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use beacon_core::model::{EpisodeId, ShowId, ShowPrefs};
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

use crate::ui::episode_list::{Entry, EpisodeList, Folding};
use crate::ui::format;
use crate::ui::messages::{Inbox, Output};
use crate::ui::models::{EpisodeItem, RowAction, ShowPageData};
use crate::ui::page_filters;
use crate::ui::selection::Selection;
use crate::ui::selection_bar::{self, Bulk};
use crate::ui::show_hero::ShowHero;
use crate::ui::show_menu::{empty_message, more_button, more_menu};
use crate::ui::show_search::ShowSearch;
use crate::ui::widgets::episode_row::RowSetup;

pub const SHOW_TAG: &str = "show";

#[derive(Debug)]
pub enum ShowPageInput {
    Set(Box<ShowPageData>),
    /// A choice in the filter menu: the group's action name and the value.
    Choose(page_filters::Choice),
    /// Mark All as Played, or as Unplayed when every episode is played.
    MarkAll,
    DownloadAll,
    CopyFeedAddress,
    Unsubscribe,
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
    use std::cell::{OnceCell, RefCell};

    use super::*;

    #[derive(Default)]
    pub struct ShowPage {
        pub parts: OnceCell<super::Parts>,
        pub data: RefCell<Option<ShowPageData>>,
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
    hero: ShowHero,
    /// The filter menu's radio groups, by action name.
    filter: page_filters::FilterButton,
    /// More Options: Mark All, Download All Episodes, Copy Feed Address.
    more: gtk::MenuButton,
    /// Download All Episodes, unavailable when all are downloaded.
    download_all: gio::SimpleAction,
    rows: RowSetup,
    list: EpisodeList,
    /// Folded seasons, kept while the app runs.
    folding: Folding,
    search: ShowSearch,
    /// Shows the header's title once the hero's has scrolled away.
    title_check: std::rc::Rc<dyn Fn()>,
}

impl ShowPage {
    /// `details_toggle`: the page's details toggle, kept in step by the
    /// content pane.
    pub fn new(details_toggle: &gtk::ToggleButton) -> Self {
        let page: Self = glib::Object::builder()
            .property("tag", SHOW_TAG)
            // Until the show's own title arrives: the page is pushed first,
            // and a page needs a title for Back and screen readers.
            .property("title", gettext("Podcast"))
            .build();
        let output = page.imp().output.clone();
        let input = {
            let page = page.downgrade();
            move |message: ShowPageInput| {
                if let Some(page) = page.upgrade() {
                    page.send(message);
                }
            }
        };
        // The page is the show's own: its name and cover would repeat on
        // every row.
        let rows = RowSetup {
            on_action: Rc::new({
                let output = output.clone();
                move |item, action| {
                    output.emit(ShowPageOutput::Action(Box::new(item.clone()), action));
                }
            }),
            show_identity: false,
            selection: Selection::default(),
            selectable: true,
        };
        let hero = ShowHero::new();
        let folding = Folding {
            folded: Rc::default(),
            changed: Rc::new({
                let input = input.clone();
                move || input(ShowPageInput::Refold)
            }),
        };
        // Translators: how a screen reader names a show's episode list.
        let list = EpisodeList::new(
            &rows,
            &gettext("Episodes"),
            Some(hero.widget().clone().upcast()),
            Some(folding.clone()),
        );
        let filter = page_filters::FilterButton::new({
            let input = input.clone();
            move |choice| input(ShowPageInput::Choose(choice))
        });
        let more = more_button();

        let search = ShowSearch::new({
            let (output, page) = (output.clone(), page.downgrade());
            move |query| {
                let show = page
                    .upgrade()
                    .and_then(|p| p.imp().data.borrow().as_ref().map(|d| d.show.id));
                if let Some(show) = show {
                    output.emit(ShowPageOutput::Search(show, query));
                }
            }
        });
        let header = adw::HeaderBar::builder().show_title(false).build();
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.add_top_bar(&search.bar);
        toolbar.set_content(Some(list.widget()));
        page.set_child(Some(&toolbar));

        // The details toggle at the far end, as on every page; then the
        // show's More Options menu, which the HIG puts in the header bar
        // ("Secondary menus are located in the header bar"), as GNOME
        // Podcasts does; then sort and filter.
        header.pack_end(details_toggle);
        header.pack_end(&more);
        header.pack_end(filter.widget());
        header.pack_end(&search.toggle);
        let select = selection_bar::select_button(&rows.selection);
        header.pack_end(&selection_bar::select_holder(&select));
        selection_bar::attach(
            &toolbar,
            &header,
            &select,
            &rows.selection,
            Box::new({
                let output = output.clone();
                move |ids, change| output.emit(ShowPageOutput::Change(ids, change))
            }),
        );
        let title_check =
            crate::ui::show_title::follow_scroll(&header, list.widget(), hero.title());
        // Type to search while the page shows, as on the root page and in
        // GNOME Podcasts (`show.rs`), except in selection mode. The window
        // catches the typing, as the root page's does: the page alone lost
        // the second letter while the field took focus.
        // On screen from when it maps until it starts to leave: during
        // Back's slide it is still mapped, and leaving selection mode then
        // gave it the typing back (found in the tests).
        let on_screen = Rc::new(std::cell::Cell::new(false));
        let capture = Rc::new({
            let (bar, weak) = (search.bar.clone(), page.downgrade());
            let (selecting, on_screen) = (rows.selection.clone(), on_screen.clone());
            move || {
                let window = weak
                    .upgrade()
                    .and_then(|page| page.root())
                    .filter(|_| on_screen.get() && !selecting.is_active());
                bar.set_key_capture_widget(window.as_ref());
            }
        });
        page.connect_map({
            let (on_screen, capture) = (on_screen.clone(), capture.clone());
            move |_| {
                on_screen.set(true);
                capture();
            }
        });
        page.connect_hiding({
            let (on_screen, capture) = (on_screen.clone(), capture.clone());
            move |_| {
                on_screen.set(false);
                capture();
            }
        });
        page.connect_unmap({
            let capture = capture.clone();
            move |_| {
                on_screen.set(false);
                capture();
            }
        });
        rows.selection.connect_changed(move |_| capture());
        // Leaving the page leaves selection mode and the search.
        let selection = rows.selection.clone();
        let bar = search.bar.clone();
        page.connect_hidden(move |_| {
            selection.set_active(false);
            bar.set_search_mode(false);
        });

        let actions = gio::SimpleActionGroup::new();
        let mark_all = gio::SimpleAction::new("mark-all", None);
        let i = input.clone();
        mark_all.connect_activate(move |_, _| i(ShowPageInput::MarkAll));
        actions.add_action(&mark_all);
        let download_all = gio::SimpleAction::new("download-all", None);
        let i = input.clone();
        download_all.connect_activate(move |_, _| i(ShowPageInput::DownloadAll));
        actions.add_action(&download_all);
        let copy = gio::SimpleAction::new("copy-feed", None);
        let i = input.clone();
        copy.connect_activate(move |_, _| i(ShowPageInput::CopyFeedAddress));
        actions.add_action(&copy);
        page.insert_action_group("show", Some(&actions));

        hero.unsubscribe
            .connect_clicked(move |_| input(ShowPageInput::Unsubscribe));

        let _ = page.imp().parts.set(Parts {
            hero,
            filter,
            more,
            download_all,
            rows,
            list,
            folding,
            search,
            title_check,
        });
        page
    }

    /// Ctrl+F: open or close the search bar.
    pub fn toggle_search(&self) {
        self.parts().search.toggle();
    }

    /// The page's search, for tests.
    #[cfg(test)]
    pub fn search(&self) -> &ShowSearch {
        &self.parts().search
    }

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

    fn update(&self, message: ShowPageInput) {
        let imp = self.imp();
        let parts = self.parts();
        match message {
            ShowPageInput::Set(data) => {
                let shown = imp
                    .data
                    .borrow()
                    .as_ref()
                    .map(|d| (d.show.id, d.all_played));
                if shown.map(|(id, _)| id) != Some(data.show.id) {
                    parts.rows.selection.set_active(false);
                    parts.search.close();
                    parts.list.set(Vec::new());
                }
                self.set_title(&data.show.info.title);
                crate::ui::show_title::check_later(&parts.title_check);
                parts.hero.update(&(&*data).into());
                if shown.map(|(_, all)| all) != Some(data.all_played) {
                    parts.more.set_menu_model(Some(&more_menu(data.all_played)));
                }
                parts.filter.set_prefs(&data.prefs);
                parts.download_all.set_enabled(!data.to_download.is_empty());
                imp.data.replace(Some(*data));
                self.show_episodes();
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
                let url = imp.data.borrow().as_ref().map(|d| d.show.feed_url.clone());
                if let (Some(url), Some(display)) = (url, gdk::Display::default()) {
                    display.clipboard().set_text(url.as_str());
                    imp.output.emit(ShowPageOutput::FeedAddressCopied);
                }
            }
            ShowPageInput::Unsubscribe => {
                let show = imp.data.borrow().as_ref().map(|d| d.show.id);
                if let Some(show) = show {
                    imp.output.emit(ShowPageOutput::Unsubscribe(show));
                }
            }
            ShowPageInput::Refold => self.show_episodes(),
        }
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
    /// episodes; or the episodes alone when the feed has no seasons.
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
            entries.push(Entry::Message(empty_message(
                &data.prefs,
                data.query.is_some(),
            )));
        }
        entries.push(Entry::Space(24));
        drop(shown);
        parts.rows.selection.set_items(all);
        parts.list.set(entries);
    }
}
