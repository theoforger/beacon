// SPDX-License-Identifier: GPL-3.0-or-later

//! Where the content pane's pages send their requests: to the window as
//! `ContentOutput`, or back to the pane.

use beacon_core::queue::Source;

use super::{ContentInput, ContentOutput, SearchScope};
use crate::ui::messages::Output;
use crate::ui::search_view::SearchViewOutput;
use crate::ui::show_page::ShowPageOutput;

/// The search results' requests: rows and shows as everywhere else; a
/// row's episode plays on into the rest of its show.
pub(super) fn search_output(
    output: &Output<ContentOutput>,
    input: &impl Fn(ContentInput),
    message: SearchViewOutput,
) {
    match message {
        SearchViewOutput::Action(item, action) => {
            let source = Source::Show(item.episode.show_id);
            output.emit(ContentOutput::Action(item, action, Some(source)));
        }
        SearchViewOutput::OpenShow(id) => output.emit(ContentOutput::OpenShow(id)),
        SearchViewOutput::SearchOnline(query) => input(ContentInput::SearchOnline(query)),
    }
}

/// A podcast page's requests. A row's episode plays on into the rest of
/// its show, whether it is the page's own or an Everywhere result.
pub(super) fn show_page_output(
    output: &Output<ContentOutput>,
    input: &impl Fn(ContentInput),
    message: ShowPageOutput,
) {
    output.emit(match message {
        ShowPageOutput::Action(item, action) => {
            let source = Source::Show(item.episode.show_id);
            ContentOutput::Action(item, action, Some(source))
        }
        ShowPageOutput::PrefsChanged(id, prefs) => ContentOutput::ShowPrefsChanged(id, prefs),
        ShowPageOutput::MarkAll(id, played) => ContentOutput::MarkAll(id, played),
        ShowPageOutput::Unsubscribe(id) => ContentOutput::Unsubscribe(id),
        ShowPageOutput::FeedAddressCopied => ContentOutput::FeedAddressCopied,
        ShowPageOutput::Change(ids, change) => ContentOutput::Change(ids, change),
        ShowPageOutput::DownloadAll(ids) => ContentOutput::DownloadAll(ids),
        ShowPageOutput::OpenShow(id) => ContentOutput::OpenShow(id),
        ShowPageOutput::Search(show, query) => {
            output.emit(ContentOutput::Search(
                SearchScope::Show(show),
                query.this_show,
            ));
            ContentOutput::Search(SearchScope::Library, query.everywhere)
        }
        ShowPageOutput::SearchOnline(query) => {
            input(ContentInput::SearchOnline(query));
            return;
        }
    });
}
