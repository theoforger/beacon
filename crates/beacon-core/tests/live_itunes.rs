// SPDX-License-Identifier: GPL-3.0-or-later

//! What Beacon assumes of the real iTunes Search API
//! (`docs/specs/add-and-itunes.md`): podcast results carry `feedUrl`, a
//! 100px thumbnail and a store link; a lookup by ID finds the show; a
//! country without a directory answers 400; a few listed storefronts still
//! answer. Not run in CI's checks, which never call live servers; a
//! scheduled job that cannot block merges runs it:
//!
//! ```sh
//! cargo test -p beacon-core --test live_itunes -- --ignored
//! ```
//!
//! Six calls, well under Apple's 20 a minute.

use beacon_core::ItunesError;
use beacon_core::services::http_client;
use beacon_core::services::itunes::{ItunesClient, has_store};

#[tokio::test]
#[ignore = "calls the live iTunes Search API"]
async fn apples_directory_answers_as_beacon_expects() {
    let itunes = ItunesClient::new(http_client());

    let shows = itunes.search("hey riddle riddle", "US").await.unwrap();
    let show = shows
        .iter()
        .find(|s| s.title == "Hey Riddle Riddle")
        .expect("the show is found, with a feed");
    assert_eq!(show.feed_url.scheme(), "https");
    let artwork = show.artwork.as_ref().expect("a thumbnail");
    assert!(artwork.as_str().contains("100x100"), "{artwork}");
    let store = show.store_url.as_ref().expect("a store link");
    assert_eq!(store.host_str(), Some("podcasts.apple.com"));
    assert!(show.author.is_some() && show.genre.is_some());

    let found = itunes.lookup(show.id, "US").await.unwrap();
    assert_eq!(found.feed_url, show.feed_url);

    assert!(!has_store("KP"));
    assert!(matches!(
        itunes.search("news", "KP").await,
        Err(ItunesError::NoStore(_))
    ));

    for country in ["GB", "DE", "JP"] {
        assert!(has_store(country));
        let shows = itunes.search("news", country).await.unwrap();
        assert!(!shows.is_empty(), "{country} has a directory");
    }
}
