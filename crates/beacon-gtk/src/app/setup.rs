// SPDX-License-Identifier: GPL-3.0-or-later

//! Building the window: the three columns, the views wired to the window,
//! the window's actions, the library and the player.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use beacon_core::services::library::Library;
use gettextrs::gettext;
use gtk::{gio, glib};

use super::sender::AppSender;
use super::{
    App, AppMsg, AppWidgets, DETAILS_WIDTH, controls, library, media, playback, toast, window,
};
use crate::config::APP_ID;
use crate::navigation::Location;
use crate::ui::content::Content;
use crate::ui::details_pane::DetailsPane;
use crate::ui::dialogs::about;
use crate::ui::player_bar::{PlayerBar, PlayerBarInput};
use crate::ui::sidebar::{Sidebar, SidebarOutput};
use crate::ui::{errors, labels};

impl App {
    /// Build the window's contents into `root`, its views and actions
    /// reporting to `sender`.
    pub(super) fn init(root: &adw::ApplicationWindow, sender: &AppSender) -> Self {
        let sidebar = Sidebar::default();
        let s = sender.clone();
        sidebar.connect_output(move |output| {
            s.input(match output {
                SidebarOutput::Activated(location) => AppMsg::Navigate(location),
                SidebarOutput::AddPodcast => AppMsg::AddPodcast,
            });
        });
        let content = Content::default();
        let s = sender.clone();
        content.connect_output(move |output| s.input(AppMsg::Content(output)));
        let details = DetailsPane::default();
        let s = sender.clone();
        details.connect_output(move |output| s.input(AppMsg::Details(output)));
        let bar = PlayerBar::default();
        let s = sender.clone();
        bar.connect_output(move |output| s.input(AppMsg::Bar(output)));
        let refresh_action = add_actions(root, sender);

        let library = Library::open(
            &glib::user_data_dir().join("beacon"),
            &glib::user_cache_dir().join("beacon"),
        );
        let open_error = library.as_ref().err().map(errors::store);
        if let Err(err) = &library {
            tracing::error!("could not open the library: {err}");
        }

        let widgets = widgets(root, &sidebar, &content, &details);
        let mut model = App {
            widgets,
            sender: sender.clone(),
            settings: gio::Settings::new(APP_ID),
            library: library.ok().map(Arc::new),
            last_show: None,
            details_episode: None,
            source: None,
            details_source: None,
            after: None,
            up_next: None,
            requests: library::Requests::default(),
            has_shows: false,
            titles: HashMap::new(),
            subscribed_toast: None,
            refresh: refresh_action,
            details_origin: Rc::default(),
            player: None,
            player_error: None,
            media: media::Media::default(),
            frame: adw::ToolbarView::builder()
                .bottom_bar_style(adw::ToolbarStyle::RaisedBorder)
                .build(),
            sheet: adw::BottomSheet::builder()
                .can_open(false)
                .reveal_bottom_bar(false)
                .build(),
            now_playing: None,
            last_position: Rc::default(),
            sidebar,
            content,
            details,
            bar,
            sleep: controls::Sleep::default(),
            downloads: Rc::default(),
        };

        // The details toggle and the pane stay in step whichever side changes.
        model
            .content
            .widgets()
            .details_toggle()
            .bind_property("active", &model.widgets.details_split, "show-sidebar")
            .bidirectional()
            .sync_create()
            .build();

        model.build_frame(root);
        root.add_breakpoint(model.narrow_breakpoint());
        model.manage_focus();
        model.content.widgets().capture_typing(root);

        model
            .bar
            .send(PlayerBarInput::Volume(model.settings.double("volume")));
        model
            .bar
            .send(PlayerBarInput::Rate(beacon_core::playback::nearest_rate(
                model.settings.double("playback-rate"),
            )));
        match playback::start_player(&model.settings, sender) {
            Ok(player) => {
                model.player = Some(player);
                model.media.start(sender);
                playback::watch_network(sender);
            }
            Err(err) => {
                tracing::warn!("{err}");
                model.player_error = Some(err);
            }
        }

        model.restore_window_state(root);
        model.restore_play_source();
        let settings = model.settings.clone();
        let library = model.library.clone();
        let last_position = model.last_position.clone();
        root.connect_close_request(move |window| {
            window::save_window_state(&settings, window);
            playback::save_on_close(library.as_ref(), &last_position);
            glib::Propagation::Proceed
        });

        model.refresh.set_enabled(model.library.is_some());
        match open_error {
            Some(message) => toast(&model.widgets.toasts, &message),
            None => {
                model.reload(library::Parts::ALL);
                model.restore_last_episode();
                // New episodes since last time, as GNOME Podcasts looks for
                // them (`refresh-on-startup`).
                if model.settings.boolean("refresh-on-startup") {
                    model.start_refresh(true);
                }
                // Old downloads, if Preferences asks (`app/downloads.rs`).
                model.schedule_cleanup();
            }
        }

        model
    }
}

/// The window's actions: About, Preferences, Search, Back, Refresh Feeds
/// and the details pane. Returns Refresh Feeds, which is unavailable without a library.
fn add_actions(root: &adw::ApplicationWindow, sender: &AppSender) -> gio::SimpleAction {
    let window = root.downgrade();
    let about = gio::ActionEntry::builder("about")
        .activate(move |_: &adw::ApplicationWindow, _, _| {
            if let Some(window) = window.upgrade() {
                about::present(&window);
            }
        })
        .build();
    let message = |name: &str, message: fn() -> AppMsg| {
        let s = sender.clone();
        gio::ActionEntry::builder(name)
            .activate(move |_: &adw::ApplicationWindow, _, _| s.input(message()))
            .build()
    };
    root.add_action_entries([
        about,
        message("search", || AppMsg::ToggleSearch),
        message("back", || AppMsg::GoBack),
        message("details", || AppMsg::ToggleDetails),
        message("preferences", || AppMsg::Preferences),
    ]);
    let refresh = gio::SimpleAction::new("refresh", None);
    let s = sender.clone();
    refresh.connect_activate(move |_, _| s.input(AppMsg::Refresh));
    root.add_action(&refresh);
    refresh
}

/// The main view: sidebar, content and details. `build_frame` sets it into
/// the window's frame, inside the toasts.
fn widgets(
    root: &adw::ApplicationWindow,
    sidebar: &Sidebar,
    content: &Content,
    details: &DetailsPane,
) -> AppWidgets {
    root.set_title(Some(&gettext("Beacon")));
    root.set_width_request(360);
    root.set_height_request(294);
    let toasts = adw::ToastOverlay::new();

    let content_page = adw::NavigationPage::builder()
        .title(labels::title(Location::ALL[0]))
        .tag("content")
        .child(content)
        .build();
    let split_view = adw::NavigationSplitView::builder()
        .sidebar(
            &adw::NavigationPage::builder()
                .title(gettext("Beacon"))
                .tag("sidebar")
                .child(sidebar)
                .build(),
        )
        .content(&content_page)
        .build();
    let details_split = adw::OverlaySplitView::builder()
        .sidebar_position(gtk::PackType::End)
        .show_sidebar(false)
        .max_sidebar_width(DETAILS_WIDTH)
        .sidebar_width_fraction(0.3)
        .content(&split_view)
        .sidebar(details)
        .build();
    AppWidgets {
        toasts,
        details_split,
        split_view,
        content_page,
    }
}
