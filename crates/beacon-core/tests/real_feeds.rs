// SPDX-License-Identifier: GPL-3.0-or-later

//! Parsing real feeds. The fixtures are trimmed copies of published feeds
//! (see `tests/fixtures/README.md`); nothing here touches the network.

use std::path::{Path, PathBuf};
use std::time::Duration;

use beacon_core::model::ParsedFeed;
use beacon_core::services::feeds::parse_feed;
use beacon_core::store::Store;
use chrono::{TimeZone, Utc};

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn parse_fixture(name: &str) -> ParsedFeed {
    parse_feed(&std::fs::read(fixture_path(name)).unwrap()).unwrap()
}

fn hms(h: u64, m: u64, s: u64) -> Option<Duration> {
    Some(Duration::from_secs(h * 3600 + m * 60 + s))
}

#[test]
fn hello_from_the_magic_tavern() {
    let feed = parse_fixture("magic-tavern.xml");
    assert_eq!(feed.show.title, "Hello From The Magic Tavern");
    assert_eq!(feed.show.author.as_deref(), Some("Arnie Niekamp"));
    assert_eq!(feed.show.category.as_deref(), Some("Comedy"));
    assert!(feed.show.image_url.is_some());
    assert!(feed.show.description.is_some());

    let episodes: Vec<_> = feed.episodes.iter().map(|e| &e.info).collect();
    assert_eq!(episodes.len(), 4);
    assert_eq!(
        episodes[0].title,
        "Season 6, Ep 11 - Chunt for Red October 10: Dead or Elaine"
    );
    assert_eq!(episodes[0].duration, hms(0, 50, 28));
    // `-0000` is RFC 2822's "local time unknown"; read it as UTC.
    assert_eq!(
        episodes[0].published,
        Some(Utc.with_ymd_and_hms(2026, 10, 26, 4, 0, 0).unwrap())
    );
    assert_eq!(
        episodes[3].published,
        Some(Utc.with_ymd_and_hms(2015, 3, 5, 8, 0, 0).unwrap())
    );
    // The seasons are only in the titles, so there are no season tags to group by.
    assert!(
        episodes
            .iter()
            .all(|e| e.season.is_none() && e.number.is_none())
    );
    // Recent episodes carry ART19 identifiers; the first one kept the GUID it
    // had at Earwolf, the show's earlier host.
    assert!(
        episodes[..3]
            .iter()
            .all(|e| e.guid.starts_with("gid://art19-episode-locator/"))
    );
    assert_eq!(
        episodes[3].guid,
        "http://www.earwolf.com/episode/hello-from-the-magic-tavern/"
    );
    for episode in &episodes {
        assert!(episode.enclosure.is_some(), "{}", episode.title);
        assert!(episode.image_url.is_some(), "{}", episode.title);
        assert!(episode.description.is_some(), "{}", episode.title);
    }
}

#[test]
fn hey_riddle_riddle() {
    let feed = parse_fixture("hey-riddle-riddle.xml");
    assert_eq!(feed.show.title, "Hey Riddle Riddle");
    assert_eq!(feed.show.author.as_deref(), Some("Headgum"));
    assert_eq!(feed.show.category.as_deref(), Some("Leisure"));

    let summary: Vec<_> = feed
        .episodes
        .iter()
        .map(|e| (e.info.number, e.info.duration))
        .collect();
    assert_eq!(
        summary,
        [
            (Some(432.0), hms(1, 6, 23)),
            (None, hms(0, 8, 45)),
            (Some(431.0), hms(1, 8, 13)),
            (None, hms(0, 10, 36)),
            (Some(268.0), hms(0, 9, 44)),
            (None, hms(0, 0, 56)),
        ]
    );
    let trailer = &feed.episodes[5].info;
    assert_eq!(trailer.title, "Trailer");
    assert_eq!(
        trailer.published,
        Some(Utc.with_ymd_and_hms(2018, 7, 15, 22, 10, 4).unwrap())
    );
    assert!(feed.episodes.iter().all(|e| e.info.season.is_none()));
}

#[test]
fn destination_linux() {
    let feed = parse_fixture("destination-linux.xml");
    assert_eq!(feed.show.title, "Destination Linux");
    assert!(feed.show.image_url.is_some());

    let episodes: Vec<_> = feed.episodes.iter().map(|e| &e.info).collect();
    assert_eq!(episodes.len(), 5);
    assert_eq!(
        episodes[0].title,
        "489: Destination Linux 489: PHP2Shell Emergency & Inside the OOSC Conference"
    );
    // Single-digit hours, and MM:SS on older episodes.
    let durations: Vec<_> = episodes.iter().map(|e| e.duration).collect();
    assert_eq!(
        durations,
        [
            hms(1, 35, 41),
            hms(1, 9, 40),
            hms(1, 4, 44),
            hms(0, 56, 17),
            hms(0, 43, 28)
        ]
    );
    assert_eq!(
        episodes[0].published,
        Some(Utc.with_ymd_and_hms(2026, 10, 26, 10, 0, 0).unwrap())
    );
    // Podcasting 2.0 chapters on recent episodes, none on old ones.
    let with_chapters: Vec<_> = episodes.iter().map(|e| e.chapters_url.is_some()).collect();
    assert_eq!(with_chapters, [true, true, false, true, false]);
    assert!(
        episodes[0]
            .chapters_url
            .as_ref()
            .unwrap()
            .as_str()
            .ends_with("/2ccead8f-d833-4919-a578-d1ca3b8e2b58/chapters")
    );
    // The first episode kept its WordPress identifier from before the move.
    assert_eq!(episodes[4].guid, "http://destinationlinux.org/?p=16");
    assert!(
        episodes
            .iter()
            .all(|e| e.season.is_none() && e.number.is_none())
    );
}

#[test]
fn real_feeds_store_every_episode_once() {
    for name in [
        "magic-tavern.xml",
        "hey-riddle-riddle.xml",
        "destination-linux.xml",
    ] {
        let feed = parse_fixture(name);
        let mut store = Store::open_in_memory().unwrap();
        let url = url::Url::parse("https://example.com/feed.xml").unwrap();
        let now = Utc::now();
        let show = store.insert_show(&url, &feed.show, now).unwrap();
        let stats = store.sync_episodes(show, &feed.episodes, now).unwrap();
        assert_eq!(stats.added, feed.episodes.len(), "{name}");
        assert_eq!(
            store.sync_episodes(show, &feed.episodes, now).unwrap(),
            Default::default(),
            "{name}: a second sync of the same feed changes nothing"
        );
    }
}

/// Parse full, untrimmed feeds from a local folder:
/// `BEACON_REAL_FEEDS=/path/to/feeds cargo test -p beacon-core -- --ignored`.
/// Not run in CI, which never calls real podcast servers.
#[test]
#[ignore = "needs BEACON_REAL_FEEDS pointing at downloaded feeds"]
fn full_real_feeds() {
    let dir = std::env::var("BEACON_REAL_FEEDS").expect("set BEACON_REAL_FEEDS");
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "xml"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty());
    for path in paths {
        let name = path.file_name().unwrap().to_string_lossy();
        let feed = parse_feed(&std::fs::read(&path).unwrap())
            .unwrap_or_else(|err| panic!("{name}: {err}"));
        let total = feed.episodes.len();
        let count = |f: &dyn Fn(&beacon_core::model::EpisodeInfo) -> bool| {
            feed.episodes.iter().filter(|e| f(&e.info)).count()
        };
        let dated = count(&|e| e.published.is_some());
        let audio = count(&|e| e.enclosure.is_some());
        let timed = count(&|e| e.duration.is_some());
        let seasons = count(&|e| e.season.is_some());
        let chapters = feed
            .episodes
            .iter()
            .filter(|e| !e.chapters.is_empty() || e.info.chapters_url.is_some())
            .count();
        let mut guids: Vec<_> = feed.episodes.iter().map(|e| &e.info.guid).collect();
        guids.sort();
        guids.dedup();
        println!(
            "{name}: {total} episodes, dated {dated}, audio {audio}, duration {timed}, \
             seasons {seasons}, chapters {chapters}, unique ids {}",
            guids.len()
        );
        assert_eq!(dated, total, "{name}: every episode has a date");
        assert_eq!(guids.len(), total, "{name}: identities are unique");
    }
}
