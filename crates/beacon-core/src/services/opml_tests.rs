// SPDX-License-Identifier: GPL-3.0-or-later

use chrono::TimeZone;

use super::*;
use crate::model::{ShowId, ShowInfo};

fn url(s: &str) -> Url {
    Url::parse(s).unwrap()
}

fn feed(address: &str, title: Option<&str>) -> OpmlFeed {
    OpmlFeed {
        url: url(address),
        title: title.map(str::to_owned),
    }
}

fn show(id: i64, title: &str, feed: &str, link: Option<&str>) -> Show {
    Show {
        id: ShowId(id),
        feed_url: url(feed),
        info: ShowInfo {
            title: title.into(),
            author: None,
            description: None,
            link: link.map(url),
            image_url: None,
            category: None,
        },
        cover_path: None,
        subscribed_at: Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap(),
        last_refreshed_at: None,
    }
}

#[test]
fn nested_outlines_are_flattened_in_order() {
    let xml = r#"<opml version="1.0"><body>
        <outline text="feeds">
          <outline type="rss" text="One" xmlUrl="https://a.example/1"/>
          <outline text="Comedy">
            <outline type="rss" text="Two" xmlUrl="https://a.example/2"/>
          </outline>
        </outline>
        <outline type="rss" text="Three" xmlUrl="https://a.example/3"/>
      </body></opml>"#;
    let names: Vec<_> = parse(xml)
        .unwrap()
        .iter()
        .map(|f| f.name().to_owned())
        .collect();
    assert_eq!(names, ["One", "Two", "Three"]);
}

#[test]
fn attribute_names_are_read_in_any_letter_case() {
    let xml = r#"<OPML><Body>
        <outline TEXT="Upper" XMLURL="https://a.example/1"/>
        <outline text="Lower" xmlurl="https://a.example/2"/>
        <Outline Title="Mixed" XmlUrl="https://a.example/3"/>
      </Body></OPML>"#;
    let feeds = parse(xml).unwrap();
    assert_eq!(
        feeds,
        [
            feed("https://a.example/1", Some("Upper")),
            feed("https://a.example/2", Some("Lower")),
            feed("https://a.example/3", Some("Mixed")),
        ]
    );
}

#[test]
fn the_name_is_the_title_else_the_text() {
    // gPodder puts the description in `text`.
    let xml = r#"<opml version="2.0"><body>
        <outline title="Small Hours" text="A show about nights" xmlUrl="https://a.example/1"/>
        <outline text="Only Text" xmlUrl="https://a.example/2"/>
        <outline title="" text="Empty Title" xmlUrl="https://a.example/3"/>
        <outline xmlUrl="https://a.example/4"/>
      </body></opml>"#;
    let feeds = parse(xml).unwrap();
    let titles: Vec<_> = feeds.iter().map(|f| f.title.as_deref()).collect();
    assert_eq!(
        titles,
        [
            Some("Small Hours"),
            Some("Only Text"),
            Some("Empty Title"),
            None
        ]
    );
    assert_eq!(feeds[3].name(), "https://a.example/4");
}

#[test]
fn outlines_without_a_feed_address_are_skipped() {
    // Folders, Overcast-style episodes and links to web pages.
    let xml = r#"<opml version="2.0"><body>
        <outline text="Folder"/>
        <outline type="rss" text="Show" xmlUrl="https://a.example/feed">
          <outline type="podcast-episode" title="Episode" url="https://a.example/ep"/>
        </outline>
        <outline type="link" text="Site" url="https://a.example/"/>
        <outline text="Blank" xmlUrl="  "/>
        <outline text="Mail" xmlUrl="mailto:someone@example.com"/>
      </body></opml>"#;
    assert_eq!(
        parse(xml).unwrap(),
        [feed("https://a.example/feed", Some("Show"))]
    );
}

#[test]
fn escaped_text_and_whitespace_are_read() {
    let xml = "\u{feff}<?xml version=\"1.0\"?>\n<!-- exported -->\n<opml version=\"2.0\"><head><title>x</title></head><body>\
        <outline text=\" Salt &amp; Pepper \" xmlUrl=\"  https://A.Example/feed?a=1&amp;b=2  \"/>\
        </body></opml>";
    assert_eq!(
        parse(xml).unwrap(),
        [feed(
            "https://a.example/feed?a=1&b=2",
            Some("Salt & Pepper")
        )]
    );
}

#[test]
fn files_with_no_version_or_no_feeds_still_read() {
    assert_eq!(parse("<opml><body/></opml>").unwrap(), []);
    assert_eq!(
        parse(r#"<opml version="2.0"><head/><body></body></opml>"#).unwrap(),
        []
    );
}

#[test]
fn bad_xml_is_an_error() {
    for xml in [
        r#"<opml version="2.0"><body><outline text="a""#,
        r#"<opml version="2.0"><body><outline text="a" xmlUrl="https://a.example/"></body></opml>"#,
        r#"<opml version="2.0"><body>"#,
        "",
    ] {
        let result = parse(xml);
        assert!(
            matches!(result, Err(OpmlError::Xml(_) | OpmlError::NotOpml)),
            "{xml:?} gave {result:?}"
        );
    }
    assert!(matches!(
        parse(r#"<opml version="2.0"><body></opml>"#),
        Err(OpmlError::Xml(_))
    ));
}

#[test]
fn other_xml_is_not_opml() {
    let rss =
        r#"<?xml version="1.0"?><rss version="2.0"><channel><title>A</title></channel></rss>"#;
    assert!(matches!(parse(rss), Err(OpmlError::NotOpml)));
    assert!(matches!(parse("just text"), Err(OpmlError::NotOpml)));
}

#[test]
fn addresses_are_normalised() {
    let cases = [
        (
            "  https://Feeds.Example.COM/Show  ",
            Some("https://feeds.example.com/Show"),
        ),
        (
            "http://example.com/feed#top",
            Some("http://example.com/feed"),
        ),
        ("HTTPS://example.com/feed", Some("https://example.com/feed")),
        ("feed://example.com/rss", Some("http://example.com/rss")),
        (
            "feed:https://example.com/rss",
            Some("https://example.com/rss"),
        ),
        ("itpc://example.com/rss", Some("http://example.com/rss")),
        ("pcast://example.com/rss", Some("http://example.com/rss")),
        (
            "https://example.com:443/rss",
            Some("https://example.com/rss"),
        ),
        (
            "https://example.com:8443/rss",
            Some("https://example.com:8443/rss"),
        ),
        ("ftp://example.com/rss", None),
        ("javascript:alert(1)", None),
        ("example.com/rss", None),
        ("https://", None),
        ("", None),
    ];
    for (raw, expected) in cases {
        assert_eq!(
            normalise(raw).as_ref().map(Url::as_str),
            expected,
            "normalising {raw:?}"
        );
    }
}

#[test]
fn repeats_within_the_file_are_left_out() {
    let feeds = vec![
        feed("https://a.example/1", Some("First")),
        feed("https://a.example/2", None),
        feed("http://a.example/1/", Some("Again")),
        feed("https://a.example/2", None),
    ];
    let plan = plan(feeds, &[]);
    assert_eq!(
        plan.feeds,
        [
            feed("https://a.example/1", Some("First")),
            feed("https://a.example/2", None)
        ]
    );
    assert_eq!((plan.subscribed, plan.repeated), (0, 2));
}

#[test]
fn feeds_already_subscribed_to_are_left_out() {
    let feeds = vec![
        feed("https://a.example/1", None),
        feed("https://a.example/2", None),
        feed("https://a.example/3?key=x", None),
    ];
    let subscribed = [url("http://A.example/1/"), url("https://a.example/3?key=x")];
    let plan = plan(feeds, &subscribed);
    assert_eq!(plan.feeds, [feed("https://a.example/2", None)]);
    assert_eq!((plan.subscribed, plan.repeated), (2, 0));
}

#[test]
fn different_queries_are_different_feeds() {
    let feeds = vec![
        feed("https://a.example/rss?show=1", None),
        feed("https://a.example/rss?show=2", None),
    ];
    assert_eq!(plan(feeds, &[]).feeds.len(), 2);
}

#[test]
fn an_export_is_opml_2_with_a_head_and_rss_outlines() {
    let shows = [
        show(
            1,
            "Small Hours",
            "https://a.example/small",
            Some("https://small.example/"),
        ),
        show(2, "The Long Signal", "https://a.example/long", None),
    ];
    let now = Utc.with_ymd_and_hms(2026, 11, 12, 9, 30, 0).unwrap();
    let xml = write(&shows, now);
    assert!(xml.starts_with(r#"<?xml version="1.0" encoding="UTF-8"?>"#));
    assert!(xml.contains(r#"<opml version="2.0">"#));
    assert!(xml.contains("<title>Beacon Subscriptions</title>"));
    assert!(xml.contains("<dateCreated>Thu, 12 Nov 2026 09:30:00 +0000</dateCreated>"));
    assert!(xml.contains(
        r#"<outline type="rss" text="Small Hours" title="Small Hours" xmlUrl="https://a.example/small" htmlUrl="https://small.example/"/>"#
    ));
    assert!(xml.contains(
        r#"<outline type="rss" text="The Long Signal" title="The Long Signal" xmlUrl="https://a.example/long"/>"#
    ));
}

#[test]
fn an_export_reads_back_with_nothing_lost() {
    let shows = [
        show(
            1,
            "Salt & \"Pepper\" <Live>",
            "https://a.example/feed?a=1&b=2",
            None,
        ),
        show(
            2,
            "Ünïcødé 播客",
            "https://b.example/rss",
            Some("https://b.example/"),
        ),
        show(3, "Plain", "http://c.example:8080/feed.xml", None),
    ];
    let xml = write(&shows, Utc::now());
    let read: Vec<_> = parse(&xml)
        .unwrap()
        .into_iter()
        .map(|f| (f.url, f.title.unwrap()))
        .collect();
    let expected: Vec<_> = shows
        .iter()
        .map(|s| (s.feed_url.clone(), s.info.title.clone()))
        .collect();
    assert_eq!(read, expected);
}

#[test]
fn an_empty_library_exports_a_file_that_reads_back() {
    assert_eq!(parse(&write(&[], Utc::now())).unwrap(), []);
}

#[test]
fn addresses_that_may_hold_a_secret_are_noticed() {
    for secret in [
        "https://user:pass@a.example/feed",
        "https://www.patreon.com/rss/show?auth=abc123",
        "https://a.example/feed?api_key=x",
        "https://a.example/feed?accessToken=x",
        "https://a.example/feed?format=rss&Token=x",
        "https://a.example/private/0a1b2c3d-4e5f-6a7b-8c9d-0e1f2a3b4c5d/feed",
        "https://a.example/token/8f3k2j9d0s7a6h5g4f3d.rss",
    ] {
        assert!(may_hold_secret(&url(secret)), "{secret}");
    }
    for public in [
        "https://feeds.simplecast.com/cYQVc__c",
        "https://rss.art19.com/hello-from-magic-tavern",
        "https://feeds.megaphone.fm/ADV1234567890",
        // Shows named by UUID.
        "https://a.example/show/0a1b2c3d-4e5f-6a7b-8c9d-0e1f2a3b4c5d/feed.xml",
        "https://a.example/8f3k2j9d0s7a6h5g4f3d.rss",
        // Tracking and format parameters.
        "https://a.example/feed?utm_source=opml&utm_campaign=x7Hq2kLm9Pz",
        "https://a.example/rss?format=xml&show=2",
        // Words that only contain a credential's name.
        "https://a.example/authors/jane/feed?monkey=1",
        "https://a.example/keynotes/feed",
        "https://a.example/feeds/private",
    ] {
        assert!(!may_hold_secret(&url(public)), "{public}");
    }
}
