// SPDX-License-Identifier: GPL-3.0-or-later

//! The sidebar: its own header (Add Podcast, title, Main Menu) and the
//! library items with count badges.

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::navigation::Location;
use crate::ui::labels;
use crate::ui::messages::Output;

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

mod imp {
    use std::cell::{OnceCell, RefCell};

    use super::*;

    #[derive(Default)]
    pub struct Sidebar {
        pub library: OnceCell<adw::Sidebar>,
        pub badges: RefCell<Vec<(Location, gtk::Label)>>,
        pub output: Output<SidebarOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Sidebar {
        const NAME: &'static str = "BeaconSidebar";
        type Type = super::Sidebar;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for Sidebar {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }
    impl WidgetImpl for Sidebar {}
    impl BinImpl for Sidebar {}
}

glib::wrapper! {
    pub struct Sidebar(ObjectSubclass<imp::Sidebar>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for Sidebar {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Sidebar {
    fn build(&self) {
        let output = self.imp().output.clone();
        let add = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text(gettext("Add Podcast"))
            .build();
        add.update_property(&[gtk::accessible::Property::Label(&gettext("Add Podcast"))]);
        add.connect_clicked({
            let output = output.clone();
            move |_| output.emit(SidebarOutput::AddPodcast)
        });
        let menu = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&primary_menu())
            .primary(true)
            .tooltip_text(gettext("Main Menu"))
            .build();
        menu.update_property(&[gtk::accessible::Property::Label(&gettext("Main Menu"))]);
        let header = adw::HeaderBar::new();
        header.pack_start(&add);
        header.pack_end(&menu);

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
        let library = adw::Sidebar::new();
        library.append(section);
        library.connect_activated(move |_, index| {
            if let Some(&location) = Location::ALL.get(index as usize) {
                output.emit(SidebarOutput::Activated(location));
            }
        });

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&library));
        self.set_child(Some(&toolbar));

        let imp = self.imp();
        imp.badges.replace(badges);
        let _ = imp.library.set(library);
        self.select(Location::Shows);
    }

    pub fn connect_output(&self, receive: impl Fn(SidebarOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    pub fn send(&self, input: SidebarInput) {
        match input {
            SidebarInput::Select(location) => self.select(location),
            SidebarInput::SetCounts(counts) => {
                for (location, badge) in self.imp().badges.borrow().iter() {
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

    fn select(&self, location: Location) {
        let index = Location::ALL.iter().position(|&l| l == location);
        self.library()
            .set_selected(index.map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
    }

    /// Follows the window's narrow layout.
    pub fn library(&self) -> &adw::Sidebar {
        self.imp().library.get().expect("built when constructed")
    }
}

fn primary_menu() -> gio::Menu {
    // Actions that do not exist yet leave their items disabled until the
    // feature lands: refresh in M3, import and export in M8, Preferences in M5.
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
