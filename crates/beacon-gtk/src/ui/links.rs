// SPDX-License-Identifier: GPL-3.0-or-later

//! Keyboard movement between the links of a label. GTK moves between them
//! with Tab and Shift+Tab only (`gtk_label_focus`); descriptions are full of
//! links, so the arrow keys do it too.

use gtk::{self, gdk, glib, prelude::*};

/// Let Left and Up go to the previous link of `label`, Right and Down to the
/// next. Past the first or last link the arrow does what it would anyway.
pub fn arrow_keys_between_links(label: &gtk::Label) {
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(|keys, key, _, modifiers| {
        if !modifiers.is_empty() {
            return glib::Propagation::Proceed;
        }
        let direction = match key {
            gdk::Key::Left | gdk::Key::Up => gtk::DirectionType::TabBackward,
            gdk::Key::Right | gdk::Key::Down => gtk::DirectionType::TabForward,
            _ => return glib::Propagation::Proceed,
        };
        match keys.widget() {
            Some(label) if label.child_focus(direction) => glib::Propagation::Stop,
            _ => glib::Propagation::Proceed,
        }
    });
    label.add_controller(keys);
}

/// `markup` with its links as plain spans: a label handles `<a>`, but
/// Pango's own markup, which measuring uses, rejects it and lays out
/// nothing.
pub fn without_links(markup: &str) -> String {
    let mut out = String::with_capacity(markup.len());
    let mut rest = markup;
    while let Some(start) = rest.find("<a ") {
        out.push_str(&rest[..start]);
        match rest[start..].find('>') {
            Some(end) => {
                out.push_str("<span>");
                rest = &rest[start + end + 1..];
            }
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out.replace("</a>", "</span>")
}

#[cfg(test)]
mod without_links_tests {
    use super::without_links;

    #[test]
    fn links_become_spans_for_measuring() {
        assert_eq!(
            without_links(
                "Site <a href=\"https://a.example/?x=1&amp;y=2\">a.example</a> and <b>more</b>"
            ),
            "Site <span>a.example</span> and <b>more</b>"
        );
        assert!(
            gtk::pango::parse_markup(&without_links("<a href=\"https://a.example\">a</a>"), '\0')
                .is_ok()
        );
    }
}
