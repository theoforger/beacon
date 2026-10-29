// SPDX-License-Identifier: GPL-3.0-or-later

//! Fetch and parse podcast feeds (RSS 2.0 with the iTunes, Podcasting 2.0 and
//! Podlove Simple Chapters namespaces).

mod namespaces;
pub mod time;

use rss::{Channel, Item};
use url::Url;

use self::namespaces::{Extensions, PODCAST, PSC};
use self::time::{parse_date, parse_npt};
use crate::error::FeedError;
use crate::model::{Enclosure, EpisodeInfo, ParsedEpisode, ParsedFeed, Season, ShowInfo};

#[derive(Debug, Clone)]
pub struct FeedClient {
    http: reqwest::Client,
}

impl FeedClient {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    pub async fn fetch(&self, url: &Url) -> Result<ParsedFeed, FeedError> {
        let response = self.http.get(url.clone()).send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(FeedError::Status(status.as_u16()));
        }
        let body = response.bytes().await?;
        parse_feed(&body)
    }
}

pub fn parse_feed(bytes: &[u8]) -> Result<ParsedFeed, FeedError> {
    let channel = Channel::read_from(bytes).map_err(|err| FeedError::Parse(err.to_string()))?;
    let show = show_info(&channel);
    let episodes = channel
        .items()
        .iter()
        .map(|item| parse_item(item, &channel))
        .collect();
    Ok(ParsedFeed { show, episodes })
}

fn show_info(channel: &Channel) -> ShowInfo {
    let itunes = channel.itunes_ext();
    ShowInfo {
        title: channel.title().trim().to_owned(),
        author: itunes.and_then(|i| non_empty(i.author.as_deref())),
        description: non_empty(Some(channel.description()))
            .or_else(|| itunes.and_then(|i| non_empty(i.summary.as_deref()))),
        link: url(Some(channel.link())),
        image_url: url(itunes.and_then(|i| i.image.as_deref()))
            .or_else(|| url(channel.image().map(|i| i.url()))),
        category: itunes
            .and_then(|i| i.categories.first())
            .map(|c| c.text.trim().to_owned())
            .or_else(|| {
                channel
                    .categories()
                    .first()
                    .map(|c| c.name.trim().to_owned())
            })
            .filter(|c| !c.is_empty()),
    }
}

fn parse_item(item: &Item, channel: &Channel) -> ParsedEpisode {
    let itunes = item.itunes_ext();
    let extensions = Extensions::new(item.extensions(), channel.namespaces());

    let enclosure = item.enclosure().and_then(|e| {
        Some(Enclosure {
            url: url(Some(&e.url))?,
            mime_type: non_empty(Some(&e.mime_type)),
            length: e.length.trim().parse().ok().filter(|&l| l > 0),
        })
    });

    // Identity must survive refreshes. Without a GUID, the enclosure URL is
    // the most stable thing an item has.
    let guid = non_empty(item.guid().map(|g| g.value()))
        .or_else(|| enclosure.as_ref().map(|e| e.url.to_string()))
        .or_else(|| non_empty(item.link()))
        .or_else(|| non_empty(item.title()))
        .unwrap_or_default();

    let podcast_season = extensions.first(PODCAST, "season");
    let season = itunes
        .and_then(|i| i.season.as_deref())
        .or_else(|| podcast_season.and_then(|s| s.value.as_deref()))
        .and_then(|n| n.trim().parse().ok())
        .map(|number| Season {
            number,
            name: podcast_season.and_then(|s| non_empty(s.attrs.get("name").map(String::as_str))),
        });

    let info = EpisodeInfo {
        guid,
        title: item.title().unwrap_or_default().trim().to_owned(),
        description: non_empty(item.description())
            .or_else(|| non_empty(item.content()))
            .or_else(|| itunes.and_then(|i| non_empty(i.summary.as_deref()))),
        published: item.pub_date().and_then(parse_date),
        duration: itunes
            .and_then(|i| i.duration.as_deref())
            .and_then(parse_npt),
        enclosure,
        season,
        number: itunes
            .and_then(|i| i.episode.as_deref())
            .or_else(|| {
                extensions
                    .first(PODCAST, "episode")
                    .and_then(|e| e.value.as_deref())
            })
            .and_then(|n| n.trim().parse().ok()),
        image_url: url(itunes.and_then(|i| i.image.as_deref())),
        chapters_url: url(extensions
            .first(PODCAST, "chapters")
            .and_then(|c| c.attrs.get("url"))
            .map(String::as_str)),
    };
    let chapters = extensions
        .first(PSC, "chapters")
        .map(namespaces::psc_chapters)
        .unwrap_or_default();
    ParsedEpisode { info, chapters }
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

fn url(value: Option<&str>) -> Option<Url> {
    Url::parse(value?.trim()).ok()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const MINIMAL: &str = r#"<?xml version="1.0"?>
        <rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd"
             xmlns:pod="https://podcastindex.org/namespace/1.0"
             xmlns:psc="http://podlove.org/simple-chapters"><channel>
          <title> Small Hours </title>
          <link>https://example.com/</link>
          <description>Night radio</description>
          <itunes:author>Odile Brandt</itunes:author>
          <itunes:image href="https://example.com/cover.png"/>
          <itunes:category text="Science"/>
          <item>
            <title>One</title>
            <guid>ep-1</guid>
            <pubDate>Tue, 27 Oct 2026 08:00:00 GMT</pubDate>
            <itunes:duration>52:00</itunes:duration>
            <itunes:season>2</itunes:season>
            <pod:season name="Deserts">2</pod:season>
            <itunes:episode>5</itunes:episode>
            <itunes:image href="https://example.com/ep.png"/>
            <pod:chapters url="https://example.com/ch.json" type="application/json+chapters"/>
            <psc:chapters version="1.2">
              <psc:chapter start="03:12" title="Second"/>
              <psc:chapter start="0" title="First" href="https://example.com/a"/>
            </psc:chapters>
            <enclosure url="https://example.com/1.mp3" type="audio/mpeg" length="1234"/>
          </item>
          <item>
            <title>No GUID</title>
            <enclosure url="https://example.com/2.mp3" type="audio/mpeg" length="0"/>
          </item>
        </channel></rss>"#;

    #[test]
    fn reads_show_metadata() {
        let feed = parse_feed(MINIMAL.as_bytes()).unwrap();
        assert_eq!(feed.show.title, "Small Hours");
        assert_eq!(feed.show.author.as_deref(), Some("Odile Brandt"));
        assert_eq!(feed.show.description.as_deref(), Some("Night radio"));
        assert_eq!(feed.show.category.as_deref(), Some("Science"));
        assert_eq!(
            feed.show.image_url.unwrap().as_str(),
            "https://example.com/cover.png"
        );
        assert_eq!(feed.show.link.unwrap().as_str(), "https://example.com/");
    }

    #[test]
    fn reads_episode_metadata() {
        let feed = parse_feed(MINIMAL.as_bytes()).unwrap();
        let one = &feed.episodes[0].info;
        assert_eq!(one.guid, "ep-1");
        assert_eq!(one.duration, Some(Duration::from_secs(52 * 60)));
        let season = one.season.as_ref().unwrap();
        assert_eq!(
            (season.number, season.name.as_deref()),
            (2, Some("Deserts"))
        );
        assert_eq!(one.number, Some(5.0));
        assert_eq!(
            one.image_url.as_ref().unwrap().as_str(),
            "https://example.com/ep.png"
        );
        assert_eq!(
            one.chapters_url.as_ref().unwrap().as_str(),
            "https://example.com/ch.json"
        );
        let enclosure = one.enclosure.as_ref().unwrap();
        assert_eq!(enclosure.mime_type.as_deref(), Some("audio/mpeg"));
        assert_eq!(enclosure.length, Some(1234));
        assert!(one.published.is_some());
    }

    #[test]
    fn reads_psc_chapters_in_order() {
        let feed = parse_feed(MINIMAL.as_bytes()).unwrap();
        let chapters = &feed.episodes[0].chapters;
        let titles: Vec<_> = chapters.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, ["First", "Second"]);
        assert_eq!(chapters[1].start, Duration::from_secs(192));
        assert_eq!(
            chapters[0].url.as_ref().unwrap().as_str(),
            "https://example.com/a"
        );
        assert!(feed.episodes[1].chapters.is_empty());
    }

    #[test]
    fn missing_guid_falls_back_to_enclosure_url() {
        let feed = parse_feed(MINIMAL.as_bytes()).unwrap();
        let two = &feed.episodes[1].info;
        assert_eq!(two.guid, "https://example.com/2.mp3");
        assert_eq!(two.enclosure.as_ref().unwrap().length, None);
        assert_eq!(two.season, None);
    }

    #[test]
    fn rejects_non_feeds() {
        assert!(matches!(
            parse_feed(b"<html><body>Not a feed</body></html>"),
            Err(FeedError::Parse(_))
        ));
        assert!(matches!(
            parse_feed(b"<rss><channel>"),
            Err(FeedError::Parse(_))
        ));
    }
}
