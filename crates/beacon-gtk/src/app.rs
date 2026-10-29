// SPDX-License-Identifier: GPL-3.0-or-later

use gettextrs::gettext;
use relm4::actions::{RelmAction, RelmActionGroup};
use relm4::adw::prelude::*;
use relm4::gtk::{gio, glib};
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};

use crate::config::APP_ID;
use crate::ui::dialogs::about;

/// The sidebar and content collapse into pages below this width.
const NARROW_BREAKPOINT_SP: f64 = 760.0;

relm4::new_action_group!(WindowActionGroup, "win");
relm4::new_stateless_action!(AboutAction, WindowActionGroup, "about");

pub struct App {
    settings: gio::Settings,
}

#[relm4::component(pub)]
impl SimpleComponent for App {
    type Init = ();
    type Input = ();
    type Output = ();

    view! {
        main_window = adw::ApplicationWindow {
            set_title: Some(&gettext("Beacon")),
            set_width_request: 360,
            set_height_request: 294,

            #[name = "split_view"]
            adw::NavigationSplitView {
                #[wrap(Some)]
                set_sidebar = &adw::NavigationPage {
                    set_title: &gettext("Beacon"),
                    set_tag: Some("sidebar"),

                    #[wrap(Some)]
                    set_child = &adw::ToolbarView {
                        add_top_bar = &adw::HeaderBar {
                            pack_end = &gtk::MenuButton {
                                set_icon_name: "open-menu-symbolic",
                                set_menu_model: Some(&primary_menu()),
                                set_primary: true,
                                set_tooltip_text: Some(&gettext("Main Menu")),
                                update_property: &[gtk::accessible::Property::Label(&gettext("Main Menu"))],
                            },
                        },
                    },
                },

                #[wrap(Some)]
                set_content = &adw::NavigationPage {
                    set_title: &gettext("Shows"),
                    set_tag: Some("content"),

                    #[wrap(Some)]
                    set_child = &adw::ToolbarView {
                        add_top_bar = &adw::HeaderBar {},

                        #[wrap(Some)]
                        set_content = &adw::StatusPage {
                            set_icon_name: Some("com.theoforger.Beacon-symbolic"),
                            set_title: &gettext("No Podcasts"),
                            set_description: Some(&gettext("Subscribed podcasts will appear here")),
                        },
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let settings = gio::Settings::new(APP_ID);
        let model = App { settings };
        let widgets = view_output!();

        let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
            adw::BreakpointConditionLengthType::MaxWidth,
            NARROW_BREAKPOINT_SP,
            adw::LengthUnit::Sp,
        ));
        breakpoint.add_setter(&widgets.split_view, "collapsed", Some(&true.to_value()));
        root.add_breakpoint(breakpoint);

        model.restore_window_state(&root);
        let settings = model.settings.clone();
        root.connect_close_request(move |window| {
            save_window_state(&settings, window);
            glib::Propagation::Proceed
        });

        let mut actions = RelmActionGroup::<WindowActionGroup>::new();
        let window = root.clone();
        actions.add_action(RelmAction::<AboutAction>::new_stateless(move |_| {
            about::present(&window);
        }));
        actions.register_for_widget(&root);

        ComponentParts { model, widgets }
    }
}

impl App {
    fn restore_window_state(&self, window: &adw::ApplicationWindow) {
        window.set_default_size(
            self.settings.int("window-width"),
            self.settings.int("window-height"),
        );
        if self.settings.boolean("is-maximized") {
            window.maximize();
        }
    }
}

fn save_window_state(settings: &gio::Settings, window: &adw::ApplicationWindow) {
    let (width, height) = window.default_size();
    let result = settings
        .set_int("window-width", width)
        .and_then(|()| settings.set_int("window-height", height))
        .and_then(|()| settings.set_boolean("is-maximized", window.is_maximized()));
    if let Err(err) = result {
        tracing::warn!("could not save the window state: {err}");
    }
}

fn primary_menu() -> gio::Menu {
    let section = gio::Menu::new();
    section.append(Some(&gettext("_Keyboard Shortcuts")), Some("app.shortcuts"));
    section.append(Some(&gettext("_About Beacon")), Some("win.about"));

    let menu = gio::Menu::new();
    menu.append_section(None, &section);
    menu
}
