// SPDX-License-Identifier: GPL-3.0-or-later

//! OPML import and export (`docs/specs/opml.md`).
//!
//! Reading walks every `outline` however deeply nested, reads attribute
//! names in any letter case and takes no notice of the version or of
//! anything else it does not know, since every podcast app writes OPML its
//! own way. Writing is OPML 2.0, one `outline type="rss"` per show.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use quick_xml::events::{BytesDecl, BytesText, Event};
use quick_xml::{Reader, Writer, XmlVersion};
use url::Url;

use crate::error::OpmlError;
use crate::model::Show;

/// A feed an OPML file lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpmlFeed {
    pub url: Url,
    /// The name the file gives it, if any, to show until the feed's own
    /// title is known.
    pub title: Option<String>,
}

impl OpmlFeed {
    /// What to call the feed: its name in the file, else its address.
    pub fn name(&self) -> &str {
        self.title.as_deref().unwrap_or(self.url.as_str())
    }
}

/// What an import will do with a file's feeds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportPlan {
    /// The feeds to subscribe to, in the file's order.
    pub feeds: Vec<OpmlFeed>,
    /// Feeds left out because the library has them already.
    pub subscribed: usize,
    /// Feeds left out because the file lists them more than once.
    pub repeated: usize,
}

/// The feeds `xml` lists, in order, nested outlines flattened. An outline
/// with no usable `xmlUrl` is a folder, an episode or a link, and is
/// skipped. Fails only when `xml` is not well formed, or not OPML.
pub fn parse(xml: &str) -> Result<Vec<OpmlFeed>, OpmlError> {
    let mut reader = Reader::from_str(xml.trim_start_matches('\u{feff}'));
    let mut feeds = Vec::new();
    let mut depth = 0usize;
    let mut root = false;
    loop {
        let event = reader
            .read_event()
            .map_err(|err| OpmlError::Xml(err.to_string()))?;
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                let name = element.local_name();
                let name: &str = name.as_ref();
                if depth == 0 {
                    if root || !name.eq_ignore_ascii_case("opml") {
                        return Err(OpmlError::NotOpml);
                    }
                    root = true;
                }
                if name.eq_ignore_ascii_case("outline")
                    && let Some(feed) = outline(element)?
                {
                    feeds.push(feed);
                }
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                }
            }
            Event::End(_) => depth = depth.saturating_sub(1),
            Event::Eof if depth > 0 => {
                return Err(OpmlError::Xml("the file ends inside an element".into()));
            }
            Event::Eof if !root => return Err(OpmlError::NotOpml),
            Event::Eof => return Ok(feeds),
            _ => {}
        }
    }
}

/// The feed an `outline` element stands for, if it has an address.
fn outline(element: &quick_xml::events::BytesStart) -> Result<Option<OpmlFeed>, OpmlError> {
    let mut url = None;
    let mut title = None;
    let mut text = None;
    for attribute in element.attributes().with_checks(false) {
        let attribute = attribute.map_err(|err| OpmlError::Xml(err.to_string()))?;
        let key = attribute.key.local_name();
        let key = AsRef::<str>::as_ref(&key).to_ascii_lowercase();
        let slot = match key.as_str() {
            "xmlurl" => &mut url,
            "title" => &mut title,
            "text" => &mut text,
            _ => continue,
        };
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|err| OpmlError::Xml(err.to_string()))?;
        *slot = Some(value.trim().to_owned());
    }
    let Some(url) = url.as_deref().and_then(normalise) else {
        return Ok(None);
    };
    // `title` first: gPodder writes the show's description in `text`.
    let title = [title, text].into_iter().flatten().find(|t| !t.is_empty());
    Ok(Some(OpmlFeed { url, title }))
}

/// A feed address as Beacon keeps it: trimmed, the host in lower case
/// (which `Url` does), without a fragment, and the podcast schemes some
/// apps write (`feed:`, `itpc:`, `pcast:`, `podcast:`) turned into HTTP.
/// `None` for anything that is not a web address.
pub fn normalise(raw: &str) -> Option<Url> {
    let raw = raw.trim();
    let (scheme, rest) = raw.split_once(':')?;
    let raw = match scheme.to_ascii_lowercase().as_str() {
        "http" | "https" => raw.to_owned(),
        "feed" | "itpc" | "pcast" | "podcast" => match rest.trim_start_matches('/') {
            // `feed:https://host/…`
            inner if inner.starts_with("http:") || inner.starts_with("https:") => inner.to_owned(),
            inner => format!("http://{inner}"),
        },
        _ => return None,
    };
    let mut url = Url::parse(&raw).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none_or(str::is_empty) {
        return None;
    }
    url.set_fragment(None);
    Some(url)
}

/// What makes two feed addresses the same feed: everything but the
/// scheme (a feed moved to HTTPS is the same feed) and a trailing slash.
fn same_feed(url: &Url) -> String {
    let path = url.path().trim_end_matches('/');
    let port = url.port().map(|p| format!(":{p}")).unwrap_or_default();
    let query = url.query().map(|q| format!("?{q}")).unwrap_or_default();
    format!("{}{port}{path}{query}", url.host_str().unwrap_or_default())
}

/// Leave out the feeds `subscribed` already has, and those the file
/// repeats, keeping the first.
pub fn plan(feeds: Vec<OpmlFeed>, subscribed: &[Url]) -> ImportPlan {
    let library: HashSet<String> = subscribed.iter().map(same_feed).collect();
    let mut seen = HashSet::new();
    let mut plan = ImportPlan::default();
    for feed in feeds {
        let key = same_feed(&feed.url);
        if library.contains(&key) {
            plan.subscribed += 1;
        } else if !seen.insert(key) {
            plan.repeated += 1;
        } else {
            plan.feeds.push(feed);
        }
    }
    plan
}

/// The title of an exported file.
const EXPORT_TITLE: &str = "Beacon Subscriptions";

/// `shows` as an OPML 2.0 document, created at `now`.
pub fn write(shows: &[Show], now: DateTime<Utc>) -> String {
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
    // Writing to a `Vec` cannot fail.
    let _ = write_document(&mut writer, shows, now);
    let mut xml = String::from_utf8(writer.into_inner()).unwrap_or_default();
    xml.push('\n');
    xml
}

fn write_document(
    writer: &mut Writer<Vec<u8>>,
    shows: &[Show],
    now: DateTime<Utc>,
) -> std::io::Result<()> {
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    writer
        .create_element("opml")
        .with_attribute(("version", "2.0"))
        .write_inner_content(|writer| {
            writer
                .create_element("head")
                .write_inner_content(|writer| {
                    writer
                        .create_element("title")
                        .write_text_content(BytesText::new(EXPORT_TITLE))?;
                    writer
                        .create_element("dateCreated")
                        .write_text_content(BytesText::new(&now.to_rfc2822()))?;
                    Ok(())
                })?;
            writer
                .create_element("body")
                .write_inner_content(|writer| {
                    for show in shows {
                        let title = show.info.title.as_str();
                        let mut outline = writer.create_element("outline").with_attributes([
                            ("type", "rss"),
                            ("text", title),
                            ("title", title),
                            ("xmlUrl", show.feed_url.as_str()),
                        ]);
                        if let Some(link) = &show.info.link {
                            outline = outline.with_attribute(("htmlUrl", link.as_str()));
                        }
                        outline.write_empty()?;
                    }
                    Ok(())
                })?;
            Ok(())
        })?;
    Ok(())
}

#[cfg(test)]
#[path = "opml_tests.rs"]
mod tests;
