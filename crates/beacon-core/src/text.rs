// SPDX-License-Identifier: GPL-3.0-or-later

//! Text from feed descriptions, which are often HTML: plain text, and Pango
//! markup that keeps links, bold, italics, paragraphs and lists, as GNOME
//! Podcasts renders episode descriptions (`episode_description_parser.rs`).
//! The markup is plain text in Pango's format; the core still never touches
//! GTK.

use linkify::{LinkFinder, LinkKind};
use scraper::{ElementRef, Html, Node};

/// Text without tags, entities decoded, whitespace tidied.
pub fn plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        out.push_str(&decode(&rest[..start]));
        let Some(end) = rest[start..].find('>') else {
            // A lone `<` is text.
            out.push_str(&decode(&rest[start..]));
            rest = "";
            break;
        };
        let tag = rest[start + 1..start + end]
            .trim_start_matches('/')
            .to_ascii_lowercase();
        let name = tag
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        if matches!(name, "br" | "p" | "div" | "li" | "h1" | "h2" | "h3" | "h4") {
            out.push('\n');
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(&decode(rest));
    tidy(&out)
}

fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let entity = after
            .find(';')
            .filter(|&end| end <= 10)
            .map(|end| &after[..=end]);
        let decoded = entity.and_then(|e| match e {
            "&amp;" => Some('&'),
            "&lt;" => Some('<'),
            "&gt;" => Some('>'),
            "&quot;" => Some('"'),
            "&apos;" | "&#39;" => Some('\''),
            "&nbsp;" => Some(' '),
            _ => e
                .strip_prefix("&#x")
                .and_then(|n| u32::from_str_radix(n.trim_end_matches(';'), 16).ok())
                .or_else(|| {
                    e.strip_prefix("&#")
                        .and_then(|n| n.trim_end_matches(';').parse().ok())
                })
                .and_then(char::from_u32),
        });
        match (entity, decoded) {
            (Some(entity), Some(c)) => {
                out.push(c);
                rest = &after[entity.len()..];
            }
            _ => {
                out.push('&');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Collapse runs of spaces, keep at most one blank line, trim.
fn tidy(text: &str) -> String {
    let lines: Vec<String> = text
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    let mut out = Vec::new();
    for line in lines {
        if line.is_empty() && out.last().is_none_or(|l: &String| l.is_empty()) {
            continue;
        }
        out.push(line);
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// Pango markup for a description: links (web and mail only), bold,
/// italics, paragraphs as blank lines, line breaks and list items as lines.
/// Addresses in the text become links. Everything else is text, escaped.
pub fn markup(description: &str) -> String {
    let mut out = Markup::default();
    if !looks_like_html(description) {
        // Plain text keeps its own line breaks, which HTML would fold.
        for line in decode(description).lines() {
            if line.trim().is_empty() {
                out.paragraph();
            } else {
                out.line();
                out.text(line, false);
            }
        }
        out.line();
        return out.finish();
    }
    let fragment = Html::parse_fragment(description);
    children(fragment.root_element(), &mut out, false);
    out.finish()
}

fn children(parent: ElementRef<'_>, out: &mut Markup, in_link: bool) {
    for child in parent.children() {
        match child.value() {
            Node::Text(text) => out.text(text, in_link),
            Node::Element(_) => {
                if let Some(element) = ElementRef::wrap(child) {
                    element_markup(element, out, in_link);
                }
            }
            _ => {}
        }
    }
}

fn element_markup(element: ElementRef<'_>, out: &mut Markup, in_link: bool) {
    match element.value().name() {
        "script" | "style" | "head" | "title" => {}
        "a" => match element.value().attr("href").map(str::trim) {
            Some(href) if !in_link && is_safe_link(href) => {
                out.open(&format!("<a href=\"{}\">", escape(href)));
                children(element, out, true);
                out.close("</a>");
            }
            _ => children(element, out, in_link),
        },
        "b" | "strong" => {
            out.open("<b>");
            children(element, out, in_link);
            out.close("</b>");
        }
        "i" | "em" => {
            out.open("<i>");
            children(element, out, in_link);
            out.close("</i>");
        }
        "br" => out.line(),
        "li" => {
            out.line();
            out.text("• ", true);
            children(element, out, in_link);
            out.line();
        }
        "p" | "div" | "ul" | "ol" | "blockquote" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            out.paragraph();
            children(element, out, in_link);
            out.paragraph();
        }
        _ => children(element, out, in_link),
    }
}

/// Whether the description has tags: a `<` starting a tag name, closing
/// tag or comment, unlike "<3" or "a < b".
fn looks_like_html(text: &str) -> bool {
    text.as_bytes().windows(2).any(|pair| {
        pair[0] == b'<' && (pair[1].is_ascii_alphabetic() || matches!(pair[1], b'/' | b'!'))
    })
}

fn is_safe_link(href: &str) -> bool {
    let lower = href.to_ascii_lowercase();
    ["http://", "https://", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
}

/// Text made safe for Pango markup.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Markup being written, with the breaks still owed before the next text.
#[derive(Default)]
struct Markup {
    out: String,
    breaks: usize,
}

impl Markup {
    fn paragraph(&mut self) {
        self.breaks = self.breaks.max(2);
    }

    fn line(&mut self) {
        self.breaks = self.breaks.max(1);
    }

    /// Owed breaks, but none at the very start.
    fn flush(&mut self) {
        if self.breaks > 0 && !self.out.is_empty() {
            let trimmed = self.out.trim_end_matches(' ').len();
            self.out.truncate(trimmed);
            self.out.push_str(&"\n".repeat(self.breaks));
        }
        self.breaks = 0;
    }

    fn at_line_start(&self) -> bool {
        self.breaks > 0 || self.out.is_empty() || self.out.ends_with('\n')
    }

    fn open(&mut self, tag: &str) {
        self.flush();
        self.out.push_str(tag);
    }

    fn close(&mut self, tag: &str) {
        self.out.push_str(tag);
    }

    /// Text with its whitespace collapsed, and addresses linked unless it
    /// is already inside a link. Spaces at its edges stay one space, so
    /// words either side of an inline element keep apart.
    fn text(&mut self, text: &str, in_link: bool) {
        let leading = text.starts_with(char::is_whitespace);
        let trailing = text.ends_with(char::is_whitespace);
        let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if (leading || words.is_empty() && !text.is_empty())
            && !self.at_line_start()
            && !self.out.ends_with(' ')
        {
            self.out.push(' ');
        }
        if words.is_empty() {
            return;
        }
        self.flush();
        if in_link {
            self.out.push_str(&escape(&words));
        } else {
            let mut finder = LinkFinder::new();
            finder.kinds(&[LinkKind::Url, LinkKind::Email]);
            for span in finder.spans(&words) {
                let part = escape(span.as_str());
                match span.kind() {
                    Some(LinkKind::Url) if is_safe_link(span.as_str()) => {
                        self.out.push_str(&format!("<a href=\"{part}\">{part}</a>"));
                    }
                    Some(LinkKind::Email) => {
                        self.out
                            .push_str(&format!("<a href=\"mailto:{part}\">{part}</a>"));
                    }
                    _ => self.out.push_str(&part),
                }
            }
        }
        if trailing {
            self.out.push(' ');
        }
    }

    fn finish(self) -> String {
        self.out.trim().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_keeps_links_and_paragraphs() {
        assert_eq!(
            markup("<p>Notes: <a href=\"https://x.org/a?b=1&amp;c=2\">the site</a></p><p>Two</p>"),
            "Notes: <a href=\"https://x.org/a?b=1&amp;c=2\">the site</a>\n\nTwo"
        );
    }

    #[test]
    fn markup_links_bare_addresses_and_escapes_text() {
        assert_eq!(
            markup("<p>Visit https://example.com/x &amp; write to hi@example.com</p>"),
            "Visit <a href=\"https://example.com/x\">https://example.com/x</a> &amp; write to \
             <a href=\"mailto:hi@example.com\">hi@example.com</a>"
        );
    }

    #[test]
    fn markup_drops_unsafe_links_and_scripts() {
        assert_eq!(
            markup("<a href=\"javascript:alert(1)\">click</a><script>bad()</script> <b>ok</b>"),
            "click <b>ok</b>"
        );
    }

    #[test]
    fn markup_writes_lists_and_breaks_as_lines() {
        assert_eq!(
            markup("<p>Topics:</p><ul><li>One</li><li><i>Two</i></li></ul>Line<br>Next"),
            "Topics:\n\n• One\n• <i>Two</i>\n\nLine\nNext"
        );
    }

    #[test]
    fn markup_keeps_the_lines_of_plain_text() {
        assert_eq!(
            markup("First line\nSecond <3 & https://a.example/p"),
            "First line\nSecond &lt;3 &amp; <a href=\"https://a.example/p\">https://a.example/p</a>"
        );
    }

    #[test]
    fn drops_tags_and_breaks_blocks() {
        assert_eq!(
            plain("<p>Stories from <b>the edges</b>.</p><p>Second   paragraph</p>"),
            "Stories from the edges.\n\nSecond paragraph"
        );
        assert_eq!(plain("One<br/>Two"), "One\nTwo");
        assert_eq!(plain("Plain text stays"), "Plain text stays");
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(
            plain("Ham &amp; eggs &lt;3 &#8217; &#x2014;"),
            "Ham & eggs <3 \u{2019} \u{2014}"
        );
        assert_eq!(plain("AT&T & co"), "AT&T & co");
        assert_eq!(plain("a < b"), "a < b");
    }
}
