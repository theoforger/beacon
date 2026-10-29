// SPDX-License-Identifier: GPL-3.0-or-later

//! The window: three columns (sidebar, content, details pane) over a
//! full-width player bar, the navigation trail, and the narrow layout.

use gettextrs::gettext;
use relm4::actions::{RelmAction, RelmActionGroup};
use relm4::adw::prelude::*;
use relm4::gtk::{gio, glib};
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, adw, gtk,
};

use crate::config::APP_ID;
use crate::navigation::{Location, Trail};
use crate::ui::content::{Content, ContentInput, ContentOutput};
use crate::ui::details_pane::{DetailsPane, DetailsPaneOutput};
use crate::ui::dialogs::about;
use crate::ui::labels;
use crate::ui::player_bar::PlayerBar;
use crate::ui::sidebar::{Sidebar, SidebarInput, SidebarOutput};

/// Below this width the columns become pages.
const NARROW_BREAKPOINT_SP: f64 = 760.0;
/// Width of the details pane when it sits beside the content.
const DETAILS_WIDTH: f64 = 360.0;

relm4::new_action_group!(WindowActionGroup, "win");
relm4::new_stateless_action!(AboutAction, WindowActionGroup, "about");
relm4::new_stateless_action!(SearchAction, WindowActionGroup, "search");

#[derive(Debug)]
pub enum AppMsg {
    Navigate(Location),
    AddPodcast,
    Back,
    CloseDetails,
    ToggleSearch,
}

pub struct App {
    settings: gio::Settings,
    trail: Trail,
    sidebar: Controller<Sidebar>,
    content: Controller<Content>,
    details: Controller<DetailsPane>,
    _player: Controller<PlayerBar>,
}

#[relm4::component(pub)]
impl Component for App {
    type Init = ();
    type Input = AppMsg;
    type Output = ();
    type CommandOutput = ();

    view! {
        main_window = adw::ApplicationWindow {
            set_title: Some(&gettext("Beacon")),
            set_width_request: 360,
            set_height_request: 294,

            adw::ToolbarView {
                set_bottom_bar_style: adw::ToolbarStyle::RaisedBorder,

                #[wrap(Some)]
                #[name = "details_split"]
                set_content = &adw::OverlaySplitView {
                    set_sidebar_position: gtk::PackType::End,
                    set_show_sidebar: false,
                    set_max_sidebar_width: DETAILS_WIDTH,
                    set_sidebar_width_fraction: 0.3,

                    #[wrap(Some)]
                    #[name = "split_view"]
                    set_content = &adw::NavigationSplitView {
                        #[wrap(Some)]
                        set_sidebar = &adw::NavigationPage {
                            set_title: &gettext("Beacon"),
                            set_tag: Some("sidebar"),
                            set_child: Some(model.sidebar.widget()),
                        },

                        #[wrap(Some)]
                        #[name = "content_page"]
                        set_content = &adw::NavigationPage {
                            set_title: &labels::view_title(model.trail.view()),
                            set_tag: Some("content"),
                            set_child: Some(model.content.widget()),
                        },
                    },

                    #[wrap(Some)]
                    set_sidebar = model.details.widget(),
                },

                add_bottom_bar = model._player.widget(),
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let sidebar = Sidebar::builder()
            .launch(())
            .forward(sender.input_sender(), |output| match output {
                SidebarOutput::Activated(location) => AppMsg::Navigate(location),
                SidebarOutput::AddPodcast => AppMsg::AddPodcast,
            });
        let content = Content::builder()
            .launch(())
            .forward(sender.input_sender(), |output| match output {
                ContentOutput::Back => AppMsg::Back,
                ContentOutput::AddPodcast => AppMsg::AddPodcast,
            });
        let details = DetailsPane::builder()
            .launch(())
            .forward(sender.input_sender(), |DetailsPaneOutput::Close| {
                AppMsg::CloseDetails
            });
        let player = PlayerBar::builder().launch(()).detach();

        let model = App {
            settings: gio::Settings::new(APP_ID),
            trail: Trail::default(),
            sidebar,
            content,
            details,
            _player: player,
        };
        let widgets = view_output!();

        // The details toggle and the pane stay in step whichever side changes.
        model
            .content
            .widgets()
            .details_toggle()
            .bind_property("active", &widgets.details_split, "show-sidebar")
            .bidirectional()
            .sync_create()
            .build();

        root.add_breakpoint(model.narrow_breakpoint(&widgets));
        model.content.widgets().capture_typing(Some(&root));

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
        let s = sender.clone();
        actions.add_action(RelmAction::<SearchAction>::new_stateless(move |_| {
            s.input(AppMsg::ToggleSearch);
        }));
        actions.register_for_widget(&root);

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        _sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        match message {
            AppMsg::Navigate(location) => {
                if self.trail.go(location) {
                    self.show_current(widgets, root);
                }
                // In the narrow layout, choosing anything pushes the content.
                widgets.split_view.set_show_content(true);
            }
            AppMsg::AddPodcast => {
                if self.trail.open_add() {
                    self.show_current(widgets, root);
                }
                widgets.split_view.set_show_content(true);
            }
            AppMsg::Back => {
                if self.trail.back() {
                    self.show_current(widgets, root);
                }
            }
            AppMsg::CloseDetails => widgets.details_split.set_show_sidebar(false),
            AppMsg::ToggleSearch => self.content.widgets().toggle_search(),
        }
    }
}

impl App {
    fn show_current(&self, widgets: &AppWidgets, window: &adw::ApplicationWindow) {
        let view = self.trail.view();
        widgets.content_page.set_title(&labels::view_title(view));
        self.content
            .widgets()
            .capture_typing(view.has_search().then_some(window));
        self.content.emit(ContentInput::Show(view));
        self.content
            .emit(ContentInput::SetCanGoBack(self.trail.can_go_back()));
        self.sidebar
            .emit(SidebarInput::Select(self.trail.section()));
    }

    /// Narrow windows: the sidebar becomes a page with the content pushed on
    /// top, and the details pane covers the window with its own Back. It
    /// starts closed and its header toggle is hidden.
    fn narrow_breakpoint(&self, widgets: &AppWidgets) -> adw::Breakpoint {
        let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
            adw::BreakpointConditionLengthType::MaxWidth,
            NARROW_BREAKPOINT_SP,
            adw::LengthUnit::Sp,
        ));
        breakpoint.add_setter(&widgets.split_view, "collapsed", Some(&true.to_value()));
        breakpoint.add_setter(&widgets.details_split, "collapsed", Some(&true.to_value()));
        breakpoint.add_setter(
            &widgets.details_split,
            "show-sidebar",
            Some(&false.to_value()),
        );
        breakpoint.add_setter(
            &widgets.details_split,
            "max-sidebar-width",
            Some(&f64::MAX.to_value()),
        );
        breakpoint.add_setter(
            &widgets.details_split,
            "sidebar-width-fraction",
            Some(&1.0f64.to_value()),
        );
        breakpoint.add_setter(
            self.content.widgets().details_toggle(),
            "visible",
            Some(&false.to_value()),
        );
        breakpoint.add_setter(
            self.details.widgets().back_button(),
            "visible",
            Some(&true.to_value()),
        );
        breakpoint.add_setter(
            self.sidebar.widgets().library(),
            "mode",
            Some(&adw::SidebarMode::Page.to_value()),
        );
        breakpoint
    }

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
