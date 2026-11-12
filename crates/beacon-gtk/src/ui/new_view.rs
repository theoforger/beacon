// SPDX-License-Identifier: GPL-3.0-or-later

//! The New section: recent episodes of every show, a card per recency group
//! under its heading (as GNOME Podcasts' home view), in one list view
//! (`ui/episode_list.rs`). The
//! content header's filter button sorts and filters it, as on podcast pages.

use std::rc::Rc;

use adw::prelude::*;
use beacon_core::browse::RecencyGroup;
use beacon_core::model::{DownloadFilter, EpisodeFilter, ShowPrefs};
use gettextrs::gettext;

use adw::subclass::prelude::*;
use gtk::glib;

use crate::navigation::Location;
use crate::ui::episode_list::{Entry, EpisodeList};
use crate::ui::format;
use crate::ui::messages::Output;
use crate::ui::models::{EpisodeItem, RowAction};
use crate::ui::selection::Selection;
use crate::ui::widgets::episode_row::RowSetup;

#[derive(Debug)]
pub enum NewViewInput {
    Set {
        items: Vec<(RecencyGroup, EpisodeItem)>,
        prefs: ShowPrefs,
        has_shows: bool,
    },
}

#[derive(Debug)]
pub enum NewViewOutput {
    Action(Box<EpisodeItem>, RowAction),
}

mod imp {
    use std::cell::OnceCell;

    use super::*;

    #[derive(Default)]
    pub struct NewView {
        pub parts: OnceCell<super::Parts>,
        pub output: Output<NewViewOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NewView {
        const NAME: &'static str = "BeaconNewView";
        type Type = super::NewView;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for NewView {}
    impl WidgetImpl for NewView {}
    impl BinImpl for NewView {}
}

glib::wrapper! {
    pub struct NewView(ObjectSubclass<imp::NewView>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

pub struct Parts {
    rows: RowSetup,
    list: EpisodeList,
    stack: gtk::Stack,
    empty: adw::StatusPage,
}

impl NewView {
    /// The section's selection, whose bar is in the content header.
    pub fn new(selection: Selection) -> Self {
        let view: Self = glib::Object::new();
        let output = view.imp().output.clone();
        let rows = RowSetup {
            on_action: Rc::new(move |item, action| {
                output.emit(NewViewOutput::Action(Box::new(item.clone()), action));
            }),
            show_identity: true,
            selection,
            selectable: true,
            actions: true,
        };
        // Translators: how a screen reader names the New section's list.
        let list = EpisodeList::new(&rows, &gettext("New Episodes"), None, None);
        let empty = adw::StatusPage::builder()
            .icon_name("view-list-bullet-symbolic")
            .build();
        let stack = gtk::Stack::builder().vexpand(true).build();
        stack.add_named(list.widget(), Some("list"));
        stack.add_named(&empty, Some("empty"));
        view.set_child(Some(&stack));
        let _ = view.imp().parts.set(Parts {
            rows,
            list,
            stack,
            empty,
        });
        view
    }

    pub fn connect_output(&self, receive: impl Fn(NewViewOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    pub fn send(&self, input: NewViewInput) {
        let parts = self.imp().parts.get().expect("set when made");
        let NewViewInput::Set {
            items,
            prefs,
            has_shows,
        } = input;

        let only_unplayed =
            prefs.played == EpisodeFilter::Unplayed && prefs.downloaded == DownloadFilter::All;
        let (title, description) = if !has_shows {
            (
                gettext("No Episodes"),
                gettext("Subscribe to a podcast to see its episodes here"),
            )
        } else if only_unplayed {
            (
                gettext("No Unplayed Episodes"),
                gettext("You are all caught up"),
            )
        } else if prefs.is_filtered() {
            (
                gettext("No Matching Episodes"),
                gettext("No recent episodes match these filters"),
            )
        } else {
            (
                gettext("No Recent Episodes"),
                gettext("Episodes from the last month appear here"),
            )
        };
        parts.empty.set_title(&title);
        parts.empty.set_description(Some(&description));
        parts
            .stack
            .set_visible_child_name(if items.is_empty() { "empty" } else { "list" });

        // GNOME Podcasts' spacing (`home_episode_list.ui`): 32px above and
        // below, groups 24px apart, each heading 6px above its card.
        let groups = group(items);
        let mut entries = Vec::new();
        for (i, (group, items)) in groups.iter().enumerate() {
            entries.push(Entry::Heading {
                title: format::recency_title(*group),
                fold: None,
                top: if i == 0 { 32 } else { 24 },
            });
            entries.extend(Entry::card(items));
        }
        entries.push(Entry::Space(32));
        parts.rows.selection.set_items_for(
            Location::New,
            groups.into_iter().flat_map(|(_, items)| items).collect(),
        );
        parts.list.set(entries);
    }
}

/// Consecutive items of the same recency group, in order.
fn group(items: Vec<(RecencyGroup, EpisodeItem)>) -> Vec<(RecencyGroup, Vec<EpisodeItem>)> {
    let mut groups: Vec<(RecencyGroup, Vec<EpisodeItem>)> = Vec::new();
    for (group, item) in items {
        match groups.last_mut() {
            Some((last, items)) if *last == group => items.push(item),
            _ => groups.push((group, vec![item])),
        }
    }
    groups
}
