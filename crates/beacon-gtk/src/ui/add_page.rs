// SPDX-License-Identifier: GPL-3.0-or-later

//! The Add page, pushed over the current section. One rounded search field
//! that also takes a feed address: pasting one shows an RSS Feed row with
//! Add Feed. Searching podcast directories by name arrives in M7.

use adw::prelude::*;
use gettextrs::gettext;

use adw::subclass::prelude::*;
use gtk::glib;
use url::Url;

use crate::ui::messages::Output;

pub const ADD_TAG: &str = "add";

#[derive(Debug)]
pub enum AddPageInput {
    TextChanged(String),
    AddFeed,
    /// From the window: the subscription finished, either way.
    Done,
}

#[derive(Debug)]
pub enum AddPageOutput {
    Subscribe(Url),
    /// Escape in the empty search field: leave the page.
    Back,
}

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};

    use super::*;

    #[derive(Default)]
    pub struct AddPage {
        pub parts: OnceCell<super::Parts>,
        pub feed: RefCell<Option<Url>>,
        pub typed: Cell<bool>,
        pub adding: Cell<bool>,
        pub output: Output<AddPageOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AddPage {
        const NAME: &'static str = "BeaconAddPage";
        type Type = super::AddPage;
        type ParentType = adw::NavigationPage;
    }

    impl ObjectImpl for AddPage {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }
    impl WidgetImpl for AddPage {}
    impl NavigationPageImpl for AddPage {}
}

glib::wrapper! {
    pub struct AddPage(ObjectSubclass<imp::AddPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

pub struct Parts {
    header: adw::HeaderBar,
    entry: gtk::SearchEntry,
    feed_list: gtk::ListBox,
    feed_row: adw::ActionRow,
    add: gtk::Button,
    status: adw::StatusPage,
}

impl Default for AddPage {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl AddPage {
    fn build(&self) {
        self.set_tag(Some(ADD_TAG));
        self.set_title(&gettext("Add Podcast"));

        let entry = gtk::SearchEntry::builder()
            .placeholder_text(gettext("Search or Paste a Feed Address"))
            .build();
        entry.update_property(&[gtk::accessible::Property::Label(&gettext(
            "Search podcasts or paste an RSS feed address",
        ))]);
        let this = self.downgrade();
        entry.connect_search_changed(move |entry| {
            if let Some(page) = this.upgrade() {
                page.send(AddPageInput::TextChanged(entry.text().to_string()));
            }
        });
        let this = self.downgrade();
        entry.connect_activate(move |_| {
            if let Some(page) = this.upgrade() {
                page.send(AddPageInput::AddFeed);
            }
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

        let add = gtk::Button::builder()
            .valign(gtk::Align::Center)
            .css_classes(["suggested-action"])
            .build();
        let this = self.downgrade();
        add.connect_clicked(move |_| {
            if let Some(page) = this.upgrade() {
                page.send(AddPageInput::AddFeed);
            }
        });
        let feed_row = adw::ActionRow::builder()
            .title(gettext("RSS Feed"))
            .subtitle_selectable(true)
            .build();
        feed_row.add_suffix(&add);
        let feed_list = gtk::ListBox::builder()
            .css_classes(["boxed-list"])
            .selection_mode(gtk::SelectionMode::None)
            .build();
        feed_list.append(&feed_row);

        let column = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .build();
        column.append(&entry);
        column.append(&feed_list);
        let clamp = adw::Clamp::builder()
            .maximum_size(600)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .child(&column)
            .build();
        let status = adw::StatusPage::builder()
            .vexpand(true)
            .icon_name("system-search-symbolic")
            .title(gettext("Search for Podcasts"))
            .build();
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        content.append(&clamp);
        content.append(&status);

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
            feed_list,
            feed_row,
            add,
            status,
        });
        self.refresh();
    }

    pub fn connect_output(&self, receive: impl Fn(AddPageOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    pub fn send(&self, input: AddPageInput) {
        let imp = self.imp();
        match input {
            AddPageInput::TextChanged(text) => {
                imp.feed.replace(feed_address(&text));
                imp.typed.set(!text.trim().is_empty());
            }
            AddPageInput::AddFeed => {
                let feed = imp.feed.borrow().clone();
                if let (Some(url), false) = (feed, imp.adding.get()) {
                    imp.adding.set(true);
                    imp.output.emit(AddPageOutput::Subscribe(url));
                }
            }
            AddPageInput::Done => imp.adding.set(false),
        }
        self.refresh();
    }

    /// Show the feed row, or the status page, as things stand.
    fn refresh(&self) {
        let imp = self.imp();
        let parts = self.parts();
        let feed = imp.feed.borrow();
        let adding = imp.adding.get();
        parts.feed_list.set_visible(feed.is_some());
        parts
            .feed_row
            .set_subtitle(feed.as_ref().map(Url::as_str).unwrap_or_default());
        parts.add.set_label(&if adding {
            gettext("Adding…")
        } else {
            gettext("Add Feed")
        });
        parts.add.set_sensitive(!adding);
        parts.status.set_visible(feed.is_none());
        parts.status.set_description(Some(&if imp.typed.get() {
            gettext(
                "Searching by name is not available yet; paste the address of the podcast's feed instead",
            )
        } else {
            gettext("Find a show by name, or paste the address of its feed")
        }));
    }

    fn parts(&self) -> &Parts {
        self.imp().parts.get().expect("built when constructed")
    }

    pub fn header(&self) -> &adw::HeaderBar {
        &self.parts().header
    }

    pub fn clear(&self) {
        self.parts().entry.set_text("");
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
}
