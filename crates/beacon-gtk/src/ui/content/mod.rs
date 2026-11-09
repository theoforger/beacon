// SPDX-License-Identifier: GPL-3.0-or-later

//! The centre pane, an `AdwNavigationView`. Its root page shows the current
//! section: header (Refresh in Shows and New, title, Search, details
//! toggle), the search bar, and one page per section. Deeper pages (Add, a
//! podcast page) slide in from the right over whatever section is showing;
//! the view owns that stack and its Back, two-finger swipe, mouse back
//! button and Alt+Left. The pane routes data from the window to its views
//! and their actions back.

use adw::prelude::*;
use adw::subclass::prelude::*;
use beacon_core::browse::RecencyGroup;
use beacon_core::model::{EpisodeChange, EpisodeId, ShowId, ShowPrefs};
use gtk::glib;
use url::Url;

use crate::navigation::Location;
use crate::ui::add_page::{ADD_TAG, AddPage, AddPageInput, AddPageOutput};
use crate::ui::labels;
use crate::ui::messages::{Inbox, Output};
use crate::ui::models::{EpisodeItem, RowAction, ShowCard, ShowPageData};
use crate::ui::new_view::NewViewInput;
use crate::ui::page_filters;
use crate::ui::queue_data::QueueData;
use crate::ui::show_page::{SHOW_TAG, ShowPage, ShowPageInput, ShowPageOutput};
use crate::ui::shows_grid::{ShowsGrid, ShowsGridInput, ShowsGridOutput};
use beacon_core::queue::Source;

mod build;
mod lists;
mod sections;
pub use build::ContentWidgets;
use sections::{bring_forward, new_details_toggle, section_id};

const ROOT_TAG: &str = "root";

#[derive(Debug)]
pub enum ContentInput {
    /// Show a section at its top level, leaving any deeper pages.
    ShowSection(Location),
    /// Slide the Add page in over the current section.
    OpenAdd,
    /// Slide a podcast page in, or refresh it if it is already open.
    OpenShow(Box<ShowPageData>),
    /// Refresh the podcast page if it is open; never opens it.
    UpdateShow(Box<ShowPageData>),
    /// The show is gone: leave its page.
    CloseShow,
    SetShows(Vec<ShowCard>),
    SetNew {
        items: Vec<(RecencyGroup, EpisodeItem)>,
        prefs: ShowPrefs,
        has_shows: bool,
    },
    /// The Add page's subscription finished, either way.
    SubscribeDone,
    /// A choice in New's filter menu: the group's action name and the value.
    ChooseNew(page_filters::Choice),
    /// Whether feeds are refreshing.
    Refreshing(bool),
    SetQueue(Box<QueueData>),
    SetFavorites(Vec<EpisodeItem>),
    SetDownloaded(Vec<EpisodeItem>),
}

#[derive(Debug)]
pub enum ContentOutput {
    /// A row's action, with the list it is in: what plays after it, unless
    /// it is the Queue's own (`None`), which leaves that as it was.
    Action(Box<EpisodeItem>, RowAction, Option<Source>),
    OpenShow(ShowId),
    /// A choice in New's filter menu.
    NewPrefsChanged(ShowPrefs),
    ShowPrefsChanged(ShowId, ShowPrefs),
    /// Mark every episode of a show played (`true`) or unplayed.
    MarkAll(ShowId, bool),
    Unsubscribe(ShowId),
    FeedAddressCopied,
    Subscribe(Url),
    /// Leave the page showing, as Back does.
    GoBack,
    /// A change to the selected episodes.
    Change(Vec<EpisodeId>, EpisodeChange),
    /// The Queue's Clear, with the episodes it held.
    ClearQueue(Vec<EpisodeId>),
    /// Downloaded's Clear, with the downloaded episodes.
    DeleteAllDownloads(Vec<EpisodeId>),
}

mod imp {
    use std::cell::{Cell, OnceCell};

    use super::*;

    #[derive(Default)]
    pub struct Content {
        pub parts: OnceCell<super::Parts>,
        pub widgets: OnceCell<ContentWidgets>,
        /// New's sort and filters, as last set by the window.
        pub new_prefs: Cell<ShowPrefs>,
        pub inbox: Inbox<ContentInput>,
        pub output: Output<ContentOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Content {
        const NAME: &'static str = "BeaconContent";
        type Type = super::Content;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for Content {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }
    impl WidgetImpl for Content {}
    impl BinImpl for Content {}
}

glib::wrapper! {
    pub struct Content(ObjectSubclass<imp::Content>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

pub struct Parts {
    shows: ShowsGrid,
    /// New, Queue, Favorites and Downloaded, with their selection.
    lists: lists::Lists,
    show_page: ShowPage,
    add_page: AddPage,
    /// New's filter button, shown only in New.
    new_filter: page_filters::FilterButton,
    refresh_bar: sections::RefreshBar,
}

impl Default for Content {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Content {
    fn build(&self) {
        let output = self.imp().output.clone();
        let input = {
            let content = self.downgrade();
            move |message: ContentInput| {
                if let Some(content) = content.upgrade() {
                    content.send(message);
                }
            }
        };
        let shows = ShowsGrid::default();
        shows.connect_output({
            let (output, input) = (output.clone(), input.clone());
            move |message| match message {
                ShowsGridOutput::OpenShow(id) => output.emit(ContentOutput::OpenShow(id)),
                ShowsGridOutput::AddPodcast => input(ContentInput::OpenAdd),
            }
        });
        let lists = lists::Lists::new(&output);
        lists.connect_clear(&output);
        // The podcast page packs its details toggle itself, at the far end
        // of its header like the root page's, before its filter button.
        let show_toggle = new_details_toggle();
        let show_page = ShowPage::new(&show_toggle);
        show_page.connect_output({
            let output = output.clone();
            move |message| {
                output.emit(match message {
                    ShowPageOutput::Action(item, action) => {
                        let source = Source::Show(item.episode.show_id);
                        ContentOutput::Action(item, action, Some(source))
                    }
                    ShowPageOutput::PrefsChanged(id, prefs) => {
                        ContentOutput::ShowPrefsChanged(id, prefs)
                    }
                    ShowPageOutput::MarkAll(id, played) => ContentOutput::MarkAll(id, played),
                    ShowPageOutput::Unsubscribe(id) => ContentOutput::Unsubscribe(id),
                    ShowPageOutput::FeedAddressCopied => ContentOutput::FeedAddressCopied,
                    ShowPageOutput::Change(ids, change) => ContentOutput::Change(ids, change),
                });
            }
        });
        let add_page = AddPage::default();
        add_page.connect_output({
            let output = output.clone();
            move |message| {
                output.emit(match message {
                    AddPageOutput::Subscribe(url) => ContentOutput::Subscribe(url),
                    AddPageOutput::Back => ContentOutput::GoBack,
                });
            }
        });
        let new_filter = page_filters::FilterButton::new({
            let input = input.clone();
            move |choice| input(ContentInput::ChooseNew(choice))
        });
        new_filter.widget().set_visible(false);
        let refresh_bar = sections::RefreshBar::new();
        let widgets = build::widgets(new_filter.widget(), &refresh_bar);

        // Every page has its own details toggle; they all follow the root
        // page's, which the window keeps in step with the pane.
        let add_toggle = new_details_toggle();
        add_page.header().pack_end(&add_toggle);
        for toggle in [add_toggle, show_toggle] {
            widgets
                .details_toggle
                .bind_property("active", &toggle, "active")
                .bidirectional()
                .sync_create()
                .build();
            widgets
                .details_toggle
                .bind_property("visible", &toggle, "visible")
                .sync_create()
                .build();
        }

        widgets
            .search_toggle
            .bind_property("active", &widgets.search_bar, "search-mode-enabled")
            .bidirectional()
            .sync_create()
            .build();
        widgets.search_bar.connect_entry(&widgets.search_entry);

        widgets.root_header.pack_end(&lists.select);
        widgets.start.append(&lists.clear);
        sections::attach_new_selection(
            &widgets.root_toolbar,
            &widgets.root_header,
            &lists.select,
            &lists.selection,
            &widgets.search_bar,
            &widgets.navigation,
            Box::new(move |ids, change| output.emit(ContentOutput::Change(ids, change))),
        );

        for location in Location::ALL {
            let page: gtk::Widget = lists
                .page(location)
                .unwrap_or_else(|| shows.clone().upcast());
            widgets.stack.add_named(&page, Some(section_id(location)));
        }
        self.set_child(Some(&widgets.navigation));

        let imp = self.imp();
        let _ = imp.widgets.set(widgets);
        let _ = imp.parts.set(Parts {
            shows,
            lists,
            show_page,
            add_page,
            new_filter,
            refresh_bar,
        });
    }

    pub fn connect_output(&self, receive: impl Fn(ContentOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    pub fn send(&self, input: ContentInput) {
        self.imp()
            .inbox
            .deliver(input, |message| self.update(message));
    }

    /// Refresh Feeds and Clear, which share the header's start, for tests.
    #[cfg(test)]
    pub fn start_buttons(&self) -> (gtk::Button, gtk::Button) {
        let parts = self.imp().parts.get().expect("built when constructed");
        (self.widgets().refresh.clone(), parts.lists.clear.clone())
    }

    pub fn widgets(&self) -> &ContentWidgets {
        self.imp().widgets.get().expect("built when constructed")
    }

    fn update(&self, message: ContentInput) {
        let imp = self.imp();
        let parts = imp.parts.get().expect("built when constructed");
        let widgets = self.widgets();
        let navigation = &widgets.navigation;
        match message {
            ContentInput::ShowSection(location) => {
                widgets.stack.set_visible_child_name(section_id(location));
                widgets.refresh.set_visible(location.has_refresh());
                parts.lists.show(location);
                parts
                    .new_filter
                    .widget()
                    .set_visible(location == Location::New);
                widgets.root_page.set_title(&labels::title(location));
                // Leaving deeper pages for a section: no slide, the section
                // simply changes.
                if navigation.visible_page_tag().as_deref() != Some(ROOT_TAG) {
                    navigation.replace(std::slice::from_ref(&widgets.root_page));
                }
            }
            ContentInput::OpenAdd => {
                parts.add_page.clear();
                bring_forward(navigation, parts.add_page.upcast_ref(), ADD_TAG);
            }
            ContentInput::OpenShow(data) => {
                parts.show_page.send(ShowPageInput::Set(data));
                bring_forward(navigation, parts.show_page.upcast_ref(), SHOW_TAG);
            }
            ContentInput::UpdateShow(data) => {
                if navigation.find_page(SHOW_TAG).is_some() {
                    parts.show_page.send(ShowPageInput::Set(data));
                }
            }
            ContentInput::CloseShow => {
                if navigation.find_page(SHOW_TAG).is_some() {
                    navigation.replace(std::slice::from_ref(&widgets.root_page));
                }
            }
            ContentInput::SetShows(cards) => parts.shows.send(ShowsGridInput::SetCards(cards)),
            ContentInput::SetNew {
                items,
                prefs,
                has_shows,
            } => {
                imp.new_prefs.set(prefs);
                parts.new_filter.set_prefs(&prefs);
                parts.lists.set_new(NewViewInput::Set {
                    items,
                    prefs,
                    has_shows,
                });
            }
            ContentInput::SubscribeDone => parts.add_page.send(AddPageInput::Done),
            ContentInput::Refreshing(running) => parts.refresh_bar.set_running(running),
            ContentInput::SetQueue(data) => parts.lists.set_queue(data),
            ContentInput::SetFavorites(items) => parts.lists.set_favorites(items),
            ContentInput::SetDownloaded(items) => parts.lists.set_downloaded(items),
            ContentInput::ChooseNew(choice) => {
                let prefs = choice.apply(imp.new_prefs.get());
                if prefs != imp.new_prefs.get() {
                    imp.new_prefs.set(prefs);
                    imp.output.emit(ContentOutput::NewPrefsChanged(prefs));
                }
            }
        }
    }
}
