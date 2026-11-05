// SPDX-License-Identifier: GPL-3.0-or-later

//! Podcasting 2.0 JSON chapters (`<podcast:chapters>`), fetched when an
//! episode's details first need them. Inline Podlove chapters come with the
//! feed instead (`mod.rs`).

use std::time::Duration;

use serde::Deserialize;
use url::Url;

use crate::error::FeedError;
use crate::model::Chapter;

#[derive(Deserialize)]
struct File {
    chapters: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    start_time: f64,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    img: Option<String>,
    /// `false` keeps a chapter out of the table of contents.
    #[serde(default)]
    toc: Option<bool>,
}

/// The chapters of a JSON chapters file, in the order they start. Chapters
/// marked `"toc": false` are left out, as the specification asks.
pub fn parse(body: &[u8]) -> Result<Vec<Chapter>, FeedError> {
    let file: File =
        serde_json::from_slice(body).map_err(|err| FeedError::Parse(err.to_string()))?;
    let mut chapters: Vec<Chapter> = file
        .chapters
        .into_iter()
        .filter(|entry| entry.toc != Some(false))
        .filter(|entry| entry.start_time.is_finite() && entry.start_time >= 0.0)
        .map(|entry| Chapter {
            start: Duration::from_secs_f64(entry.start_time),
            title: entry.title.unwrap_or_default(),
            url: entry.url.and_then(|u| Url::parse(&u).ok()),
            image_url: entry.img.and_then(|u| Url::parse(&u).ok()),
        })
        .collect();
    chapters.sort_by_key(|chapter| chapter.start);
    Ok(chapters)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_real_chapters_file_parses_in_order() {
        let body =
            include_bytes!("../../../../../tests/fixtures/destination-linux-489-chapters.json");
        let chapters = parse(body).unwrap();
        assert_eq!(chapters.len(), 12);
        assert_eq!(chapters[0].start, Duration::ZERO);
        assert_eq!(chapters[0].title, "Intro");
        assert_eq!(chapters[2].start, Duration::from_secs(96));
        assert!(
            chapters
                .windows(2)
                .all(|pair| pair[0].start <= pair[1].start)
        );
    }

    #[test]
    fn hidden_chapters_and_bad_times_are_left_out() {
        let body = br#"{"version":"1.2.0","chapters":[
            {"startTime":30.5,"title":"Second","url":"https://example.com/a","img":"https://example.com/a.png"},
            {"startTime":0,"title":"First"},
            {"startTime":10,"title":"Hidden","toc":false},
            {"startTime":-4,"title":"Negative"}
        ]}"#;
        let chapters = parse(body).unwrap();
        let titles: Vec<_> = chapters.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, ["First", "Second"]);
        assert_eq!(chapters[1].start, Duration::from_millis(30_500));
        assert_eq!(
            chapters[1].url.as_ref().unwrap().as_str(),
            "https://example.com/a"
        );
    }

    #[test]
    fn not_a_chapters_file_is_a_parse_error() {
        assert!(matches!(parse(b"<html>"), Err(FeedError::Parse(_))));
        assert!(matches!(
            parse(b"{\"title\":\"x\"}"),
            Err(FeedError::Parse(_))
        ));
    }
}
