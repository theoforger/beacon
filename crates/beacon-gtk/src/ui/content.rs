// SPDX-License-Identifier: GPL-3.0-or-later

//! The centre pane: header (Back, title, Search, details toggle), the search
//! bar, and one page per location. Until the views land (M3 onwards) every
//! page is its empty state.

use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};

use crate::navigation::Location;
use crate::ui::labels;

#[derive(Debug)]
pub enum ContentInput {
    Show(Location),
    /// Whether a deeper page is open, so Back leads to its parent.
    SetCanGoBack(bool),
}

#[derive(Debug)]
pub enum ContentOutput {
    Back,
    AddPodcast,
}

pub struct Content {
    location: Location,
    can_go_back: bool,
}

#[relm4::component(pub)]
impl SimpleComponent for Content {
    type Init = ();
    type Input = ContentInput;
    type Output = ContentOutput;

    view! {
        adw::ToolbarView {
            add_top_bar = &adw::HeaderBar {
                // In the narrow layout the header's own Back returns to the
                // sidebar; on a deeper page ours replaces it.
                #[watch]
                set_show_back_button: !model.can_go_back,

                pack_start = &gtk::Button {
                    set_icon_name: "go-previous-symbolic",
                    set_tooltip_text: Some(&gettext("Back")),
                    update_property: &[gtk::accessible::Property::Label(&gettext("Back"))],
                    #[watch]
                    set_visible: model.can_go_back,
                    connect_clicked[sender] => move |_| {
                        let _ = sender.output(ContentOutput::Back);
                    },
                },

                #[name = "details_toggle"]
                pack_end = &gtk::ToggleButton {
                    set_icon_name: "sidebar-show-right-symbolic",
                    set_tooltip_text: Some(&gettext("Episode Details")),
                    update_property: &[gtk::accessible::Property::Label(&gettext("Episode Details"))],
                },

                #[name = "search_toggle"]
                pack_end = &gtk::ToggleButton {
                    set_icon_name: "system-search-symbolic",
                    set_tooltip_text: Some(&gettext("Search")),
                    update_property: &[gtk::accessible::Property::Label(&gettext("Search"))],
                    #[watch]
                    set_visible: model.location.has_search(),
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
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Content {
            location: Location::Shows,
            can_go_back: false,
        };
        let widgets = view_output!();

        widgets
            .search_toggle
            .bind_property("active", &widgets.search_bar, "search-mode-enabled")
            .bidirectional()
            .sync_create()
            .build();
        widgets.search_bar.connect_entry(&widgets.search_entry);

        for location in Location::SIDEBAR.into_iter().chain([Location::Add]) {
            let page = empty_state(location, &sender);
            widgets.stack.add_named(&page, Some(location.id()));
        }
        widgets.stack.set_visible_child_name(model.location.id());

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            ContentInput::Show(location) => self.location = location,
            ContentInput::SetCanGoBack(can) => self.can_go_back = can,
        }
    }

    fn post_view() {
        if stack.visible_child_name().as_deref() != Some(model.location.id()) {
            stack.set_visible_child_name(model.location.id());
        }
        if !model.location.has_search() {
            search_bar.set_search_mode(false);
        }
    }
}

impl ContentWidgets {
    pub fn details_toggle(&self) -> &gtk::ToggleButton {
        &self.details_toggle
    }

    /// Type to search: typing anywhere in `window` opens the search bar, in
    /// the views that have search.
    pub fn capture_typing(&self, window: Option<&impl IsA<gtk::Widget>>) {
        self.search_bar.set_key_capture_widget(window);
    }

    /// Ctrl+F: open the search bar, or close it if it is open.
    pub fn toggle_search(&self) {
        if self.search_toggle.is_visible() {
            self.search_toggle
                .set_active(!self.search_toggle.is_active());
        }
    }
}

fn empty_state(location: Location, sender: &ComponentSender<Content>) -> adw::StatusPage {
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
        Location::Add => (
            gettext("Search for Podcasts"),
            gettext("Find a show by name, or paste the address of its feed"),
        ),
    };
    let icon = match location {
        Location::Add => "system-search-symbolic",
        other => labels::icon(other),
    };
    let page = adw::StatusPage::builder()
        .icon_name(icon)
        .title(title)
        .description(description)
        .build();

    if location == Location::Shows {
        let add = gtk::Button::builder()
            .label(gettext("Add Podcast"))
            .halign(gtk::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .build();
        let sender = sender.clone();
        add.connect_clicked(move |_| {
            let _ = sender.output(ContentOutput::AddPodcast);
        });
        page.set_child(Some(&add));
    }
    page
}
