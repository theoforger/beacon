// SPDX-License-Identifier: GPL-3.0-or-later

//! Apple's directory, for the Add page (`docs/specs/add-and-itunes.md`):
//! searches and lookups through the core's iTunes client, each result
//! marked when its feed is already subscribed to, results' thumbnails
//! fetched into a scratch cache, and previews of a result's feed, kept so
//! that subscribing does not fetch it again.

use std::sync::Arc;

use beacon_core::services::covers::CoverCache;
use beacon_core::services::http_client;
use beacon_core::services::itunes::{self, DirectoryShow, ItunesClient};
use beacon_core::services::library::{FeedPreview, Library};
use beacon_core::{ItunesError, LibraryError};
use gtk::glib;
use gtk::prelude::*;
use url::Url;

use super::{App, AppCommand};
use crate::ui::content::ContentInput;
use crate::ui::directory_row::DirectoryItem;
use crate::ui::errors;
use crate::ui::format;
use crate::ui::preview_page::{PREVIEW_EPISODES, PreviewData};
use crate::ui::show_hero::HeroData;

/// The setting that names the directory's country; empty for automatic.
pub(super) const COUNTRY_KEY: &str = "directory-country";

pub struct Directory {
    itunes: Arc<ItunesClient>,
    art: CoverCache,
    /// The feed last previewed, for Subscribe.
    previewed: Option<FeedPreview>,
}

impl Directory {
    pub fn new() -> Self {
        let dir = glib::user_cache_dir().join("beacon").join("directory");
        if let Err(err) = itunes::empty_artwork_dir(&dir) {
            tracing::warn!("could not empty {}: {err}", dir.display());
        }
        let http = http_client();
        Self {
            itunes: Arc::new(ItunesClient::new(http.clone())),
            art: CoverCache::new(dir, http),
            previewed: None,
        }
    }
}

/// Each result, with the show already subscribed to its feed, if any.
fn mark_subscribed(library: &Library, shows: Vec<DirectoryShow>) -> Vec<DirectoryItem> {
    shows
        .into_iter()
        .map(|show| DirectoryItem {
            subscribed: library
                .with_store(|store| store.show_id_for_feed(&show.feed_url))
                .ok()
                .flatten(),
            show,
            art: None,
        })
        .collect()
}

/// What the preview shows of a fetched feed: the feed's own cover and
/// text, and its newest episodes.
pub(super) fn preview_data(preview: &FeedPreview) -> PreviewData {
    let show = &preview.feed.show;
    let mut episodes: Vec<_> = preview.feed.episodes.iter().map(|e| &e.info).collect();
    episodes.sort_by_key(|info| std::cmp::Reverse(info.published));
    PreviewData {
        hero: HeroData {
            cover: preview.cover.clone(),
            category: show.category.clone(),
            title: show.title.clone(),
            author: show.author.clone(),
            description: show.description.clone(),
        },
        episodes: episodes
            .into_iter()
            .take(PREVIEW_EPISODES)
            .map(|info| (info.title.clone(), format::info_subtitle(info)))
            .collect(),
    }
}

impl App {
    /// The directory searched: the setting's, the locale's, or the US.
    fn country(&self) -> String {
        itunes::resolve_country(
            &self.settings.string(COUNTRY_KEY),
            itunes::system_locale().as_deref(),
        )
    }

    pub(super) fn search_directory(&self, words: String) {
        let key = words.to_lowercase();
        self.ask_directory(key, move |itunes, country| async move {
            itunes.search(&words, &country).await
        });
    }

    pub(super) fn lookup_directory(&self, id: u64) {
        self.ask_directory(format!("id:{id}"), move |itunes, country| async move {
            itunes.lookup(id, &country).await.map(|show| vec![show])
        });
    }

    fn ask_directory<F>(
        &self,
        key: String,
        ask: impl FnOnce(Arc<ItunesClient>, String) -> F + Send + 'static,
    ) where
        F: Future<Output = Result<Vec<DirectoryShow>, ItunesError>> + Send,
    {
        let Some(library) = self.library.clone() else {
            return;
        };
        let (itunes, country) = (self.directory.itunes.clone(), self.country());
        self.sender.spawn(async move {
            let result = ask(itunes, country)
                .await
                .map(|shows| mark_subscribed(&library, shows));
            AppCommand::Directory(key, result)
        });
    }

    /// Show the answer, then fetch its thumbnails.
    pub(super) fn directory_answered(
        &self,
        key: String,
        result: Result<Vec<DirectoryItem>, ItunesError>,
    ) {
        if let Err(err) = &result {
            tracing::warn!("could not search Apple's directory: {err}");
        }
        let thumbnails: Vec<Url> = result
            .as_ref()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|i| i.show.artwork.clone())
                    .collect()
            })
            .unwrap_or_default();
        self.content
            .send(ContentInput::DirectoryResults(key, result));
        for url in thumbnails {
            let art = self.directory.art.clone();
            self.sender.spawn(async move {
                let path = art.fetch(&url).await.ok();
                AppCommand::DirectoryArt(url, path)
            });
        }
    }

    /// Open a result's preview and fetch its feed.
    pub(super) fn preview(&mut self, item: DirectoryItem) {
        let Some(library) = self.library.clone() else {
            return;
        };
        let url = item.show.feed_url.clone();
        self.directory.previewed = None;
        self.content.send(ContentInput::OpenPreview(Box::new(item)));
        self.sender.spawn(async move {
            let result = library.preview(&url).await;
            AppCommand::Preview(url, Box::new(result))
        });
    }

    pub(super) fn previewed(&mut self, url: Url, result: Result<FeedPreview, LibraryError>) {
        let shown = match result {
            Ok(preview) => {
                let data = preview_data(&preview);
                self.directory.previewed = Some(preview);
                Ok(Box::new(data))
            }
            Err(err) => {
                tracing::warn!("could not preview {url}: {err}");
                Err(errors::library(&err))
            }
        };
        self.content.send(ContentInput::PreviewLoaded(url, shown));
    }

    /// Subscribe to `url`: from its preview when it was just previewed,
    /// without fetching the feed again.
    pub(super) fn subscribe(&mut self, url: Url) {
        let Some(library) = self.library.clone() else {
            return;
        };
        let preview = self
            .directory
            .previewed
            .take()
            .filter(|preview| preview.url == url);
        self.sender.spawn(async move {
            let result = match &preview {
                Some(preview) => library.subscribe_preview(preview).await,
                None => library.subscribe(&url).await,
            };
            AppCommand::Subscribed(url, result)
        });
    }
}
