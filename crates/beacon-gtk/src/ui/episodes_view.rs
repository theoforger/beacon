// SPDX-License-Identifier: GPL-3.0-or-later

//! The Favorites and Downloaded sections, as the mockup lays them out: one
//! card of episodes under a heading that counts them ("4 Favorites"; "3
//! Episodes • 2 h 5 min" for downloads), newest first, or a status page
//! when there are none. Selection mode as in New, through the section
//! selection in the content header.

use std::rc::Rc;
use std::time::Duration;

use gettextrs::{gettext, ngettext};
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::{Component, ComponentParts, ComponentSender, adw};

use crate::navigation::Location;
use crate::ui::episode_list::{Entry, EpisodeList};
use crate::ui::format;
use crate::ui::labels;
use crate::ui::models::{EpisodeItem, RowAction};
use crate::ui::selection::Selection;
use crate::ui::widgets::episode_row::RowSetup;

#[derive(Debug)]
pub enum EpisodesViewInput {
    Set(Vec<EpisodeItem>),
}

#[derive(Debug)]
pub enum EpisodesViewOutput {
    Action(Box<EpisodeItem>, RowAction),
}

pub struct EpisodesView {
    section: Location,
    selection: Selection,
    list: EpisodeList,
}

#[relm4::component(pub)]
impl Component for EpisodesView {
    /// Which section (Favorites or Downloaded), and the sections' selection.
    type Init = (Location, Selection);
    type Input = EpisodesViewInput;
    type Output = EpisodesViewOutput;
    type CommandOutput = ();

    view! {
        #[name = "stack"]
        gtk::Stack {
            set_vexpand: true,

            add_named[Some("list")] = model.list.widget(),

            add_named[Some("empty")] = &adw::StatusPage {
                set_icon_name: Some(labels::icon(model.section)),
                set_title: &empty.0,
                set_description: Some(&empty.1),
            },
        }
    }

    fn init(
        (section, selection): Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let rows = RowSetup {
            on_action: Rc::new(move |item, action| {
                let _ = sender.output(EpisodesViewOutput::Action(Box::new(item.clone()), action));
            }),
            show_identity: true,
            selection: selection.clone(),
            selectable: true,
        };
        let list = EpisodeList::new(&rows, &labels::title(section), None, None);
        let empty = empty_page(section);
        let model = EpisodesView {
            section,
            selection,
            list,
        };
        let widgets = view_output!();
        widgets.stack.set_visible_child_name("empty");
        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        let EpisodesViewInput::Set(items) = message;
        widgets
            .stack
            .set_visible_child_name(if items.is_empty() { "empty" } else { "list" });
        let entries = entries(self.section, &items);
        self.selection.set_items_for(self.section, items);
        self.list.set(entries);
    }
}

/// The mockup's empty states.
fn empty_page(section: Location) -> (String, String) {
    match section {
        Location::Downloaded => (
            gettext("No Downloads"),
            gettext("Download an episode to listen offline"),
        ),
        _ => (
            gettext("No Favorites"),
            gettext("Mark an episode as a favorite to find it here"),
        ),
    }
}

/// One card under its counting heading, with New's spacing.
fn entries(section: Location, items: &[EpisodeItem]) -> Vec<Entry> {
    if items.is_empty() {
        return Vec::new();
    }
    let count = items.len();
    let title = match section {
        Location::Downloaded => {
            let total: Duration = items.iter().filter_map(|i| i.episode.info.duration).sum();
            let mut parts = vec![
                ngettext("{n} Episode", "{n} Episodes", count as u32)
                    .replace("{n}", &count.to_string()),
            ];
            if !total.is_zero() {
                parts.push(format::length(total));
            }
            parts.join(&gettext(" • "))
        }
        _ => ngettext("{n} Favorite", "{n} Favorites", count as u32)
            .replace("{n}", &count.to_string()),
    };
    let mut entries = vec![Entry::Heading {
        title,
        fold: None,
        top: 32,
    }];
    entries.extend(Entry::card(items));
    entries.push(Entry::Space(32));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::episode_actions::item;

    fn heading(section: Location, minutes: &[u64]) -> Option<String> {
        let items: Vec<_> = minutes
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let mut item = item();
                item.episode.id = beacon_core::model::EpisodeId(i as i64 + 1);
                item.episode.info.duration = Some(Duration::from_secs(m * 60));
                item
            })
            .collect();
        entries(section, &items).into_iter().find_map(|e| match e {
            Entry::Heading { title, .. } => Some(title),
            _ => None,
        })
    }

    #[test]
    fn headings_count_as_the_mockup_does() {
        assert_eq!(
            heading(Location::Favorites, &[10]),
            Some("1 Favorite".into())
        );
        assert_eq!(
            heading(Location::Favorites, &[10, 20]),
            Some("2 Favorites".into())
        );
        assert_eq!(
            heading(Location::Downloaded, &[45, 80]),
            Some("2 Episodes • 2 h 5 min".into())
        );
        assert_eq!(heading(Location::Downloaded, &[]), None);
    }
}
