// SPDX-License-Identifier: GPL-3.0-or-later

//! The New section: recent episodes of every show, a card per recency group
//! under its heading (as GNOME Podcasts' home view), in one list view
//! (`ui/episode_list.rs`). The
//! content header's filter button sorts and filters it, as on podcast pages.

use std::rc::Rc;

use beacon_core::browse::RecencyGroup;
use beacon_core::model::{DownloadFilter, EpisodeFilter, ShowPrefs};
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::{Component, ComponentParts, ComponentSender, adw};

use crate::ui::episode_list::{Entry, EpisodeList};
use crate::ui::format;
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

pub struct NewView {
    rows: RowSetup,
    list: EpisodeList,
}

#[relm4::component(pub)]
impl Component for NewView {
    /// The section's selection, whose bar is in the content header.
    type Init = Selection;
    type Input = NewViewInput;
    type Output = NewViewOutput;
    type CommandOutput = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            #[name = "stack"]
            gtk::Stack {
                set_vexpand: true,

                add_named[Some("list")] = model.list.widget(),

                #[name = "empty"]
                add_named[Some("empty")] = &adw::StatusPage {
                    set_icon_name: Some("view-list-bullet-symbolic"),
                },
            },
        }
    }

    fn init(
        selection: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let rows = RowSetup {
            on_action: Rc::new(move |item, action| {
                let _ = sender.output(NewViewOutput::Action(Box::new(item.clone()), action));
            }),
            show_identity: true,
            selection,
            selectable: true,
        };
        // Translators: how a screen reader names the New section's list.
        let list = EpisodeList::new(&rows, &gettext("New Episodes"), None, None);
        let model = NewView { rows, list };
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        let NewViewInput::Set {
            items,
            prefs,
            has_shows,
        } = message;

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
        widgets.empty.set_title(&title);
        widgets.empty.set_description(Some(&description));
        widgets
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
        self.rows
            .selection
            .set_items(groups.into_iter().flat_map(|(_, items)| items).collect());
        self.list.set(entries);
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
