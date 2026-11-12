// SPDX-License-Identifier: GPL-3.0-or-later

//! The window: three columns (sidebar, content, details pane) over a
//! full-width player bar, toasts, and the narrow layout. `App` owns the
//! library; `library.rs` turns its data into view models and applies what
//! the views ask for.

mod actions;
mod controls;
mod directory;
pub(crate) mod downloads;
mod library;
mod lists;
mod media;
mod object;
mod playback;
mod sender;
mod setup;
mod sync;
mod window;

pub use object::Window;
use sender::AppSender;
#[cfg(test)]
pub(crate) use setup::details_split;
#[cfg(test)]
pub(crate) use window::details_breakpoint;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use beacon_core::model::{EpisodeId, ShowId};
use beacon_core::queue::Source;
use beacon_core::services::library::{Library, RefreshSummary};
use beacon_core::{LibraryError, PlaybackError, StoreError};
use gtk::{gio, glib};
use url::Url;

use crate::navigation::Location;
use crate::player::mpris::MediaControl;
use crate::player::{Player, PlayerEvent};
use crate::ui::content::{Content, ContentInput, ContentOutput};
use crate::ui::details_pane::{DetailsPane, DetailsPaneInput, DetailsPaneOutput};
use crate::ui::labels;
use crate::ui::models::EpisodeItem;
use crate::ui::player_bar::{PlayerBar, PlayerBarInput, PlayerBarOutput};
use crate::ui::sidebar::{Sidebar, SidebarInput};

/// Below this width the columns become pages.
const NARROW_BREAKPOINT_SP: f64 = 760.0;
/// Width of the details pane when it sits beside the content.
const DETAILS_WIDTH: f64 = 360.0;

#[derive(Debug)]
pub enum AppMsg {
    Navigate(Location),
    AddPodcast,
    /// What the details pane asks for.
    Details(DetailsPaneOutput),
    ToggleSearch,
    /// F9: open or close the details pane, as its header toggle does.
    ToggleDetails,
    /// Alt+Left or the Back key, wherever the focus is.
    GoBack,
    Refresh,
    Content(ContentOutput),
    /// The user confirmed unsubscribing in the dialog.
    ConfirmUnsubscribe(ShowId),
    /// Undo of Clear Queue.
    Requeue(Vec<EpisodeId>),
    /// Delete All Downloads, confirmed.
    DeleteDownloads(Vec<EpisodeId>),
    /// Download these episodes (Download All Episodes, confirmed).
    Download(Vec<EpisodeId>),
    /// Look for old downloads to delete (at startup and once a day).
    CleanDownloads,
    /// Open Preferences.
    Preferences,
    Player(PlayerEvent),
    /// What the player bar asks for.
    Bar(PlayerBarOutput),
    /// The sleep timer faded playback out and paused it.
    SleepTimerDone,
    /// What media keys or the desktop's media controls ask for.
    Media(MediaControl),
    /// The connection came or went, or became metered or not.
    NetworkChanged,
    Sync(sync::SyncMsg),
}

#[derive(Debug)]
pub enum AppCommand {
    Loaded(Box<Result<library::Loaded, StoreError>>),
    Subscribed(Url, Result<ShowId, LibraryError>),
    /// A refresh finished; `true` for the one at startup.
    Refreshed(RefreshSummary, bool),
    /// An episode's own cover, fetched for the details pane and the bar.
    EpisodeCover(EpisodeId, Option<PathBuf>),
    /// The episode Next Episode found, if any.
    PlayNext(Option<Box<EpisodeItem>>),
    /// An episode's chapters, loaded for the details pane and the player
    /// bar.
    Chapters(EpisodeId, Vec<beacon_core::model::Chapter>),
    DownloadsDeleted(Result<usize, StoreError>),
    /// A download ended: done, failed or cancelled, with the episode's
    /// title when it failed.
    Downloaded(
        EpisodeId,
        String,
        Result<PathBuf, beacon_core::DownloadError>,
    ),
    /// Old downloads deleted, as Preferences asks.
    Cleaned(Result<usize, StoreError>),
    /// The episode last played, read back at launch.
    Restore(Option<Box<EpisodeItem>>),
    Sync(Box<sync::SyncCommand>),
    /// Apple's directory answered the request with this key.
    Directory(
        String,
        Result<Vec<crate::ui::directory_row::DirectoryItem>, beacon_core::ItunesError>,
    ),
    /// A directory result's thumbnail, if it could be fetched.
    DirectoryArt(Url, Option<PathBuf>),
    /// A directory result's feed, fetched for its preview.
    Preview(
        Url,
        Box<Result<beacon_core::services::library::FeedPreview, LibraryError>>,
    ),
}

/// The window's parts the app reaches into.
pub struct AppWidgets {
    toasts: adw::ToastOverlay,
    /// The details pane beside, or in the narrow layout over, the rest.
    details_split: adw::OverlaySplitView,
    /// The sidebar and the content.
    split_view: adw::NavigationSplitView,
    content_page: adw::NavigationPage,
}

pub struct App {
    widgets: AppWidgets,
    /// Where views, the player and background work report to the window.
    sender: AppSender,
    settings: gio::Settings,
    library: Option<Arc<Library>>,
    /// The podcast page last opened, refreshed with the rest when data
    /// changes. Harmless if the page has since been closed.
    last_show: Option<ShowId>,
    /// The episode in the details pane, and its show.
    details_episode: Option<(EpisodeId, ShowId)>,
    /// The list the playing episode was started from, whose rest plays
    /// after the queue (`beacon_core::queue`), and the list the details
    /// pane's episode was opened from, for its Play.
    source: Option<Source>,
    details_source: Option<Source>,
    /// The episode the list carries on after: the playing one, or else
    /// the last played. It and `source` are kept in settings, so the
    /// Queue's Next from survives a restart (found missing by the user).
    after: Option<EpisodeId>,
    /// What plays after the playing episode, as last loaded; preloaded
    /// once the playing one has buffered.
    up_next: Option<beacon_core::model::Episode>,
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
    /// The player bar under the main view, in wide windows.
    frame: adw::ToolbarView,
    /// Around everything: in narrow windows it holds the player, opened
    /// from the slim bar.
    sheet: adw::BottomSheet,
    now_playing: Option<playback::NowPlaying>,
    last_position: playback::LastPosition,
    /// Where focus was when a row opened the details pane, to return to
    /// when the pane closes.
    details_origin: Rc<RefCell<Option<glib::WeakRef<gtk::Widget>>>>,
    sidebar: Sidebar,
    content: Content,
    details: DetailsPane,
    bar: PlayerBar,
    sleep: controls::Sleep,
    /// The downloads under way.
    downloads: downloads::Active,
    /// The sync account and the syncs it runs.
    sync: sync::Sync,
    /// Apple's directory, for the Add page.
    directory: directory::Directory,
    /// The search showing: of a section's own list, with the section
    /// (`search`), or of the podcast page (`show_search`).
    search: Option<(String, crate::navigation::Location)>,
    show_search: Option<String>,
}

impl App {
    /// Handle a message from a view, the player or the desktop.
    pub(super) fn update(&mut self, message: AppMsg, root: &adw::ApplicationWindow) {
        tracing::debug!(?message, "app message");
        match message {
            AppMsg::Navigate(location) => {
                self.widgets
                    .content_page
                    .set_title(&labels::title(location));
                self.content.send(ContentInput::ShowSection(location));
                self.sidebar.send(SidebarInput::Select(location));
                // In the narrow layout, choosing anything pushes the content.
                self.widgets.split_view.set_show_content(true);
            }
            AppMsg::AddPodcast => {
                self.content.send(ContentInput::OpenAdd);
                self.widgets.split_view.set_show_content(true);
            }
            AppMsg::Details(output) => self.details_output(output),
            AppMsg::GoBack => self.go_back(),
            AppMsg::ToggleSearch => self.content.toggle_search(),
            // The HIG's Utility Panes: "If utility pane visibility can be
            // toggled, assign the F9 key as a shortcut." The header toggle
            // follows, bound to the pane.
            AppMsg::ToggleDetails => self
                .widgets
                .details_split
                .set_show_sidebar(!self.widgets.details_split.shows_sidebar()),
            AppMsg::Refresh => self.start_refresh(false),
            AppMsg::Content(output) => self.handle_content(output, root),
            AppMsg::ConfirmUnsubscribe(id) => self.unsubscribe(id),
            AppMsg::Requeue(ids) => self.requeue(ids),
            AppMsg::DeleteDownloads(ids) => self.delete_downloads(ids),
            AppMsg::Download(ids) => self.start_downloads(ids),
            AppMsg::CleanDownloads => self.clean_downloads(),
            AppMsg::Preferences => self.open_preferences(root),
            AppMsg::Player(event) => self.player_event(event),
            AppMsg::Bar(output) => self.bar_output(output),
            AppMsg::Media(control) => self.media_control(control, root),
            AppMsg::SleepTimerDone => self.end_sleep_timer(),
            AppMsg::NetworkChanged => {
                self.buffer_restored();
                self.preload_next();
            }
            AppMsg::Sync(message) => self.sync_message(message),
        }
    }

    /// Apply the result of work done off the main thread.
    pub(super) fn update_cmd(&mut self, message: AppCommand) {
        match message {
            AppCommand::Loaded(result) => self.loaded(*result),
            AppCommand::Subscribed(url, result) => {
                self.subscribed(&url, result);
            }
            AppCommand::Refreshed(summary, at_startup) => {
                self.refreshed(&summary, at_startup);
            }
            AppCommand::EpisodeCover(id, Some(path)) => self.episode_cover_arrived(id, path),
            AppCommand::EpisodeCover(_, None)
            | AppCommand::PlayNext(None)
            | AppCommand::Restore(None) => {}
            AppCommand::Restore(Some(item)) => self.restore(*item),
            AppCommand::PlayNext(Some(item)) => self.play_episode(&item),
            AppCommand::DownloadsDeleted(result) => self.downloads_deleted(result),
            AppCommand::Downloaded(id, title, result) => self.downloaded(id, &title, result),
            AppCommand::Cleaned(result) => self.cleaned(result),
            AppCommand::Sync(command) => self.sync_command(*command),
            AppCommand::Directory(key, result) => self.directory_answered(key, result),
            AppCommand::DirectoryArt(url, Some(path)) => {
                self.content.send(ContentInput::DirectoryArt(url, path));
            }
            AppCommand::DirectoryArt(_, None) => {}
            AppCommand::Preview(url, result) => self.previewed(url, *result),
            AppCommand::Chapters(id, chapters) => {
                self.bar
                    .send(PlayerBarInput::Chapters(id, chapters.clone()));
                self.details.send(DetailsPaneInput::Chapters(id, chapters));
            }
        }
    }
}

/// Show a short message at the bottom of the window.
fn toast(overlay: &adw::ToastOverlay, message: &str) {
    overlay.add_toast(adw::Toast::new(message));
}
