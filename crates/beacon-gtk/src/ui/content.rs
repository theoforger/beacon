// SPDX-License-Identifier: GPL-3.0-or-later

//! The centre pane, an `AdwNavigationView`. Its root page shows the current
//! section: header (Refresh in Shows and New, title, Search, details
//! toggle), the search bar, and one page per section. Deeper pages slide in
//! from the right over whatever section is showing; the view owns that
//! stack and its Back, two-finger swipe, mouse back button and Alt+Left.
//! Until the views land (M3 onwards) every page is its empty state.

use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{Component, ComponentParts, ComponentSender, adw, gtk};

use crate::navigation::Location;
use crate::ui::labels;

const ROOT_TAG: &str = "root";
const ADD_TAG: &str = "add";

#[derive(Debug)]
pub enum ContentInput {
    /// Show a section at its top level, leaving any deeper pages.
    ShowSection(Location),
    /// Slide the Add page in over the current section.
    OpenAdd,
}

pub struct Content {
    add_page: adw::NavigationPage,
}

#[relm4::component(pub)]
impl Component for Content {
    type Init = ();
    type Input = ContentInput;
    type Output = ();
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
                        // Enabled once feeds can be refreshed (M3).
                        #[name = "refresh"]
                        pack_start = &gtk::Button {
                            set_icon_name: "view-refresh-symbolic",
                            set_action_name: Some("win.refresh"),
                            set_tooltip_text: Some(&gettext("Refresh Feeds")),
                            update_property: &[gtk::accessible::Property::Label(&gettext("Refresh Feeds"))],
                        },

                        #[name = "details_toggle"]
                        pack_end = &details_toggle() -> gtk::ToggleButton {},

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
        let add_toggle = details_toggle();
        let model = Content {
            add_page: add_page(&add_toggle),
        };
        let widgets = view_output!();

        // Every page has its own details toggle; they all follow the root
        // page's, which the window keeps in step with the pane.
        widgets
            .details_toggle
            .bind_property("active", &add_toggle, "active")
            .bidirectional()
            .sync_create()
            .build();
        widgets
            .details_toggle
            .bind_property("visible", &add_toggle, "visible")
            .sync_create()
            .build();

        widgets
            .search_toggle
            .bind_property("active", &widgets.search_bar, "search-mode-enabled")
            .bidirectional()
            .sync_create()
            .build();
        widgets.search_bar.connect_entry(&widgets.search_entry);

        for location in Location::ALL {
            widgets.stack.add_named(
                &section_empty_state(location, &sender),
                Some(section_id(location)),
            );
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
            ContentInput::OpenAdd => match navigation.find_page(ADD_TAG) {
                None => navigation.push(&self.add_page),
                Some(page) if navigation.visible_page().as_ref() != Some(&page) => {
                    navigation.pop_to_page(&page);
                }
                Some(_) => {}
            },
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

fn details_toggle() -> gtk::ToggleButton {
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

/// The Add page. Its header gets Back from the navigation view; the search
/// field and results arrive in M6.
fn add_page(details_toggle: &gtk::ToggleButton) -> adw::NavigationPage {
    let header = adw::HeaderBar::new();
    header.pack_end(details_toggle);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&empty_state_page(
        "system-search-symbolic",
        gettext("Search for Podcasts"),
        gettext("Find a show by name, or paste the address of its feed"),
    )));

    adw::NavigationPage::builder()
        .tag(ADD_TAG)
        .title(gettext("Add Podcast"))
        .child(&toolbar)
        .build()
}

fn section_empty_state(location: Location, sender: &ComponentSender<Content>) -> adw::StatusPage {
    let (title, description) = match location {
        Location::Shows => (
            gettext("No Podcasts"),
            gettext("Add a podcast by searching for it or pasting its feed address"),
        ),
        Location::New => (
            gettext("No Episodes"),
            gettext("Subscribe to a podcast to see its episodes here"),
        ),
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
    };
    let page = empty_state_page(labels::icon(location), title, description);

    if location == Location::Shows {
        let add = gtk::Button::builder()
            .label(gettext("Add Podcast"))
            .halign(gtk::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .build();
        let sender = sender.clone();
        add.connect_clicked(move |_| {
            sender.input(ContentInput::OpenAdd);
        });
        page.set_child(Some(&add));
    }
    page
}

fn empty_state_page(icon: &str, title: String, description: String) -> adw::StatusPage {
    adw::StatusPage::builder()
        .icon_name(icon)
        .title(title)
        .description(description)
        .build()
}
