// SPDX-License-Identifier: GPL-3.0-or-later

//! The Queue section, laid out as the mockup's: Now Playing, then Next in
//! Queue with its count and total time, then Next from the list the playing
//! episode was started from (at most ten). Each group is a card under its
//! heading in one list view (`ui/episode_list.rs`), as New's groups are.
//! What follows is worked out in `beacon_core::queue`.

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
use crate::ui::queue_data::QueueData;
use crate::ui::selection::Selection;
use crate::ui::widgets::episode_row::RowSetup;

#[derive(Debug)]
pub enum QueueViewInput {
    Set(Box<QueueData>),
}

#[derive(Debug)]
pub enum QueueViewOutput {
    Action(Box<EpisodeItem>, RowAction),
}

pub struct QueueView {
    list: EpisodeList,
}

#[relm4::component(pub)]
impl Component for QueueView {
    type Init = ();
    type Input = QueueViewInput;
    type Output = QueueViewOutput;
    type CommandOutput = ();

    view! {
        #[name = "stack"]
        gtk::Stack {
            set_vexpand: true,

            add_named[Some("list")] = model.list.widget(),

            add_named[Some("empty")] = &adw::StatusPage {
                set_icon_name: Some(labels::icon(Location::Queue)),
                set_title: &gettext("Queue Empty"),
                set_description: Some(&gettext("Episodes added to the queue play next")),
            },
        }
    }

    fn init(_init: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let rows = RowSetup {
            on_action: Rc::new(move |item, action| {
                let _ = sender.output(QueueViewOutput::Action(Box::new(item.clone()), action));
            }),
            show_identity: true,
            // No selection mode here: its bar is New's and the podcast
            // pages'.
            selection: Selection::default(),
            selectable: false,
        };
        // Translators: how a screen reader names the Queue section's list.
        let list = EpisodeList::new(&rows, &gettext("Queue"), None, None);
        let model = QueueView { list };
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
        let QueueViewInput::Set(data) = message;
        let entries = entries(&data);
        widgets
            .stack
            .set_visible_child_name(if entries.is_empty() { "empty" } else { "list" });
        self.list.set(entries);
    }
}

/// The groups as list entries, with New's spacing: 32px above the first
/// heading, 24px between groups, 32px at the end. Nothing when nothing
/// plays and nothing is queued.
fn entries(data: &QueueData) -> Vec<Entry> {
    let mut groups: Vec<(String, Vec<EpisodeItem>)> = Vec::new();
    if let Some(now) = &data.now_playing {
        groups.push((gettext("Now Playing"), vec![now.clone()]));
    }
    if !data.queued.is_empty() {
        groups.push((queued_title(&data.queued), data.queued.clone()));
    }
    if let (false, Some(name)) = (data.from_source.is_empty(), &data.source_name) {
        groups.push((
            gettext("Next from {list}").replace("{list}", name),
            data.from_source.clone(),
        ));
    }
    if groups.is_empty() || (data.now_playing.is_none() && data.queued.is_empty()) {
        return Vec::new();
    }
    let mut entries = Vec::new();
    for (i, (title, items)) in groups.iter().enumerate() {
        entries.push(Entry::Heading {
            title: title.clone(),
            fold: None,
            top: if i == 0 { 32 } else { 24 },
        });
        entries.extend(Entry::card(items));
    }
    entries.push(Entry::Space(32));
    entries
}

/// "Next in Queue • 3 Episodes • 2 h 5 min", as the mockup has it; the
/// time leaves out episodes of unknown length.
fn queued_title(queued: &[EpisodeItem]) -> String {
    let count = queued.len();
    let total: Duration = queued.iter().filter_map(|i| i.episode.info.duration).sum();
    // Translators: the count of queued episodes in the Queue's heading.
    let episodes =
        ngettext("{n} Episode", "{n} Episodes", count as u32).replace("{n}", &count.to_string());
    let mut parts = vec![gettext("Next in Queue"), episodes];
    if !total.is_zero() {
        parts.push(format::length(total));
    }
    parts.join(&gettext(" • "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::episode_actions::item;

    fn episode(id: i64, minutes: u64) -> EpisodeItem {
        let mut item = item();
        item.episode.id = beacon_core::model::EpisodeId(id);
        item.episode.info.duration = Some(Duration::from_secs(minutes * 60));
        item
    }

    fn headings(entries: &[Entry]) -> Vec<String> {
        entries
            .iter()
            .filter_map(|e| match e {
                Entry::Heading { title, .. } => Some(title.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_groups_follow_the_mockup() {
        let data = QueueData {
            now_playing: Some(episode(1, 30)),
            queued: vec![episode(2, 45), episode(3, 80)],
            from_source: vec![episode(4, 10)],
            source_name: Some("New".into()),
            next: None,
        };
        assert_eq!(
            headings(&entries(&data)),
            [
                "Now Playing",
                "Next in Queue • 2 Episodes • 2 h 5 min",
                "Next from New"
            ]
        );
    }

    #[test]
    fn an_empty_queue_with_nothing_playing_is_empty() {
        let data = QueueData {
            from_source: vec![episode(4, 10)],
            source_name: Some("New".into()),
            ..Default::default()
        };
        assert!(entries(&data).is_empty());
        let playing = QueueData {
            now_playing: Some(episode(1, 30)),
            ..Default::default()
        };
        assert_eq!(headings(&entries(&playing)), ["Now Playing"]);
    }
}
