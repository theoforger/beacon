// SPDX-License-Identifier: GPL-3.0-or-later

//! Podcast tags outside RSS and iTunes: Podlove Simple Chapters
//! (`<psc:chapters>`) and the Podcasting 2.0 namespace (`<podcast:season>`,
//! `<podcast:episode>`, `<podcast:chapters>`).
//!
//! The `rss` crate keeps these in an extension map keyed by the prefix the
//! feed declared, so look the prefix up by namespace URI.

use std::collections::BTreeMap;

use rss::extension::{Extension, ExtensionMap};
use url::Url;

use super::time::parse_npt;
use crate::model::Chapter;

pub const PSC: &str = "http://podlove.org/simple-chapters";
pub const PODCAST: &str = "https://podcastindex.org/namespace/1.0";

/// Finds extension elements by namespace URI.
pub struct Extensions<'a> {
    map: &'a ExtensionMap,
    prefixes: Vec<(&'a str, &'a str)>,
}

impl<'a> Extensions<'a> {
    pub fn new(map: &'a ExtensionMap, namespaces: &'a BTreeMap<String, String>) -> Self {
        let prefixes = namespaces
            .iter()
            .map(|(prefix, uri)| (uri.trim_end_matches('/'), prefix.as_str()))
            .collect();
        Self { map, prefixes }
    }

    pub fn first(&self, namespace: &str, name: &str) -> Option<&'a Extension> {
        let namespace = namespace.trim_end_matches('/');
        self.prefixes
            .iter()
            .filter(|(uri, _)| *uri == namespace)
            .find_map(|(_, prefix)| self.map.get(*prefix)?.get(name)?.first())
    }
}

/// Chapters from `<psc:chapters>`, sorted by start time.
pub fn psc_chapters(element: &Extension) -> Vec<Chapter> {
    let mut chapters: Vec<Chapter> = element
        .children
        .get("chapter")
        .into_iter()
        .flatten()
        .filter_map(|c| {
            Some(Chapter {
                start: parse_npt(c.attrs.get("start")?)?,
                title: c
                    .attrs
                    .get("title")
                    .map(|t| t.trim().to_owned())
                    .unwrap_or_default(),
                url: c.attrs.get("href").and_then(|u| Url::parse(u).ok()),
                image_url: c.attrs.get("image").and_then(|u| Url::parse(u).ok()),
            })
        })
        .collect();
    chapters.sort_by_key(|c| c.start);
    chapters.dedup_by_key(|c| c.start);
    chapters
}
