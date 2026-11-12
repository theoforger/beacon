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
use beacon_core::model::ShowPrefs;
use gtk::glib;

use crate::navigation::Location;
use crate::ui::add_page::AddPage;
use crate::ui::labels;
use crate::ui::messages::{Inbox, Output};
use crate::ui::new_view::NewViewInput;
use crate::ui::page_filters;
use crate::ui::preview_page::{PREVIEW_TAG, PreviewPage};
use crate::ui::search_view::SearchView;
use crate::ui::selection_bar;
use crate::ui::show_page::{SHOW_TAG, ShowPage, ShowPageInput};
use crate::ui::shows_grid::{ShowsGrid, ShowsGridInput, ShowsGridOutput};

mod build;
mod directory;
mod lists;
mod messages;
mod routes;
pub(crate) mod search;
mod sections;
pub use build::ContentWidgets;
pub use messages::{ContentInput, ContentOutput, SearchScope};
use routes::{search_output, show_page_output};
use sections::{bring_forward, new_details_toggle, section_id};

const ROOT_TAG: &str = "root";

mod imp {
    use std::cell::{Cell, OnceCell};

    use super::*;

    #[derive(Default)]
    pub struct Content {
        pub parts: OnceCell<super::Parts>,
        pub widgets: OnceCell<ContentWidgets>,
        /// New's sort and filters, as last set by the window.
        pub new_prefs: Cell<ShowPrefs>,
        /// The section showing, under any search results.
        pub section: Cell<Option<Location>>,
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
    preview_page: PreviewPage,
    /// The root page's search results.
    search: SearchView,
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
        let search = SearchView::default();
        search.connect_output({
            let (output, input) = (output.clone(), input.clone());
            move |message| search_output(&output, &input, message)
        });
        let lists = lists::Lists::new(&output);
        lists.connect_clear(&output);
        // The podcast page packs its details toggle itself, at the far end
        // of its header like the root page's, before its filter button.
        let show_toggle = new_details_toggle();
        let show_page = ShowPage::new(&show_toggle);
        show_page.connect_output({
            let output = output.clone();
            move |message| show_page_output(&output, message)
        });
        let (add_page, preview_page) = directory::pages(&output);
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
        let preview_toggle = new_details_toggle();
        preview_page.pack_end_buttons(&preview_toggle);
        for toggle in [add_toggle, show_toggle, preview_toggle] {
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
        search::connect_query(&widgets.search_bar, &widgets.search_entry, {
            let (output, content) = (output.clone(), self.downgrade());
            move |query| {
                let Some(content) = content.upgrade() else {
                    return;
                };
                content.show_search(query.is_some());
                let section = content.imp().section.get().unwrap_or(Location::Shows);
                output.emit(ContentOutput::Search(SearchScope::Section(section), query));
            }
        });

        widgets
            .root_header
            .pack_end(&selection_bar::select_holder(&lists.select));
        widgets.start.append(&lists.clear);
        // The start goes when neither shows, or the header would still
        // space it from the title: 6px New's narrow header has not got.
        let fit_start = {
            let (start, holder, refresh, clear) = (
                widgets.start.clone(),
                widgets.refresh_holder.clone(),
                widgets.refresh.clone(),
                lists.clear.clone(),
            );
            // Each one's own flag: `is_visible` also asks the start, which
            // this hides.
            let shown = |widget: &gtk::Widget| widget.property::<bool>("visible");
            std::rc::Rc::new(move || {
                start.set_visible(
                    (shown(holder.upcast_ref()) && shown(refresh.upcast_ref()))
                        || shown(clear.upcast_ref()),
                );
            })
        };
        fit_start();
        for widget in [
            widgets.refresh_holder.upcast_ref::<gtk::Widget>(),
            widgets.refresh.upcast_ref(),
            lists.clear.upcast_ref(),
        ] {
            let fit_start = fit_start.clone();
            widget.connect_visible_notify(move |_| fit_start());
        }
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
        widgets.stack.add_named(&search, Some(search::RESULTS));
        self.set_child(Some(&widgets.navigation));

        let imp = self.imp();
        let _ = imp.widgets.set(widgets);
        let _ = imp.parts.set(Parts {
            shows,
            lists,
            show_page,
            add_page,
            preview_page,
            search,
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

    /// The header's Select, for tests.
    #[cfg(test)]
    pub fn select_button(&self) -> gtk::Button {
        let parts = self.imp().parts.get().expect("built");
        parts.lists.select.clone()
    }

    /// The sections' selection, for tests.
    #[cfg(test)]
    pub fn selection(&self) -> crate::ui::selection::Selection {
        let parts = self.imp().parts.get().expect("built when constructed");
        parts.lists.selection.clone()
    }

    /// Refresh Feeds and Clear, which share the header's start, for tests.
    #[cfg(test)]
    pub fn start_buttons(&self) -> (gtk::Button, gtk::Button) {
        let parts = self.imp().parts.get().expect("built when constructed");
        (self.widgets().refresh.clone(), parts.lists.clear.clone())
    }

    /// The root page shows the search results, or its section.
    fn show_search(&self, searching: bool) {
        let location = self.imp().section.get().unwrap_or(Location::Shows);
        search::show_results(
            self.widgets(),
            searching,
            section_id(location),
            &labels::title(location),
        );
    }

    /// Ctrl+F: open or close the search bar of the page showing.
    pub fn toggle_search(&self) {
        let widgets = self.widgets();
        if widgets.navigation.visible_page_tag().as_deref() == Some(SHOW_TAG) {
            let parts = self.imp().parts.get().expect("built when constructed");
            parts.show_page.toggle_search();
        } else {
            widgets.toggle_search();
        }
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
                imp.section.set(Some(location));
                // A section starts without a search, as in the mockup, and
                // its field says what it searches.
                widgets.search_bar.set_search_mode(false);
                widgets
                    .search_entry
                    .set_placeholder_text(Some(&search::placeholder(location)));
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
            ContentInput::OpenShow(data) => {
                parts.show_page.send(ShowPageInput::Set(data));
                bring_forward(navigation, parts.show_page.upcast_ref(), SHOW_TAG);
                // Subscribed from its preview: Back skips the preview.
                if navigation.find_page(PREVIEW_TAG).is_some() {
                    navigation.remove(&parts.preview_page);
                }
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
            ContentInput::Refreshing(running) => parts.refresh_bar.set_running(running),
            ContentInput::SetQueue(data) => parts.lists.set_queue(data),
            ContentInput::SetFavorites(items) => parts.lists.set_favorites(items),
            ContentInput::SetDownloaded(items) => parts.lists.set_downloaded(items),
            ContentInput::OpenAdd
            | ContentInput::SubscribeDone
            | ContentInput::DirectoryResults(..)
            | ContentInput::DirectoryArt(..)
            | ContentInput::OpenPreview(_)
            | ContentInput::PreviewLoaded(..)
            | ContentInput::SetSearch(_)
            | ContentInput::SearchOnline(_) => self.update_directory(message),
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
