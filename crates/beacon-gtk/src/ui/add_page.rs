// SPDX-License-Identifier: GPL-3.0-or-later

//! The Add page, pushed over the current section. One rounded search field
//! that also takes a feed address: pasting one shows an RSS Feed row with
//! Add Feed. Searching podcast directories by name arrives in M6.

use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::{Component, ComponentParts, ComponentSender, adw};
use url::Url;

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
}

pub struct AddPage {
    feed: Option<Url>,
    typed: bool,
    adding: bool,
}

#[relm4::component(pub)]
impl Component for AddPage {
    type Init = ();
    type Input = AddPageInput;
    type Output = AddPageOutput;
    type CommandOutput = ();

    view! {
        adw::NavigationPage {
            set_tag: Some(ADD_TAG),
            set_title: &gettext("Add Podcast"),

            #[wrap(Some)]
            set_child = &adw::ToolbarView {
                #[name = "header"]
                add_top_bar = &adw::HeaderBar {},

                #[wrap(Some)]
                set_content = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,

                    adw::Clamp {
                        set_maximum_size: 600,
                        set_margin_top: 12,
                        set_margin_bottom: 12,
                        set_margin_start: 12,
                        set_margin_end: 12,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 18,

                            #[name = "entry"]
                            gtk::SearchEntry {
                                set_placeholder_text: Some(&gettext("Search or Paste a Feed Address")),
                                update_property: &[gtk::accessible::Property::Label(&gettext("Search podcasts or paste an RSS feed address"))],
                                connect_search_changed[sender] => move |entry| {
                                    sender.input(AddPageInput::TextChanged(entry.text().to_string()));
                                },
                                connect_activate[sender] => move |_| sender.input(AddPageInput::AddFeed),
                            },

                            gtk::ListBox {
                                add_css_class: "boxed-list",
                                set_selection_mode: gtk::SelectionMode::None,
                                #[watch]
                                set_visible: model.feed.is_some(),

                                adw::ActionRow {
                                    set_title: &gettext("RSS Feed"),
                                    #[watch]
                                    set_subtitle: model.feed.as_ref().map(Url::as_str).unwrap_or_default(),
                                    set_subtitle_selectable: true,

                                    add_suffix = &gtk::Button {
                                        #[watch]
                                        set_label: &if model.adding { gettext("Adding…") } else { gettext("Add Feed") },
                                        #[watch]
                                        set_sensitive: !model.adding,
                                        set_valign: gtk::Align::Center,
                                        add_css_class: "suggested-action",
                                        connect_clicked => AddPageInput::AddFeed,
                                    },
                                },
                            },
                        },
                    },

                    adw::StatusPage {
                        set_vexpand: true,
                        set_icon_name: Some("system-search-symbolic"),
                        #[watch]
                        set_visible: model.feed.is_none(),
                        set_title: &gettext("Search for Podcasts"),
                        #[watch]
                        set_description: Some(&if model.typed {
                            gettext("Searching by name is not available yet; paste the address of the podcast's feed instead")
                        } else {
                            gettext("Find a show by name, or paste the address of its feed")
                        }),
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = AddPage {
            feed: None,
            typed: false,
            adding: false,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match message {
            AddPageInput::TextChanged(text) => {
                self.feed = feed_address(&text);
                self.typed = !text.trim().is_empty();
            }
            AddPageInput::AddFeed => {
                if let (Some(url), false) = (&self.feed, self.adding) {
                    self.adding = true;
                    let _ = sender.output(AddPageOutput::Subscribe(url.clone()));
                }
            }
            AddPageInput::Done => self.adding = false,
        }
    }
}

impl AddPageWidgets {
    pub fn header(&self) -> &adw::HeaderBar {
        &self.header
    }

    pub fn clear(&self) {
        self.entry.set_text("");
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
