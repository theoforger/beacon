// SPDX-License-Identifier: GPL-3.0-or-later

//! The centre pane, an `AdwNavigationView`. Its root page shows the current
//! section: header (Refresh in Shows and New, title, Search, details
//! toggle), the search bar, and one page per section. Deeper pages (Add, a
//! podcast page) slide in from the right over whatever section is showing;
//! the view owns that stack and its Back, two-finger swipe, mouse back
//! button and Alt+Left. The pane routes data from the window to its views
//! and their actions back.

use beacon_core::browse::RecencyGroup;
use beacon_core::model::{EpisodeFilter, EpisodeId, ShowId, ShowPrefs};
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, adw, gtk,
};
use url::Url;

use crate::navigation::Location;
use crate::ui::add_page::{ADD_TAG, AddPage, AddPageInput, AddPageOutput};
use crate::ui::labels;
use crate::ui::models::{EpisodeItem, RowAction, ShowCard, ShowPageData};
use crate::ui::new_view::{NewView, NewViewInput, NewViewOutput};
use crate::ui::show_page::{SHOW_TAG, ShowPage, ShowPageInput, ShowPageOutput};
use crate::ui::shows_grid::{ShowsGrid, ShowsGridInput, ShowsGridOutput};

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
        filter: EpisodeFilter,
        has_shows: bool,
    },
    /// The Add page's subscription finished, either way.
    SubscribeDone,
}

#[derive(Debug)]
pub enum ContentOutput {
    Action(EpisodeId, RowAction),
    OpenShow(ShowId),
    NewFilterChanged(EpisodeFilter),
    ShowPrefsChanged(ShowId, ShowPrefs),
    MarkAllPlayed(ShowId),
    Unsubscribe(ShowId),
    FeedAddressCopied,
    Subscribe(Url),
}

pub struct Content {
    shows: Controller<ShowsGrid>,
    new: Controller<NewView>,
    show_page: Controller<ShowPage>,
    add_page: Controller<AddPage>,
}

#[relm4::component(pub)]
impl Component for Content {
    type Init = ();
    type Input = ContentInput;
    type Output = ContentOutput;
    type CommandOutput = ();

    view! {
        #[name = "navigation"]
        adw::NavigationView {
            // Type to search only where the search bar is: the root page.
            connect_visible_page_notify[search_bar] => move |navigation| {
                let on_root = navigation.visible_page_tag().as_deref() == Some(ROOT_TAG);
                let window = on_root.then(|| search_bar.root()).flatten();
                search_bar.set_key_capture_widget(window.as_ref());
                if !on_root {
                    search_bar.set_search_mode(false);
                }
            },

            #[name = "root_page"]
            add = &adw::NavigationPage {
                set_tag: Some(ROOT_TAG),
                set_title: &labels::title(Location::Shows),

                #[wrap(Some)]
                set_child = &adw::ToolbarView {
                    add_top_bar = &adw::HeaderBar {
                        // Only in Shows and New, where new episodes appear.
                        #[name = "refresh"]
                        pack_start = &gtk::Button {
                            set_icon_name: "view-refresh-symbolic",
                            set_action_name: Some("win.refresh"),
                            set_tooltip_text: Some(&gettext("Refresh Feeds")),
                            update_property: &[gtk::accessible::Property::Label(&gettext("Refresh Feeds"))],
                        },

                        #[name = "details_toggle"]
                        pack_end = &new_details_toggle() -> gtk::ToggleButton {},

                        #[name = "search_toggle"]
                        pack_end = &gtk::ToggleButton {
                            set_icon_name: "system-search-symbolic",
                            set_tooltip_text: Some(&gettext("Search")),
                            update_property: &[gtk::accessible::Property::Label(&gettext("Search"))],
                        },
                    },

                    #[name = "search_bar"]
                    add_top_bar = &gtk::SearchBar {
                        #[wrap(Some)]
                        set_child = &adw::Clamp {
                            set_maximum_size: 400,
                            // Results arrive with M6.
                            #[name = "search_entry"]
                            gtk::SearchEntry {
                                set_hexpand: true,
                                set_placeholder_text: Some(&gettext("Search")),
                            },
                        },
                    },

                    #[wrap(Some)]
                    #[name = "stack"]
                    set_content = &gtk::Stack {
                        set_transition_type: gtk::StackTransitionType::Crossfade,
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let output = sender.output_sender().clone();
        let input = sender.input_sender().clone();
        let shows = ShowsGrid::builder().launch(()).connect_receiver({
            let output = output.clone();
            move |_, message| match message {
                ShowsGridOutput::OpenShow(id) => {
                    let _ = output.send(ContentOutput::OpenShow(id));
                }
                ShowsGridOutput::AddPodcast => input.emit(ContentInput::OpenAdd),
            }
        });
        let new = NewView::builder()
            .launch(())
            .forward(&output, |message| match message {
                NewViewOutput::Action(id, action) => ContentOutput::Action(id, action),
                NewViewOutput::FilterChanged(filter) => ContentOutput::NewFilterChanged(filter),
            });
        let show_page = ShowPage::builder()
            .launch(())
            .forward(&output, |message| match message {
                ShowPageOutput::Action(id, action) => ContentOutput::Action(id, action),
                ShowPageOutput::PrefsChanged(id, prefs) => {
                    ContentOutput::ShowPrefsChanged(id, prefs)
                }
                ShowPageOutput::MarkAllPlayed(id) => ContentOutput::MarkAllPlayed(id),
                ShowPageOutput::Unsubscribe(id) => ContentOutput::Unsubscribe(id),
                ShowPageOutput::FeedAddressCopied => ContentOutput::FeedAddressCopied,
            });
        let add_page = AddPage::builder()
            .launch(())
            .forward(&output, |AddPageOutput::Subscribe(url)| {
                ContentOutput::Subscribe(url)
            });

        let model = Content {
            shows,
            new,
            show_page,
            add_page,
        };
        let widgets = view_output!();

        // Every page has its own details toggle; they all follow the root
        // page's, which the window keeps in step with the pane.
        for header in [
            model.add_page.widgets().header().clone(),
            model.show_page.widgets().header().clone(),
        ] {
            let toggle = new_details_toggle();
            header.pack_end(&toggle);
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

        for location in Location::ALL {
            let page: gtk::Widget = match location {
                Location::Shows => model.shows.widget().clone().upcast(),
                Location::New => model.new.widget().clone().upcast(),
                other => empty_state(other).upcast(),
            };
            widgets.stack.add_named(&page, Some(section_id(location)));
        }

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        let navigation = &widgets.navigation;
        match message {
            ContentInput::ShowSection(location) => {
                widgets.stack.set_visible_child_name(section_id(location));
                widgets.refresh.set_visible(location.has_refresh());
                widgets.root_page.set_title(&labels::title(location));
                // Leaving deeper pages for a section: no slide, the section
                // simply changes.
                if navigation.visible_page_tag().as_deref() != Some(ROOT_TAG) {
                    navigation.replace(std::slice::from_ref(&widgets.root_page));
                }
            }
            ContentInput::OpenAdd => {
                self.add_page.widgets().clear();
                bring_forward(navigation, self.add_page.widget(), ADD_TAG);
            }
            ContentInput::OpenShow(data) => {
                self.show_page.emit(ShowPageInput::Set(data));
                bring_forward(navigation, self.show_page.widget(), SHOW_TAG);
            }
            ContentInput::UpdateShow(data) => {
                if navigation.find_page(SHOW_TAG).is_some() {
                    self.show_page.emit(ShowPageInput::Set(data));
                }
            }
            ContentInput::CloseShow => {
                if navigation.find_page(SHOW_TAG).is_some() {
                    navigation.replace(std::slice::from_ref(&widgets.root_page));
                }
            }
            ContentInput::SetShows(cards) => self.shows.emit(ShowsGridInput::SetCards(cards)),
            ContentInput::SetNew {
                items,
                filter,
                has_shows,
            } => self.new.emit(NewViewInput::Set {
                items,
                filter,
                has_shows,
            }),
            ContentInput::SubscribeDone => self.add_page.emit(AddPageInput::Done),
        }
    }
}

/// Show `page`: push it, or go back to it if it is already further down.
fn bring_forward(navigation: &adw::NavigationView, page: &adw::NavigationPage, tag: &str) {
    if navigation.visible_page_tag().as_deref() == Some(tag) {
        return;
    }
    match navigation.find_page(tag) {
        Some(existing) => {
            navigation.pop_to_page(&existing);
        }
        None => navigation.push(page),
    }
}

impl ContentWidgets {
    /// The root page's details toggle. The toggles on deeper pages follow it.
    pub fn details_toggle(&self) -> &gtk::ToggleButton {
        &self.details_toggle
    }

    /// Type to search: typing anywhere in `window` opens the search bar
    /// while the root page shows.
    pub fn capture_typing(&self, window: &impl IsA<gtk::Widget>) {
        self.search_bar.set_key_capture_widget(Some(window));
    }

    /// Leave the deeper page, as its Back button does.
    pub fn go_back(&self) {
        self.navigation.pop();
    }

    /// Ctrl+F: open the search bar, or close it if it is open. Only the root
    /// page has one.
    pub fn toggle_search(&self) {
        if self.navigation.visible_page_tag().as_deref() != Some(ROOT_TAG) {
            return;
        }
        self.search_toggle
            .set_active(!self.search_toggle.is_active());
    }
}

fn new_details_toggle() -> gtk::ToggleButton {
    let toggle = gtk::ToggleButton::builder()
        .icon_name("sidebar-show-right-symbolic")
        .tooltip_text(gettext("Episode Details"))
        .build();
    toggle.update_property(&[gtk::accessible::Property::Label(&gettext(
        "Episode Details",
    ))]);
    toggle
}

fn section_id(location: Location) -> &'static str {
    match location {
        Location::Shows => "shows",
        Location::New => "new",
        Location::Queue => "queue",
        Location::Favorites => "favorites",
        Location::Downloaded => "downloaded",
    }
}

/// Sections whose views arrive later (M5) show their empty state.
fn empty_state(location: Location) -> adw::StatusPage {
    let (title, description) = match location {
        Location::Queue => (
            gettext("Queue Is Empty"),
            gettext("Add episodes to the queue to play them next"),
        ),
        Location::Favorites => (
            gettext("No Favorites"),
            gettext("Mark an episode as a favorite to find it here"),
        ),
        Location::Downloaded => (
            gettext("No Downloads"),
            gettext("Download an episode to listen offline"),
        ),
        Location::Shows | Location::New => unreachable!("Shows and New have their own views"),
    };
    adw::StatusPage::builder()
        .icon_name(labels::icon(location))
        .title(title)
        .description(description)
        .build()
}
