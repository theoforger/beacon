// SPDX-License-Identifier: GPL-3.0-or-later

//! The Queue section, laid out as the mockup's: Now Playing, then Next in
//! Queue with its count and total time, then Next from the list the playing
//! episode was started from (at most ten). Each group is a card under its
//! heading in one list view (`ui/episode_list.rs`), as New's groups are.
//! What follows is worked out in `beacon_core::queue`.

use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gettextrs::{gettext, ngettext};

use adw::subclass::prelude::*;
use gtk::glib;

use crate::navigation::Location;
use crate::ui::episode_list::{Entry, EpisodeList};
use crate::ui::format;
use crate::ui::labels;
use crate::ui::messages::Output;
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

mod imp {
    use std::cell::OnceCell;

    use super::*;

    #[derive(Default)]
    pub struct QueueView {
        pub parts: OnceCell<super::Parts>,
        pub output: Output<QueueViewOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for QueueView {
        const NAME: &'static str = "BeaconQueueView";
        type Type = super::QueueView;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for QueueView {}
    impl WidgetImpl for QueueView {}
    impl BinImpl for QueueView {}
}

glib::wrapper! {
    pub struct QueueView(ObjectSubclass<imp::QueueView>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

pub struct Parts {
    selection: Selection,
    list: EpisodeList,
    stack: gtk::Stack,
}

impl QueueView {
    /// The sections' selection, whose bar is in the content header.
    pub fn new(selection: Selection) -> Self {
        let view: Self = glib::Object::new();
        let output = view.imp().output.clone();
        let rows = RowSetup {
            on_action: Rc::new(move |item, action| {
                output.emit(QueueViewOutput::Action(Box::new(item.clone()), action));
            }),
            show_identity: true,
            selection: selection.clone(),
            selectable: true,
            actions: true,
        };
        // Translators: how a screen reader names the Queue section's list.
        let list = EpisodeList::new(&rows, &gettext("Queue"), None, None);
        let empty = adw::StatusPage::builder()
            .icon_name(labels::icon(Location::Queue))
            .title(gettext("Queue Is Empty"))
            .description(gettext("Add episodes to the queue to play them next"))
            .build();
        let stack = gtk::Stack::builder().vexpand(true).build();
        stack.add_named(list.widget(), Some("list"));
        stack.add_named(&empty, Some("empty"));
        stack.set_visible_child_name("empty");
        view.set_child(Some(&stack));
        let _ = view.imp().parts.set(Parts {
            selection,
            list,
            stack,
        });
        view
    }

    pub fn connect_output(&self, receive: impl Fn(QueueViewOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    pub fn send(&self, input: QueueViewInput) {
        let parts = self.imp().parts.get().expect("set when made");
        let QueueViewInput::Set(data) = input;
        let entries = entries(&data);
        let listed = data
            .now_playing
            .iter()
            .chain(&data.queued)
            .chain(&data.from_source)
            .cloned()
            .collect();
        parts.selection.set_items_for(Location::Queue, listed);
        parts
            .stack
            .set_visible_child_name(if entries.is_empty() { "empty" } else { "list" });
        parts.list.set(entries);
    }
}

/// The groups as list entries, with New's spacing: 32px above the first
/// heading, 24px between groups, 32px at the end. Nothing when nothing
/// plays and nothing is queued.
fn entries(data: &QueueData) -> Vec<Entry> {
    // Each group's title and episodes, and whether they can be reordered:
    // only the queue's.
    let mut groups: Vec<(String, Vec<EpisodeItem>, bool)> = Vec::new();
    if let Some(now) = &data.now_playing {
        groups.push((gettext("Now Playing"), vec![now.clone()], false));
    }
    if !data.queued.is_empty() {
        groups.push((queued_title(&data.queued), data.queued.clone(), true));
    }
    if let (false, Some(name)) = (data.from_source.is_empty(), &data.source_name) {
        groups.push((
            gettext("Next from {list}").replace("{list}", name),
            data.from_source.clone(),
            false,
        ));
    }
    if groups.is_empty() || (data.now_playing.is_none() && data.queued.is_empty()) {
        return Vec::new();
    }
    let mut entries = Vec::new();
    for (i, (title, items, movable)) in groups.iter().enumerate() {
        entries.push(Entry::Heading {
            title: title.clone(),
            fold: None,
            top: if i == 0 { 32 } else { 24 },
        });
        if *movable {
            entries.push(Entry::QueueCard {
                items: items.clone(),
                // Translators: how a screen reader names the queued
                // episodes' card in the Queue.
                label: gettext("Next in Queue"),
            });
        } else {
            entries.extend(Entry::card(items));
        }
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
    fn only_the_queue_can_be_reordered() {
        let data = QueueData {
            now_playing: Some(episode(1, 30)),
            queued: vec![episode(2, 45), episode(3, 80)],
            from_source: vec![episode(4, 10)],
            source_name: Some("New".into()),
            next: None,
        };
        let shown: Vec<_> = entries(&data)
            .iter()
            .filter_map(|e| match e {
                Entry::Episode { item, .. } => Some(vec![item.episode.id.0]),
                Entry::QueueCard { items, .. } => {
                    Some(items.iter().map(|i| i.episode.id.0).collect())
                }
                _ => None,
            })
            .collect();
        // The queue as one card; the others as rows of their own.
        assert_eq!(shown, [vec![1], vec![2, 3], vec![4]]);
        assert!(
            entries(&data)
                .iter()
                .any(|e| matches!(e, Entry::QueueCard { items, .. } if items.len() == 2))
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
