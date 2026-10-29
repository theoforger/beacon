// SPDX-License-Identifier: GPL-3.0-or-later

//! Plain text from feed descriptions, which are often HTML.
//!
//! A stopgap for M3's short show descriptions: tags are dropped, block
//! breaks become newlines and the common entities are decoded. The details
//! pane (M4) needs real rich text and should use a maintained crate instead.

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

#[cfg(test)]
mod tests {
    use super::*;

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
