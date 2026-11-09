// SPDX-License-Identifier: GPL-3.0-or-later

//! A show's full description, opened by Read More on its page when the page
//! shows only the start of it.

use adw::prelude::*;
use gtk::{self, pango};

use crate::ui::links;

/// The dialog's width where the window allows it.
const WIDTH: i32 = 560;

/// How tall `label`, with its margins, is in a dialog `width` wide.
pub(crate) fn text_height(label: &gtk::Label, width: i32) -> i32 {
    let inner = width - label.margin_start() - label.margin_end();
    label.measure(gtk::Orientation::Vertical, inner).1 + label.margin_top() + label.margin_bottom()
}

/// Show `markup` (Pango markup from `text::markup`) in a dialog titled
/// `title`, over `parent`.
pub fn present(parent: &impl IsA<gtk::Widget>, title: &str, markup: &str) {
    let label = gtk::Label::builder()
        .use_markup(true)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .xalign(0.0)
        .valign(gtk::Align::Start)
        .margin_top(6)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();
    label.set_markup(markup);
    links::arrow_keys_between_links(&label);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .child(&label)
        .build();
    let header = adw::HeaderBar::new();
    let content = adw::ToolbarView::new();
    content.add_top_bar(&header);
    content.set_content(Some(&scrolled));

    // As tall as the text at the width it gets: left to itself, the dialog
    // measured the text as one long line, so the narrow layout's bottom
    // sheet showed a few lines to scroll through. The window caps it.
    let width = parent
        .as_ref()
        .root()
        .map_or(WIDTH, |r| r.width().min(WIDTH));
    let height = text_height(&label, width) + header.measure(gtk::Orientation::Vertical, width).1;
    let dialog = adw::Dialog::builder()
        .title(title)
        .content_width(WIDTH)
        .content_height(height)
        .child(&content)
        .build();
    dialog.present(Some(parent));
}
