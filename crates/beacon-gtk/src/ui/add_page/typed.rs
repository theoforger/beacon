// SPDX-License-Identifier: GPL-3.0-or-later

//! What the Add page's field holds: words to search for, a feed address,
//! or a link to a show on Apple Podcasts.

use beacon_core::services::itunes::apple_podcasts_id;
use url::Url;

/// What the field holds.
#[derive(Debug, Clone, PartialEq)]
pub enum Typed {
    Empty,
    Feed(Url),
    /// A podcasts.apple.com link, by the show's ID.
    Apple(u64),
    Words(String),
}

/// What `text` is: a link to a show on Apple Podcasts, another web
/// address (a feed), or words to search for.
pub fn classify(text: &str) -> Typed {
    let text = text.trim();
    if text.is_empty() {
        return Typed::Empty;
    }
    match feed_address(text) {
        Some(url) => match apple_podcasts_id(&url) {
            Some(id) => Typed::Apple(id),
            None => Typed::Feed(url),
        },
        None => Typed::Words(text.split_whitespace().collect::<Vec<_>>().join(" ")),
    }
}

/// The key a request's answer comes back under.
pub fn request_key(typed: &Typed) -> Option<String> {
    match typed {
        Typed::Apple(id) => Some(format!("id:{id}")),
        Typed::Words(words) => Some(words.to_lowercase()),
        Typed::Empty | Typed::Feed(_) => None,
    }
}

/// A web address typed or pasted into the field, if it is one.
pub fn feed_address(text: &str) -> Option<Url> {
    let url = Url::parse(text.trim()).ok()?;
    matches!(url.scheme(), "http" | "https")
        .then_some(url)
        .filter(|u| u.host_str().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_feed_addresses() {
        assert!(feed_address(" https://example.com/feed.xml ").is_some());
        assert!(feed_address("http://example.com/rss").is_some());
        assert!(feed_address("the long signal").is_none());
        assert!(feed_address("ftp://example.com/feed").is_none());
        assert!(feed_address("file:///etc/passwd").is_none());
    }

    #[test]
    fn tells_words_from_feeds_and_apple_links() {
        assert_eq!(classify("   "), Typed::Empty);
        assert_eq!(
            classify("  the   long signal "),
            Typed::Words("the long signal".into())
        );
        assert!(matches!(
            classify("https://example.com/feed.xml"),
            Typed::Feed(_)
        ));
        assert_eq!(
            classify("https://podcasts.apple.com/us/podcast/hey-riddle-riddle/id1412570127"),
            Typed::Apple(1412570127)
        );
        // Words differing only in case and spaces ask once.
        assert_eq!(
            request_key(&classify("The  Long Signal")),
            request_key(&classify("the long signal"))
        );
        assert_eq!(request_key(&Typed::Apple(12)).as_deref(), Some("id:12"));
        assert_eq!(request_key(&classify("https://example.com/feed")), None);
    }
}
