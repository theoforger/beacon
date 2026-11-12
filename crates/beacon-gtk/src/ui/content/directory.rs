// SPDX-License-Identifier: GPL-3.0-or-later

//! The pages for finding shows: the Add page and a directory result's
//! preview (a podcast page in preview mode), pushed over the current
//! section, and the search results.

use adw::prelude::*;
use adw::subclass::prelude::*;

use super::sections::bring_forward;
use super::{Content, ContentInput, ContentOutput};
use crate::ui::add_page::{ADD_TAG, AddPage, AddPageInput, AddPageOutput};
use crate::ui::messages::Output;
use crate::ui::preview_data;
use crate::ui::search_view::SearchViewInput;
use crate::ui::show_page::{PREVIEW_TAG, ShowPageInput};

/// The Add page, reporting to `output`.
pub(super) fn add_page(output: &Output<ContentOutput>) -> AddPage {
    let add_page = AddPage::default();
    let output = output.clone();
    add_page.connect_output(move |message| {
        output.emit(match message {
            AddPageOutput::Subscribe(url) => ContentOutput::Subscribe(url),
            AddPageOutput::Back => ContentOutput::GoBack,
            AddPageOutput::Search(words) => ContentOutput::SearchDirectory(words),
            AddPageOutput::Lookup(id) => ContentOutput::LookupDirectory(id),
            AddPageOutput::OpenShow(id) => ContentOutput::OpenShow(id),
            AddPageOutput::Open(item) => ContentOutput::Preview(item),
        });
    });
    add_page
}

impl Content {
    pub(super) fn update_directory(&self, message: ContentInput) {
        let imp = self.imp();
        let parts = imp.parts.get().expect("built when constructed");
        let widgets = self.widgets();
        let navigation = &widgets.navigation;
        match message {
            ContentInput::OpenAdd => {
                parts.add_page.clear();
                bring_forward(navigation, parts.add_page.upcast_ref(), ADD_TAG);
            }
            ContentInput::SubscribeDone => {
                parts.add_page.send(AddPageInput::Done);
                parts.preview_page.send(ShowPageInput::SubscribeDone);
            }
            ContentInput::DirectoryResults(key, result) => {
                parts.add_page.send(AddPageInput::Results(key, result));
            }
            ContentInput::DirectoryArt(url, path) => {
                parts.add_page.send(AddPageInput::Art(url, path));
            }
            ContentInput::OpenPreview(item) => {
                let data = preview_data::directory_page(&item);
                let store = item.show.store_url.clone();
                parts
                    .preview_page
                    .send(ShowPageInput::Preview(Box::new(data), store));
                bring_forward(navigation, parts.preview_page.upcast_ref(), PREVIEW_TAG);
            }
            ContentInput::PreviewLoaded(url, result) => {
                if parts.preview_page.feed().as_ref() == Some(&url) {
                    parts
                        .preview_page
                        .send(ShowPageInput::PreviewLoaded(result));
                }
            }
            ContentInput::SetSearch(results) => {
                parts.search.send(SearchViewInput::Set(results));
            }
            ContentInput::SearchOnline(query) => {
                widgets.search_bar.set_search_mode(false);
                parts.add_page.clear();
                bring_forward(navigation, parts.add_page.upcast_ref(), ADD_TAG);
                parts.add_page.send(AddPageInput::Search(query));
            }
            _ => {}
        }
    }

    /// The root page's search field, title and the page showing, for tests.
    #[cfg(test)]
    pub fn search_state(&self) -> (gtk::SearchEntry, String, Option<String>) {
        let widgets = self.widgets();
        (
            widgets.search_entry.clone(),
            widgets.root_page.title().to_string(),
            widgets.navigation.visible_page_tag().map(|t| t.to_string()),
        )
    }

    /// The root page's search results and the Add page, for tests.
    #[cfg(test)]
    pub fn search_pages(&self) -> (crate::ui::search_view::SearchView, AddPage) {
        let parts = self.imp().parts.get().expect("built when constructed");
        (parts.search.clone(), parts.add_page.clone())
    }
}
