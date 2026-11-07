// SPDX-License-Identifier: GPL-3.0-or-later

//! The centre pane, an `AdwNavigationView`. Its root page shows the current
//! section: header (Refresh in Shows and New, title, Search, details
//! toggle), the search bar, and one page per section. Deeper pages (Add, a
//! podcast page) slide in from the right over whatever section is showing;
//! the view owns that stack and its Back, two-finger swipe, mouse back
//! button and Alt+Left. The pane routes data from the window to its views
//! and their actions back.

use beacon_core::browse::RecencyGroup;
use beacon_core::model::{EpisodeChange, EpisodeId, ShowId, ShowPrefs};
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
use crate::ui::page_filters;
use crate::ui::selection::Selection;
use crate::ui::selection_bar;
use crate::ui::show_page::{SHOW_TAG, ShowPage, ShowPageInput, ShowPageOutput};
use crate::ui::shows_grid::{ShowsGrid, ShowsGridInput, ShowsGridOutput};

mod sections;
use sections::{bring_forward, empty_state, new_details_toggle, section_id};

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
}

#[derive(Debug)]
pub enum ContentOutput {
    Action(Box<EpisodeItem>, RowAction),
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
}

pub struct Content {
    shows: Controller<ShowsGrid>,
    new: Controller<NewView>,
    show_page: Controller<ShowPage>,
    add_page: Controller<AddPage>,
    /// New's sort and filters, as last set by the window.
    new_prefs: ShowPrefs,
    /// New's filter button, shown only in New.
    new_filter: page_filters::FilterButton,
    /// New's selection; its selection mode is the root page's.
    new_selection: Selection,
    /// The Select button, shown only in New.
    new_select: gtk::Button,
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
                #[name = "root_toolbar"]
                set_child = &adw::ToolbarView {
                    #[name = "root_header"]
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

                        // Sort and filter, only in New, as on podcast pages.
                        pack_end: model.new_filter.widget(),

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
        let new_selection = Selection::default();
        let new = NewView::builder()
            .launch(new_selection.clone())
            .forward(&output, |message| match message {
                NewViewOutput::Action(id, action) => ContentOutput::Action(id, action),
            });
        // The podcast page packs its details toggle itself, at the far end
        // of its header like the root page's, before its filter button.
        let show_toggle = new_details_toggle();
        let show_page = ShowPage::builder().launch(show_toggle.clone()).forward(
            &output,
            |message| match message {
                ShowPageOutput::Action(id, action) => ContentOutput::Action(id, action),
                ShowPageOutput::PrefsChanged(id, prefs) => {
                    ContentOutput::ShowPrefsChanged(id, prefs)
                }
                ShowPageOutput::MarkAll(id, played) => ContentOutput::MarkAll(id, played),
                ShowPageOutput::Unsubscribe(id) => ContentOutput::Unsubscribe(id),
                ShowPageOutput::FeedAddressCopied => ContentOutput::FeedAddressCopied,
                ShowPageOutput::Change(ids, change) => ContentOutput::Change(ids, change),
            },
        );
        let add_page = AddPage::builder()
            .launch(())
            .forward(&output, |message| match message {
                AddPageOutput::Subscribe(url) => ContentOutput::Subscribe(url),
                AddPageOutput::Back => ContentOutput::GoBack,
            });

        let model = Content {
            shows,
            new,
            show_page,
            add_page,
            new_prefs: ShowPrefs::default(),
            new_filter: page_filters::FilterButton::new({
                let s = sender.clone();
                move |choice| s.input(ContentInput::ChooseNew(choice))
            }),
            new_select: selection_bar::select_button(&new_selection),
            new_selection,
        };
        model.new_filter.widget().set_visible(false);
        let widgets = view_output!();

        // Every page has its own details toggle; they all follow the root
        // page's, which the window keeps in step with the pane.
        let add_toggle = new_details_toggle();
        model.add_page.widgets().header().pack_end(&add_toggle);
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

        model.new_select.set_visible(false);
        widgets.root_header.pack_end(&model.new_select);
        sections::attach_new_selection(
            &widgets.root_toolbar,
            &widgets.root_header,
            &model.new_select,
            &model.new_selection,
            &widgets.search_bar,
            &widgets.navigation,
            Box::new(move |ids, change| {
                let _ = output.send(ContentOutput::Change(ids, change));
            }),
        );

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
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        let navigation = &widgets.navigation;
        match message {
            ContentInput::ShowSection(location) => {
                widgets.stack.set_visible_child_name(section_id(location));
                widgets.refresh.set_visible(location.has_refresh());
                self.new_selection.set_active(false);
                self.new_select.set_visible(location == Location::New);
                self.new_filter
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
                prefs,
                has_shows,
            } => {
                self.new_prefs = prefs;
                self.new_filter.set_prefs(&prefs);
                self.new.emit(NewViewInput::Set {
                    items,
                    prefs,
                    has_shows,
                });
            }
            ContentInput::SubscribeDone => self.add_page.emit(AddPageInput::Done),
            ContentInput::ChooseNew(choice) => {
                let prefs = choice.apply(self.new_prefs);
                if prefs != self.new_prefs {
                    self.new_prefs = prefs;
                    let _ = sender.output(ContentOutput::NewPrefsChanged(prefs));
                }
            }
        }
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
    /// Leave the deeper page showing, if there is one.
    pub fn go_back(&self) -> bool {
        self.navigation.pop()
    }

    /// Ctrl+F: open the search bar, or close it if it is open. Only the root
    /// page has one.
    pub fn toggle_search(&self) {
        if self.navigation.visible_page_tag().as_deref() != Some(ROOT_TAG)
            || !self.root_header.is_visible()
        {
            return;
        }
        self.search_toggle
            .set_active(!self.search_toggle.is_active());
    }
}
