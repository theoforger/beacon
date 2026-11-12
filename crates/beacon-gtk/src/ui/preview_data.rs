// SPDX-License-Identifier: GPL-3.0-or-later

//! A show not in the library, as its podcast page previews it
//! (`ui/show_page/`): first from what Apple's directory said, then from its
//! fetched feed, with every episode, newest first, in seasons when the feed
//! tags them.

use std::path::PathBuf;

use beacon_core::model::{Episode, EpisodeId, Show, ShowId, ShowInfo, ShowPrefs};
use beacon_core::services::library::FeedPreview;
use chrono::Utc;
use url::Url;

use crate::ui::directory_row::DirectoryItem;
use crate::ui::models::{EpisodeItem, ShowPageData, arrange};

/// The id a previewed show and its episodes stand in with: the store's ids
/// start at 1, and a preview's are never sent anywhere (its rows do not
/// act).
pub const PREVIEW_SHOW: ShowId = ShowId(0);

/// A show not in the library, as its podcast page previews it: from what
/// Apple's directory said, while its feed loads.
pub fn directory_page(item: &DirectoryItem) -> ShowPageData {
    let show = &item.show;
    preview_show(
        show.feed_url.clone(),
        ShowInfo {
            title: show.title.clone(),
            author: show.author.clone(),
            category: show.genre.clone(),
            ..Default::default()
        },
        item.art.clone(),
        Vec::new(),
    )
}

/// The same from its fetched feed: its own cover and text, and every
/// episode, newest first, in seasons when it has them.
pub fn preview_page(preview: &FeedPreview) -> ShowPageData {
    let now = Utc::now();
    let episodes = preview
        .feed
        .episodes
        .iter()
        .zip(1..)
        .map(|(parsed, n)| Episode {
            id: EpisodeId(-n),
            show_id: PREVIEW_SHOW,
            info: parsed.info.clone(),
            state: Default::default(),
            added_at: now,
        })
        .collect();
    preview_show(
        preview.url.clone(),
        preview.feed.show.clone(),
        preview.cover.clone(),
        episodes,
    )
}

fn preview_show(
    feed_url: Url,
    info: ShowInfo,
    cover: Option<PathBuf>,
    episodes: Vec<Episode>,
) -> ShowPageData {
    let show_title = info.title.clone();
    let prefs = ShowPrefs::default();
    let (seasons, episodes) = arrange(episodes, prefs, |episode| EpisodeItem {
        episode,
        show_title: show_title.clone(),
        cover: cover.clone(),
        in_queue: false,
    });
    ShowPageData {
        show: Show {
            id: PREVIEW_SHOW,
            feed_url,
            info,
            cover_path: cover,
            subscribed_at: Utc::now(),
            last_refreshed_at: None,
        },
        prefs,
        seasons,
        episodes,
        all_played: false,
        to_download: Vec::new(),
        query: None,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A feed fixture, as fetched for a preview.
    pub(crate) fn preview(name: &str) -> FeedPreview {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(name);
        let bytes = std::fs::read(path).unwrap();
        FeedPreview {
            url: Url::parse("https://example.com/feed.xml").unwrap(),
            feed: beacon_core::services::feeds::parse_feed(&bytes).unwrap(),
            cover: None,
            subscribed: None,
        }
    }

    #[test]
    fn a_preview_is_the_podcast_page_of_its_feed() {
        let preview = preview("seasons.xml");
        let data = preview_page(&preview);
        assert_eq!(data.show.id, PREVIEW_SHOW);
        assert_eq!(data.show.info, preview.feed.show);
        // In seasons, as the feed tags them, every episode with an id the
        // store never gives.
        let seasons = data.seasons.as_ref().expect("the feed has seasons");
        let items: Vec<_> = seasons.iter().flat_map(|s| &s.episodes).collect();
        assert_eq!(items.len(), preview.feed.episodes.len());
        assert!(items.iter().all(|i| i.episode.id.0 < 0 && !i.in_queue));
        let mut ids: Vec<_> = items.iter().map(|i| i.episode.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), items.len());
        assert_eq!(seasons[0].season.as_ref().map(|s| s.number), Some(3));
    }
}
