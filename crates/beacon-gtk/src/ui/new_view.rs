// SPDX-License-Identifier: GPL-3.0-or-later

//! The New section: recent episodes of every show, one boxed list per
//! recency group under its heading (as GNOME Podcasts' home view). The
//! content header's filter button sorts and filters it, as on podcast pages.

use std::rc::Rc;

use beacon_core::browse::RecencyGroup;
use beacon_core::model::{DownloadFilter, EpisodeFilter, EpisodeId, ShowPrefs};
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::{Component, ComponentParts, ComponentSender, adw};

use crate::ui::episode_list::{LazyFill, ids, new_list, update_in_place};
use crate::ui::format;
use crate::ui::models::{EpisodeItem, RowAction};
use crate::ui::widgets::episode_row::OnRowAction;

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
    on_action: OnRowAction,
    fill: LazyFill,
    /// What the lists show now: each group and its episodes.
    shown: Vec<(RecencyGroup, Vec<EpisodeId>)>,
    lists: Vec<gtk::ListBox>,
}

#[relm4::component(pub)]
impl Component for NewView {
    type Init = ();
    type Input = NewViewInput;
    type Output = NewViewOutput;
    type CommandOutput = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            #[name = "stack"]
            gtk::Stack {
                set_vexpand: true,

                add_named[Some("list")] = &gtk::ScrolledWindow {
                    set_hscrollbar_policy: gtk::PolicyType::Never,

                    adw::Clamp {
                        set_maximum_size: 900,

                        #[name = "groups"]
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 6,
                            set_margin_top: 6,
                            set_margin_bottom: 24,
                            set_margin_start: 12,
                            set_margin_end: 12,
                        },
                    },
                },

                #[name = "empty"]
                add_named[Some("empty")] = &adw::StatusPage {
                    set_icon_name: Some("view-list-bullet-symbolic"),
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let on_action: OnRowAction = Rc::new(move |item, action| {
            let _ = sender.output(NewViewOutput::Action(Box::new(item.clone()), action));
        });
        let model = NewView {
            on_action,
            fill: LazyFill::default(),
            shown: Vec::new(),
            lists: Vec::new(),
        };
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

        let groups = group(items);
        let shape: Vec<_> = groups.iter().map(|(g, items)| (*g, ids(items))).collect();
        let all: Vec<&EpisodeItem> = groups.iter().flat_map(|(_, items)| items).collect();
        if shape == self.shown && update_in_place(&self.lists, &all) {
            return;
        }

        // The episodes changed: rebuild the groups and fill them lazily.
        self.fill.cancel();
        while let Some(child) = widgets.groups.first_child() {
            widgets.groups.remove(&child);
        }
        self.lists.clear();
        let mut work = Vec::new();
        for (group, items) in groups {
            let title = format::recency_title(group);
            let heading = gtk::Label::builder()
                .label(&title)
                .xalign(0.0)
                .margin_top(12)
                .css_classes(["heading"])
                .accessible_role(gtk::AccessibleRole::Heading)
                .build();
            let list = new_list(&self.on_action, &title);
            widgets.groups.append(&heading);
            widgets.groups.append(&list);
            work.extend(items.into_iter().map(|item| (list.clone(), item)));
            self.lists.push(list);
        }
        self.shown = shape;
        self.fill.start(work, self.on_action.clone(), true);
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
