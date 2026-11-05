// SPDX-License-Identifier: GPL-3.0-or-later

//! The window: three columns (sidebar, content, details pane) over a
//! full-width player bar, toasts, and the narrow layout. `App` owns the
//! library; `library.rs` turns its data into view models and applies what
//! the views ask for.

mod actions;
mod controls;
mod library;
mod media;
mod playback;
mod window;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use beacon_core::model::{EpisodeId, ShowId};
use beacon_core::services::library::{Library, RefreshSummary};
use beacon_core::{LibraryError, PlaybackError, StoreError};
use gettextrs::gettext;
use relm4::actions::{RelmAction, RelmActionGroup};
use relm4::adw::prelude::*;
use relm4::gtk::{gio, glib};
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, adw, gtk,
};
use url::Url;

use crate::config::APP_ID;
use crate::navigation::Location;
use crate::player::mpris::MediaControl;
use crate::player::{Player, PlayerEvent};
use crate::ui::content::{Content, ContentInput, ContentOutput};
use crate::ui::details_pane::{DetailsPane, DetailsPaneInput, DetailsPaneOutput};
use crate::ui::dialogs::about;
use crate::ui::models::EpisodeItem;
use crate::ui::player_bar::{PlayerBar, PlayerBarInput, PlayerBarOutput};
use crate::ui::sidebar::{Sidebar, SidebarInput, SidebarOutput};
use crate::ui::{errors, labels};

/// Below this width the columns become pages.
const NARROW_BREAKPOINT_SP: f64 = 760.0;
/// Width of the details pane when it sits beside the content.
const DETAILS_WIDTH: f64 = 360.0;

relm4::new_action_group!(WindowActionGroup, "win");
relm4::new_stateless_action!(AboutAction, WindowActionGroup, "about");
relm4::new_stateless_action!(SearchAction, WindowActionGroup, "search");
relm4::new_stateless_action!(BackAction, WindowActionGroup, "back");
relm4::new_stateless_action!(RefreshAction, WindowActionGroup, "refresh");

#[derive(Debug)]
pub enum AppMsg {
    Navigate(Location),
    AddPodcast,
    /// What the details pane asks for.
    Details(DetailsPaneOutput),
    ToggleSearch,
    /// Alt+Left or the Back key, wherever the focus is.
    GoBack,
    Refresh,
    Content(ContentOutput),
    /// The user confirmed unsubscribing in the dialog.
    ConfirmUnsubscribe(ShowId),
    Player(PlayerEvent),
    /// What the player bar asks for.
    Bar(PlayerBarOutput),
    /// The sleep timer faded playback out and paused it.
    SleepTimerDone,
    /// What media keys or the desktop's media controls ask for.
    Media(MediaControl),
}

#[derive(Debug)]
pub enum AppCommand {
    Loaded(Box<Result<library::Loaded, StoreError>>),
    Subscribed(Url, Result<ShowId, LibraryError>),
    Refreshed(RefreshSummary),
    /// An episode's own cover, fetched for the details pane and the bar.
    EpisodeCover(EpisodeId, Option<PathBuf>),
    /// The episode Next Episode found, if any.
    PlayNext(Option<Box<EpisodeItem>>),
    /// An episode's chapters, loaded for the details pane.
    Chapters(EpisodeId, Vec<beacon_core::model::Chapter>),
}

pub struct App {
    settings: gio::Settings,
    library: Option<Arc<Library>>,
    /// The podcast page last opened, refreshed with the rest when data
    /// changes. Harmless if the page has since been closed.
    last_show: Option<ShowId>,
    /// The episode in the details pane, and its show.
    details_episode: Option<(EpisodeId, ShowId)>,
    /// The latest background request for each part of the views.
    requests: library::Requests,
    has_shows: bool,
    /// Show titles as last loaded, for dialogs and toasts.
    titles: HashMap<ShowId, String>,
    /// A subscription whose toast waits for the show's title.
    subscribed_toast: Option<ShowId>,
    refresh: gio::SimpleAction,
    /// Plays episodes; `None` when the audio system could not start, with
    /// the reason in `player_error`.
    player: Option<Rc<Player>>,
    player_error: Option<PlaybackError>,
    /// Media keys and the desktop's media controls (MPRIS).
    media: media::Media,
    now_playing: Option<playback::NowPlaying>,
    last_position: playback::LastPosition,
    /// Where focus was when a row opened the details pane, to return to
    /// when the pane closes.
    details_origin: Rc<RefCell<Option<glib::WeakRef<gtk::Widget>>>>,
    sidebar: Controller<Sidebar>,
    content: Controller<Content>,
    details: Controller<DetailsPane>,
    bar: Controller<PlayerBar>,
    sleep: controls::Sleep,
}

#[relm4::component(pub)]
impl Component for App {
    type Init = ();
    type Input = AppMsg;
    type Output = ();
    type CommandOutput = AppCommand;

    view! {
        main_window = adw::ApplicationWindow {
            set_title: Some(&gettext("Beacon")),
            set_width_request: 360,
            set_height_request: 294,

            #[name = "toasts"]
            adw::ToastOverlay {
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
                                set_title: &labels::title(Location::ALL[0]),
                                set_tag: Some("content"),
                                set_child: Some(model.content.widget()),
                            },
                        },

                        #[wrap(Some)]
                        set_sidebar = model.details.widget(),
                    },

                    add_bottom_bar = model.bar.widget(),
                },
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
            .forward(sender.input_sender(), AppMsg::Content);
        let details = DetailsPane::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::Details);
        let bar = PlayerBar::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::Bar);

        let mut actions = RelmActionGroup::<WindowActionGroup>::new();
        let window = root.clone();
        actions.add_action(RelmAction::<AboutAction>::new_stateless(move |_| {
            about::present(&window);
        }));
        let s = sender.clone();
        actions.add_action(RelmAction::<SearchAction>::new_stateless(move |_| {
            s.input(AppMsg::ToggleSearch);
        }));
        let s = sender.clone();
        actions.add_action(RelmAction::<BackAction>::new_stateless(move |_| {
            s.input(AppMsg::GoBack);
        }));
        let s = sender.clone();
        let refresh = RelmAction::<RefreshAction>::new_stateless(move |_| {
            s.input(AppMsg::Refresh);
        });
        let refresh_action = refresh.gio_action().clone();
        actions.add_action(refresh);
        actions.register_for_widget(&root);

        let library = Library::open(
            &glib::user_data_dir().join("beacon"),
            &glib::user_cache_dir().join("beacon"),
        );
        let open_error = library.as_ref().err().map(errors::store);
        if let Err(err) = &library {
            tracing::error!("could not open the library: {err}");
        }

        let mut model = App {
            settings: gio::Settings::new(APP_ID),
            library: library.ok().map(Arc::new),
            last_show: None,
            details_episode: None,
            requests: library::Requests::default(),
            has_shows: false,
            titles: HashMap::new(),
            subscribed_toast: None,
            refresh: refresh_action,
            details_origin: Rc::default(),
            player: None,
            player_error: None,
            media: media::Media::default(),
            now_playing: None,
            last_position: Rc::default(),
            sidebar,
            content,
            details,
            bar,
            sleep: controls::Sleep::default(),
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
        model.manage_focus(&widgets);
        model.content.widgets().capture_typing(&root);

        model
            .bar
            .emit(PlayerBarInput::Volume(model.settings.double("volume")));
        model
            .bar
            .emit(PlayerBarInput::Rate(beacon_core::playback::nearest_rate(
                model.settings.double("playback-rate"),
            )));
        match playback::start_player(&model.settings, &sender) {
            Ok(player) => {
                model.player = Some(player);
                model.media.start(&sender);
            }
            Err(err) => {
                tracing::warn!("{err}");
                model.player_error = Some(err);
            }
        }

        model.restore_window_state(&root);
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
            Some(message) => toast(&widgets.toasts, &message),
            None => model.reload(&sender, library::Parts::ALL),
        }

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        tracing::debug!(?message, "app message");
        match message {
            AppMsg::Navigate(location) => {
                widgets.content_page.set_title(&labels::title(location));
                self.content.emit(ContentInput::ShowSection(location));
                self.sidebar.emit(SidebarInput::Select(location));
                // In the narrow layout, choosing anything pushes the content.
                widgets.split_view.set_show_content(true);
            }
            AppMsg::AddPodcast => {
                self.content.emit(ContentInput::OpenAdd);
                widgets.split_view.set_show_content(true);
            }
            AppMsg::Details(output) => self.details_output(output, widgets, &sender),
            AppMsg::GoBack => self.go_back(widgets),
            AppMsg::ToggleSearch => self.content.widgets().toggle_search(),
            AppMsg::Refresh => self.start_refresh(&sender),
            AppMsg::Content(output) => self.handle_content(output, widgets, &sender, root),
            AppMsg::ConfirmUnsubscribe(id) => self.unsubscribe(id, widgets, &sender),
            AppMsg::Player(event) => self.player_event(event, widgets, &sender),
            AppMsg::Bar(output) => self.bar_output(output, widgets, &sender),
            AppMsg::Media(control) => self.media_control(control, widgets, &sender, root),
            AppMsg::SleepTimerDone => self.end_sleep_timer(),
        }
    }

    fn update_cmd_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            AppCommand::Loaded(result) => self.loaded(*result, widgets),
            AppCommand::Subscribed(url, result) => {
                self.subscribed(&url, result, widgets, &sender);
            }
            AppCommand::Refreshed(summary) => self.refreshed(&summary, widgets, &sender),
            AppCommand::EpisodeCover(id, Some(path)) => self.episode_cover_arrived(id, path),
            AppCommand::EpisodeCover(_, None) | AppCommand::PlayNext(None) => {}
            AppCommand::PlayNext(Some(item)) => self.play_episode(&item, widgets, &sender),
            AppCommand::Chapters(id, chapters) => {
                self.details.emit(DetailsPaneInput::Chapters(id, chapters));
            }
        }
    }
}

/// Show a short message at the bottom of the window.
fn toast(overlay: &adw::ToastOverlay, message: &str) {
    overlay.add_toast(adw::Toast::new(message));
}
