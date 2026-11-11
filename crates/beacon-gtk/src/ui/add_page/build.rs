// SPDX-License-Identifier: GPL-3.0-or-later

//! The Add page's widgets: the field and its note, the RSS Feed row, and a
//! stack of status pages and the results list.

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;

use super::{ADD_TAG, AddPage, AddPageInput, AddPageOutput, Parts, SEARCH_DELAY_MS};
use super::{ERROR, FEED, NONE, RESULTS, SEARCHING, START};

impl AddPage {
    pub(super) fn build(&self) {
        self.set_tag(Some(ADD_TAG));
        self.set_title(&gettext("Add Podcast"));

        let entry = gtk::SearchEntry::builder()
            .placeholder_text(gettext("Search or Paste a Feed Address"))
            .search_delay(SEARCH_DELAY_MS)
            .build();
        entry.update_property(&[gtk::accessible::Property::Label(&gettext(
            "Search podcasts or paste an RSS feed address",
        ))]);
        let input = {
            let page = self.downgrade();
            move |message: AddPageInput| {
                if let Some(page) = page.upgrade() {
                    page.send(message);
                }
            }
        };
        entry.connect_search_changed({
            let input = input.clone();
            move |_| input(AddPageInput::TextChanged)
        });
        entry.connect_activate({
            let input = input.clone();
            move |_| input(AddPageInput::Activate)
        });
        // A search entry keeps Escape for itself: clear the field first,
        // then leave the page, as Escape does elsewhere.
        let output = self.imp().output.clone();
        entry.connect_stop_search(move |entry| {
            if entry.text().is_empty() {
                output.emit(AddPageOutput::Back);
            } else {
                entry.set_text("");
            }
        });
        // Where searches go, as GNOME Podcasts says of its platforms; also
        // the results' attribution.
        let note = gtk::Label::builder()
            .label(gettext("Searches are sent to Apple Podcasts"))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label", "caption"])
            .build();

        let add = gtk::Button::builder()
            .valign(gtk::Align::Center)
            .css_classes(["suggested-action"])
            .build();
        add.connect_clicked({
            let input = input.clone();
            move |_| input(AddPageInput::AddFeed)
        });
        let feed_row = adw::ActionRow::builder()
            .title(gettext("RSS Feed"))
            .subtitle_selectable(true)
            .build();
        feed_row.add_suffix(&add);
        let feed_list = boxed_list();
        feed_list.append(&feed_row);

        let column = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .build();
        column.append(&entry);
        column.append(&note);
        feed_list.set_margin_top(12);
        column.append(&feed_list);
        let top = clamp(&column);

        let results = boxed_list();
        // Translators: how a screen reader names the Add page's results.
        results.update_property(&[gtk::accessible::Property::Label(&gettext("Podcasts Found"))]);
        let this = self.downgrade();
        results.connect_row_activated(move |_, row| {
            if let Some(page) = this.upgrade() {
                page.open(row.index());
            }
        });
        let list = clamp(&results);
        list.set_margin_top(6);
        list.set_margin_bottom(24);
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&list)
            .build();

        let status = |icon: &str, title: &str, description: &str| {
            adw::StatusPage::builder()
                .icon_name(icon)
                .title(title)
                .description(description)
                .build()
        };
        let start = status(
            "system-search-symbolic",
            &gettext("Search for Podcasts"),
            &gettext("Find a show by name, or paste the address of its feed"),
        );
        let searching = adw::StatusPage::builder()
            .paintable(&adw::SpinnerPaintable::new(None::<&gtk::Widget>))
            .title(gettext("Searching…"))
            .build();
        let none = status(
            "system-search-symbolic",
            &gettext("No Podcasts Found"),
            &gettext("Try a different search, or paste the address of the podcast's feed"),
        );
        let error = status("network-error-symbolic", &gettext("Could Not Search"), "");

        let stack = gtk::Stack::builder()
            .vexpand(true)
            .transition_type(gtk::StackTransitionType::Crossfade)
            .build();
        stack.add_named(&start, Some(START));
        stack.add_named(&searching, Some(SEARCHING));
        stack.add_named(&scrolled, Some(RESULTS));
        stack.add_named(&none, Some(NONE));
        stack.add_named(&error, Some(ERROR));
        stack.add_named(&gtk::Box::new(gtk::Orientation::Vertical, 0), Some(FEED));

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        content.append(&top);
        content.append(&stack);

        let header = adw::HeaderBar::new();
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&content));
        self.set_child(Some(&toolbar));

        // The page is one search field: type into it as soon as it opens.
        let focus = entry.clone();
        self.connect_shown(move |_| {
            focus.grab_focus();
        });
        let _ = self.imp().parts.set(Parts {
            header,
            entry,
            note,
            feed_list,
            feed_row,
            add,
            stack,
            results,
            error,
        });
        self.refresh();
    }
}

fn boxed_list() -> gtk::ListBox {
    gtk::ListBox::builder()
        .css_classes(["boxed-list"])
        .selection_mode(gtk::SelectionMode::None)
        .build()
}

fn clamp(child: &impl IsA<gtk::Widget>) -> adw::Clamp {
    adw::Clamp::builder()
        .maximum_size(600)
        .margin_top(12)
        .margin_start(12)
        .margin_end(12)
        .child(child)
        .build()
}
