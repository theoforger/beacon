// SPDX-License-Identifier: GPL-3.0-or-later

//! Apple's podcast directory, through the iTunes Search API
//! (`docs/specs/add-and-itunes.md`): search by name, look a show up by its
//! Apple Podcasts ID, and the country whose directory is searched. Results
//! are cached for a day and calls kept to about 20 a minute, as Apple asks.
//! Subscribing stays plain RSS: a result is only a way to its `feedUrl`.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use url::Url;

use crate::error::ItunesError;

mod limiter;
mod storefronts;

pub use limiter::{CALLS_PER_MINUTE, RateLimiter, WINDOW};
pub use storefronts::STOREFRONTS;

pub const SEARCH_URL: &str = "https://itunes.apple.com/search";
pub const LOOKUP_URL: &str = "https://itunes.apple.com/lookup";
/// Results asked for in a search.
pub const RESULT_LIMIT: u32 = 25;
/// How long results are kept: Apple serves them with a day's `max-age`.
pub const CACHE_FOR: Duration = Duration::from_secs(24 * 60 * 60);
/// Searches kept at most; the oldest goes first.
const CACHE_SIZE: usize = 64;
/// The store searched when neither the setting nor the locale names one.
pub const FALLBACK_COUNTRY: &str = "US";

/// A show in Apple's directory, with what the API returns that Beacon uses.
#[derive(Debug, Clone, PartialEq)]
pub struct DirectoryShow {
    /// Apple's ID for the show (`collectionId`).
    pub id: u64,
    pub title: String,
    pub author: Option<String>,
    pub feed_url: Url,
    /// Apple's 100px thumbnail, the size AntennaPod and GNOME Podcasts show.
    pub artwork: Option<Url>,
    pub genre: Option<String>,
    pub episode_count: Option<u32>,
    /// When the newest episode came out.
    pub latest: Option<DateTime<Utc>>,
    /// The show's page on Apple Podcasts.
    pub store_url: Option<Url>,
}

/// The search URL for `term` in `country`'s directory.
pub fn search_url(base: &Url, term: &str, country: &str) -> Url {
    let mut url = base.clone();
    url.query_pairs_mut()
        .append_pair("term", term.trim())
        .append_pair("country", country)
        .append_pair("media", "podcast")
        .append_pair("entity", "podcast")
        .append_pair("limit", &RESULT_LIMIT.to_string());
    url
}

/// The lookup URL for the show with Apple's `id`.
pub fn lookup_url(base: &Url, id: u64, country: &str) -> Url {
    let mut url = base.clone();
    url.query_pairs_mut()
        .append_pair("id", &id.to_string())
        .append_pair("country", country)
        .append_pair("entity", "podcast");
    url
}

/// The show's ID in an Apple Podcasts link, such as
/// `https://podcasts.apple.com/us/podcast/some-show/id1412570127`.
pub fn apple_podcasts_id(url: &Url) -> Option<u64> {
    let host = url.host_str()?;
    if !matches!(host, "podcasts.apple.com" | "itunes.apple.com") {
        return None;
    }
    url.path_segments()?
        .filter_map(|segment| segment.strip_prefix("id"))
        .find_map(|digits| digits.parse().ok())
}

/// The country of a POSIX locale name such as `en_GB.UTF-8` or
/// `sr_RS@latin`, upper case, if it names one.
pub fn country_from_locale(locale: &str) -> Option<String> {
    let name = locale.split(['.', '@']).next()?;
    let (_, country) = name.split_once('_')?;
    (country.len() == 2 && country.chars().all(|c| c.is_ascii_alphabetic()))
        .then(|| country.to_ascii_uppercase())
}

/// Whether Apple has a podcast directory for `code`.
pub fn has_store(code: &str) -> bool {
    STOREFRONTS
        .iter()
        .any(|(c, _)| c.eq_ignore_ascii_case(code))
}

/// The directory to search: `setting` when it names one Apple has (an
/// empty setting means automatic), else the locale's country when Apple
/// has a directory there, else the United States.
pub fn resolve_country(setting: &str, locale: Option<&str>) -> String {
    let setting = setting.trim();
    if has_store(setting) {
        return setting.to_ascii_uppercase();
    }
    locale
        .and_then(country_from_locale)
        .filter(|code| has_store(code))
        .unwrap_or_else(|| FALLBACK_COUNTRY.to_owned())
}

/// The locale that names the system's country: `LC_ALL`, then
/// `LC_MESSAGES`, then `LANG`.
pub fn system_locale() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
}

#[derive(Deserialize)]
struct Response {
    results: Vec<RawResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawResult {
    kind: Option<String>,
    collection_id: Option<u64>,
    collection_name: Option<String>,
    artist_name: Option<String>,
    feed_url: Option<String>,
    artwork_url100: Option<String>,
    primary_genre_name: Option<String>,
    track_count: Option<u32>,
    release_date: Option<String>,
    collection_view_url: Option<String>,
}

fn web_url(text: Option<&str>) -> Option<Url> {
    let url = Url::parse(text?.trim()).ok()?;
    matches!(url.scheme(), "http" | "https").then_some(url)
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

impl RawResult {
    /// The show, unless it is not a podcast or has no feed to subscribe to.
    fn show(self) -> Option<DirectoryShow> {
        if self.kind.as_deref().is_some_and(|kind| kind != "podcast") {
            return None;
        }
        Some(DirectoryShow {
            id: self.collection_id?,
            title: non_empty(self.collection_name)?,
            feed_url: web_url(self.feed_url.as_deref())?,
            author: non_empty(self.artist_name),
            artwork: web_url(self.artwork_url100.as_deref()),
            genre: non_empty(self.primary_genre_name),
            episode_count: self.track_count,
            latest: self
                .release_date
                .and_then(|d| DateTime::parse_from_rfc3339(&d).ok())
                .map(|d| d.with_timezone(&Utc)),
            store_url: web_url(self.collection_view_url.as_deref()),
        })
    }
}

/// The shows in a response, in Apple's order, each feed once.
pub fn parse_results(bytes: &[u8]) -> Result<Vec<DirectoryShow>, ItunesError> {
    let response: Response =
        serde_json::from_slice(bytes).map_err(|err| ItunesError::Parse(err.to_string()))?;
    let mut shows: Vec<DirectoryShow> = Vec::with_capacity(response.results.len());
    for show in response.results.into_iter().filter_map(RawResult::show) {
        if !shows.iter().any(|s| s.feed_url == show.feed_url) {
            shows.push(show);
        }
    }
    Ok(shows)
}

/// What a search is cached under: the term as typed, give or take case
/// and spaces, and the country.
fn cache_key(term: &str, country: &str) -> String {
    let words: Vec<_> = term.split_whitespace().collect();
    format!("{country}:{}", words.join(" ").to_lowercase())
}

#[derive(Default)]
struct State {
    limiter: RateLimiter,
    cache: HashMap<String, (Instant, Vec<DirectoryShow>)>,
}

impl State {
    fn cached(&mut self, key: &str, now: Instant) -> Option<Vec<DirectoryShow>> {
        match self.cache.get(key) {
            Some((at, shows)) if now.saturating_duration_since(*at) < CACHE_FOR => {
                Some(shows.clone())
            }
            Some(_) => {
                self.cache.remove(key);
                None
            }
            None => None,
        }
    }

    fn keep(&mut self, key: String, shows: &[DirectoryShow], now: Instant) {
        if self.cache.len() >= CACHE_SIZE
            && let Some(oldest) = self
                .cache
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(k, _)| k.clone())
        {
            self.cache.remove(&oldest);
        }
        self.cache.insert(key, (now, shows.to_vec()));
    }
}

/// The iTunes Search API, shared by every search: one cache and one limit.
pub struct ItunesClient {
    http: reqwest::Client,
    search: Url,
    lookup: Url,
    state: Mutex<State>,
}

impl ItunesClient {
    pub fn new(http: reqwest::Client) -> Self {
        Self::with_endpoints(
            http,
            Url::parse(SEARCH_URL).expect("a valid constant"),
            Url::parse(LOOKUP_URL).expect("a valid constant"),
        )
    }

    /// A client calling other endpoints, as tests do.
    pub fn with_endpoints(http: reqwest::Client, search: Url, lookup: Url) -> Self {
        Self {
            http,
            search,
            lookup,
            state: Mutex::default(),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Search `country`'s directory for shows matching `term`.
    pub async fn search(
        &self,
        term: &str,
        country: &str,
    ) -> Result<Vec<DirectoryShow>, ItunesError> {
        let key = cache_key(term, country);
        if term.trim().is_empty() {
            return Ok(Vec::new());
        }
        let url = search_url(&self.search, term, country);
        self.get(key, url, country).await
    }

    /// The show with Apple's `id`, as a pasted Apple Podcasts link names it.
    pub async fn lookup(&self, id: u64, country: &str) -> Result<DirectoryShow, ItunesError> {
        let key = format!("{country}:id:{id}");
        let url = lookup_url(&self.lookup, id, country);
        let shows = self.get(key, url, country).await?;
        shows
            .into_iter()
            .find(|show| show.id == id)
            .ok_or(ItunesError::NoFeed)
    }

    async fn get(
        &self,
        key: String,
        url: Url,
        country: &str,
    ) -> Result<Vec<DirectoryShow>, ItunesError> {
        {
            let now = Instant::now();
            let mut state = self.state();
            if let Some(shows) = state.cached(&key, now) {
                return Ok(shows);
            }
            state
                .limiter
                .acquire(now)
                .map_err(ItunesError::RateLimited)?;
        }
        let response = self.http.get(url).send().await?;
        let status = response.status().as_u16();
        match status {
            200..=299 => {}
            // Apple answers 403 when a client calls too often.
            403 | 429 => return Err(ItunesError::RateLimited(WINDOW)),
            400 => return Err(ItunesError::NoStore(country.to_owned())),
            _ => return Err(ItunesError::Status(status)),
        }
        let shows = parse_results(&response.bytes().await?)?;
        self.state().keep(key, &shows, Instant::now());
        Ok(shows)
    }
}

/// Empty `dir`, the scratch folder results' thumbnails are kept in, so
/// that none outlives the session (`docs/specs/add-and-itunes.md`,
/// decision 5). Their cache is a `CoverCache` on it, never the library's
/// covers.
pub fn empty_artwork_dir(dir: &std::path::Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(dir) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests;
