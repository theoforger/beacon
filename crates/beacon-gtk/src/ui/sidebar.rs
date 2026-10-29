// SPDX-License-Identifier: GPL-3.0-or-later

//! The sidebar: its own header (Add Podcast, title, Main Menu) and the
//! library items with count badges.

use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::gtk::gio;
use relm4::{Component, ComponentParts, ComponentSender, adw, gtk};

use crate::navigation::Location;
use crate::ui::labels;

/// Numbers shown beside sidebar items. Zero hides the badge.
#[derive(Debug, Default, Clone, Copy)]
pub struct Counts {
    /// Shows with new episodes.
    pub shows: u32,
    /// New episodes.
    pub new: u32,
    /// Queue length.
    pub queue: u32,
}

#[derive(Debug)]
pub enum SidebarInput {
    /// Reflect the current location without emitting anything.
    Select(Location),
    SetCounts(Counts),
}

#[derive(Debug)]
pub enum SidebarOutput {
    Activated(Location),
    AddPodcast,
}

pub struct Sidebar {
    badges: Vec<(Location, gtk::Label)>,
}

#[relm4::component(pub)]
impl Component for Sidebar {
    type Init = ();
    type Input = SidebarInput;
    type Output = SidebarOutput;
    type CommandOutput = ();

    view! {
        adw::ToolbarView {
            add_top_bar = &adw::HeaderBar {
                pack_start = &gtk::Button {
                    set_icon_name: "list-add-symbolic",
                    set_tooltip_text: Some(&gettext("Add Podcast")),
                    update_property: &[gtk::accessible::Property::Label(&gettext("Add Podcast"))],
                    connect_clicked[sender] => move |_| {
                        let _ = sender.output(SidebarOutput::AddPodcast);
                    },
                },
                pack_end = &gtk::MenuButton {
                    set_icon_name: "open-menu-symbolic",
                    set_menu_model: Some(&primary_menu()),
                    set_primary: true,
                    set_tooltip_text: Some(&gettext("Main Menu")),
                    update_property: &[gtk::accessible::Property::Label(&gettext("Main Menu"))],
                },
            },

            #[wrap(Some)]
            #[name = "library"]
            set_content = &adw::Sidebar {},
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();

        let section = adw::SidebarSection::new();
        let mut badges = Vec::new();
        for location in Location::ALL {
            let badge = gtk::Label::builder()
                .css_classes(["dim-label", "numeric"])
                .visible(false)
                .build();
            section.append(
                adw::SidebarItem::builder()
                    .title(labels::title(location))
                    .icon_name(labels::icon(location))
                    .suffix(&badge)
                    .build(),
            );
            badges.push((location, badge));
        }
        widgets.library.append(section);

        widgets.library.connect_activated(move |_, index| {
            if let Some(&location) = Location::ALL.get(index as usize) {
                let _ = sender.output(SidebarOutput::Activated(location));
            }
        });

        let model = Sidebar { badges };
        model.select(&widgets, Location::Shows);
        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            SidebarInput::Select(location) => self.select(widgets, location),
            SidebarInput::SetCounts(counts) => {
                for (location, badge) in &self.badges {
                    let count = match location {
                        Location::Shows => counts.shows,
                        Location::New => counts.new,
                        Location::Queue => counts.queue,
                        _ => 0,
                    };
                    badge.set_label(&count.to_string());
                    badge.set_visible(count > 0);
                }
            }
        }
    }
}

impl Sidebar {
    fn select(&self, widgets: &SidebarWidgets, location: Location) {
        let index = Location::ALL.iter().position(|&l| l == location);
        widgets
            .library
            .set_selected(index.map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
    }
}

impl SidebarWidgets {
    /// Follows the window's narrow layout.
    pub fn library(&self) -> &adw::Sidebar {
        &self.library
    }
}

fn primary_menu() -> gio::Menu {
    // Actions that do not exist yet leave their items disabled until the
    // feature lands: refresh in M3, import and export in M7, Preferences in M5.
    let files = gio::Menu::new();
    files.append(Some(&gettext("_Refresh Feeds")), Some("win.refresh"));
    files.append(Some(&gettext("_Import Subscriptions…")), Some("win.import"));
    files.append(Some(&gettext("_Export Subscriptions…")), Some("win.export"));

    let app = gio::Menu::new();
    app.append(Some(&gettext("_Preferences")), Some("win.preferences"));
    app.append(Some(&gettext("_Keyboard Shortcuts")), Some("app.shortcuts"));
    app.append(Some(&gettext("_About Beacon")), Some("win.about"));

    let menu = gio::Menu::new();
    menu.append_section(None, &files);
    menu.append_section(None, &app);
    menu
}
