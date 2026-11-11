// SPDX-License-Identifier: GPL-3.0-or-later

//! What the content pane takes from the window, and reports to it.

use beacon_core::browse::RecencyGroup;
use beacon_core::model::{EpisodeId, ShowId, ShowPrefs};
use beacon_core::queue::Source;
use url::Url;

use crate::navigation::Location;
use crate::ui::directory_row::DirectoryItem;
use crate::ui::models::{EpisodeItem, RowAction, ShowCard, ShowPageData};
use crate::ui::page_filters;
use crate::ui::preview_page::PreviewData;
use crate::ui::queue_data::QueueData;
use crate::ui::search_data::SearchResults;
use crate::ui::selection_bar::Bulk;

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
    /// Whether feeds are refreshing.
    Refreshing(bool),
    SetQueue(Box<QueueData>),
    SetFavorites(Vec<EpisodeItem>),
    SetDownloaded(Vec<EpisodeItem>),
    /// What a search of the library found, for the page searching.
    SetSearch(Box<SearchResults>),
    /// Open the Add page and search Apple's directory for this.
    SearchOnline(String),
    /// The directory's answer to the Add page's request.
    DirectoryResults(String, Result<Vec<DirectoryItem>, beacon_core::ItunesError>),
    /// A directory result's thumbnail arrived.
    DirectoryArt(Url, std::path::PathBuf),
    /// Slide a directory result's preview in over the Add page.
    OpenPreview(Box<DirectoryItem>),
    /// The previewed feed loaded, or why it did not.
    PreviewLoaded(Url, Result<Box<PreviewData>, String>),
}

/// Where a search looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchScope {
    /// Every subscribed show and its episodes.
    Library,
    /// One show's episodes, filtering its page in place.
    Show(ShowId),
}

#[derive(Debug)]
pub enum ContentOutput {
    /// A row's action, with the list it is in: what plays after it, unless
    /// it is the Queue's own (`None`), which leaves that as it was.
    Action(Box<EpisodeItem>, RowAction, Option<Source>),
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
    /// What was chosen for the selected episodes.
    Change(Vec<EpisodeId>, Bulk),
    /// A podcast page's Download All Episodes.
    DownloadAll(Vec<EpisodeId>),
    /// The Queue's Clear, with the episodes it held.
    ClearQueue(Vec<EpisodeId>),
    /// Downloaded's Clear, with the downloaded episodes.
    DeleteAllDownloads(Vec<EpisodeId>),
    /// The search changed: what to look for, or `None` when it ended.
    Search(SearchScope, Option<String>),
    /// Search Apple's directory for this, on the Add page.
    SearchDirectory(String),
    /// Look up the show with this Apple Podcasts ID.
    LookupDirectory(u64),
    /// Load the feed of a directory result for its preview.
    Preview(Box<DirectoryItem>),
}
