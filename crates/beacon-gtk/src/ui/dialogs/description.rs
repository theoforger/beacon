// SPDX-License-Identifier: GPL-3.0-or-later

//! A show's full description, opened by Read More on its page when the page
//! shows only the start of it.

use relm4::adw::{self, prelude::*};
use relm4::gtk::{self, pango};

use crate::ui::links;

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
    let content = adw::ToolbarView::new();
    content.add_top_bar(&adw::HeaderBar::new());
    content.set_content(Some(&scrolled));

    let dialog = adw::Dialog::builder()
        .title(title)
        .content_width(560)
        .child(&content)
        .build();
    dialog.present(Some(parent));
}
