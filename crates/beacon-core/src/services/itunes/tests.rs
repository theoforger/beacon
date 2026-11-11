// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use super::*;

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/itunes")
        .join(name);
    std::fs::read_to_string(path)
        .unwrap()
        .replace("{{BASE}}", "https://feeds.example")
        .into_bytes()
}

#[test]
fn search_urls_ask_for_podcasts_in_one_country() {
    let base = Url::parse(SEARCH_URL).unwrap();
    let url = search_url(&base, "  hey riddle & riddle ", "GB");
    assert_eq!(
        url.as_str(),
        "https://itunes.apple.com/search?term=hey+riddle+%26+riddle&country=GB\
         &media=podcast&entity=podcast&limit=25"
    );
    let url = lookup_url(&Url::parse(LOOKUP_URL).unwrap(), 1412570127, "US");
    assert_eq!(
        url.as_str(),
        "https://itunes.apple.com/lookup?id=1412570127&country=US&entity=podcast"
    );
}

#[test]
fn reads_a_recorded_answer() {
    let shows = parse_results(&fixture("search-recorded.json")).unwrap();
    assert_eq!(shows.len(), 1);
    let show = &shows[0];
    assert_eq!(show.id, 1412570127);
    assert_eq!(show.title, "Hey Riddle Riddle");
    assert_eq!(show.author.as_deref(), Some("Headgum"));
    assert_eq!(
        show.feed_url.as_str(),
        "https://rss.art19.com/hey-riddle-riddle"
    );
    assert!(
        show.artwork
            .as_ref()
            .unwrap()
            .as_str()
            .ends_with("/100x100bb.jpg")
    );
    assert_eq!(show.genre.as_deref(), Some("Leisure"));
    assert_eq!(show.episode_count, Some(715));
    assert_eq!(
        show.latest.unwrap().to_rfc3339(),
        "2026-11-04T09:00:00+00:00"
    );
    assert_eq!(
        show.store_url.as_ref().unwrap().as_str(),
        "https://podcasts.apple.com/us/podcast/hey-riddle-riddle/id1412570127?uo=4"
    );
}

#[test]
fn leaves_out_results_that_cannot_be_subscribed_to() {
    let shows = parse_results(&fixture("search-mixed.json")).unwrap();
    let titles: Vec<_> = shows.iter().map(|s| s.title.as_str()).collect();
    // No feed, the same feed again and an episode are left out.
    assert_eq!(titles, ["Hey Riddle Riddle", "Hello From The Magic Tavern"]);
    let tavern = &shows[1];
    assert_eq!(tavern.author, None);
    assert_eq!(tavern.latest, None);
    assert_eq!(tavern.artwork, None);
    assert!(
        parse_results(&fixture("search-empty.json"))
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        parse_results(b"<html>"),
        Err(ItunesError::Parse(_))
    ));
}

#[test]
fn finds_the_id_in_apple_podcasts_links() {
    let id = |text: &str| apple_podcasts_id(&Url::parse(text).unwrap());
    assert_eq!(
        id("https://podcasts.apple.com/us/podcast/hey-riddle-riddle/id1412570127?uo=4"),
        Some(1412570127)
    );
    assert_eq!(
        id("https://podcasts.apple.com/gb/podcast/id958230465?i=1000"),
        Some(958230465)
    );
    assert_eq!(id("https://itunes.apple.com/podcast/id12"), Some(12));
    assert_eq!(id("https://example.com/podcast/id1412570127"), None);
    assert_eq!(id("https://podcasts.apple.com/us/podcast/no-id"), None);
    assert_eq!(id("https://podcasts.apple.com/us/podcast/idea"), None);
}

#[test]
fn the_country_comes_from_the_setting_then_the_locale() {
    assert_eq!(country_from_locale("en_GB.UTF-8").as_deref(), Some("GB"));
    assert_eq!(country_from_locale("sr_RS@latin").as_deref(), Some("RS"));
    assert_eq!(country_from_locale("pt_br").as_deref(), Some("BR"));
    assert_eq!(country_from_locale("C.UTF-8"), None);
    assert_eq!(country_from_locale("POSIX"), None);
    assert_eq!(country_from_locale("es_419"), None);

    assert_eq!(resolve_country("de", Some("en_GB.UTF-8")), "DE");
    assert_eq!(resolve_country("", Some("en_GB.UTF-8")), "GB");
    // No directory for the setting or the locale's country: the US.
    assert_eq!(resolve_country("KP", Some("ko_KP.UTF-8")), "US");
    assert_eq!(resolve_country("", Some("C.UTF-8")), "US");
    assert_eq!(resolve_country("", None), "US");
}

#[test]
fn every_storefront_is_a_two_letter_code_once() {
    let mut codes: Vec<_> = STOREFRONTS.iter().map(|(code, _)| *code).collect();
    assert!(
        codes
            .iter()
            .all(|c| c.len() == 2 && c.chars().all(|c| c.is_ascii_uppercase()))
    );
    assert!(codes.contains(&FALLBACK_COUNTRY));
    let count = codes.len();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), count);
}

#[test]
fn searches_are_cached_by_their_words() {
    assert_eq!(
        cache_key("  Hey  Riddle ", "US"),
        cache_key("hey riddle", "US")
    );
    assert_ne!(cache_key("hey riddle", "US"), cache_key("hey riddle", "GB"));

    let start = Instant::now();
    let mut state = State::default();
    let shows = parse_results(&fixture("search-mixed.json")).unwrap();
    state.keep("US:riddle".into(), &shows, start);
    assert_eq!(
        state.cached("US:riddle", start + Duration::from_secs(3600)),
        Some(shows)
    );
    assert_eq!(state.cached("US:riddle", start + CACHE_FOR), None);
    assert!(state.cache.is_empty());
}

#[test]
fn the_cache_drops_its_oldest_search_when_full() {
    let start = Instant::now();
    let mut state = State::default();
    for i in 0..CACHE_SIZE + 1 {
        state.keep(
            format!("US:{i}"),
            &[],
            start + Duration::from_secs(i as u64),
        );
    }
    assert_eq!(state.cache.len(), CACHE_SIZE);
    assert!(!state.cache.contains_key("US:0"));
    assert!(state.cache.contains_key(&format!("US:{CACHE_SIZE}")));
}
