// SPDX-License-Identifier: GPL-3.0-or-later

//! The sections with episode lists under the content header: New, Queue,
//! Favorites and Downloaded. They share one selection, whose Select button
//! and bar are the header's (asked by the user 2026-11-08 for Queue,
//! Favorites and Downloaded, as New had), and Queue and Downloaded have a
//! Clear button at the header's start.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beacon_core::model::EpisodeId;
use beacon_core::queue::Source;
use gettextrs::gettext;
use gtk::prelude::*;

use super::ContentOutput;
use crate::navigation::Location;
use crate::ui::episodes_view::{EpisodesView, EpisodesViewInput, EpisodesViewOutput};
use crate::ui::messages::Output;
use crate::ui::models::EpisodeItem;
use crate::ui::new_view::{NewView, NewViewInput, NewViewOutput};
use crate::ui::queue_data::QueueData;
use crate::ui::queue_view::{QueueView, QueueViewInput, QueueViewOutput};
use crate::ui::selection::Selection;
use crate::ui::selection_bar;

pub(super) struct Lists {
    pub(super) new: NewView,
    queue: QueueView,
    favorites: EpisodesView,
    downloaded: EpisodesView,
    pub(super) selection: Selection,
    /// The header's Select, in the sections that have selection mode.
    pub(super) select: gtk::Button,
    /// The header's Clear: Clear Queue, or Delete All Downloads.
    pub(super) clear: gtk::Button,
    showing: Rc<Cell<Location>>,
    /// What each Clear applies to.
    queued: Rc<RefCell<Vec<EpisodeId>>>,
    downloads: Rc<RefCell<Vec<EpisodeId>>>,
}

impl Lists {
    pub(super) fn new(output: &Output<ContentOutput>) -> Self {
        let selection = Selection::default();
        let new = NewView::new(selection.clone());
        new.connect_output({
            let output = output.clone();
            move |message| match message {
                NewViewOutput::Action(item, action) => {
                    output.emit(ContentOutput::Action(item, action, Some(Source::New)));
                }
            }
        });
        let queue = QueueView::new(selection.clone());
        queue.connect_output({
            let output = output.clone();
            move |message| match message {
                // The Queue's rows keep the list the playing one came from.
                QueueViewOutput::Action(item, action) => {
                    output.emit(ContentOutput::Action(item, action, None));
                }
            }
        });
        let list = |section: Location, source: Source| {
            let view = EpisodesView::new(section, selection.clone());
            let output = output.clone();
            view.connect_output(move |message| match message {
                EpisodesViewOutput::Action(item, action) => {
                    output.emit(ContentOutput::Action(item, action, Some(source)));
                }
            });
            view
        };
        let favorites = list(Location::Favorites, Source::Favorites);
        let downloaded = list(Location::Downloaded, Source::Downloaded);

        let clear = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .visible(false)
            .build();
        let showing = Rc::new(Cell::new(Location::Shows));
        let lists = Self {
            new,
            queue,
            favorites,
            downloaded,
            select: selection_bar::select_button(&selection),
            selection,
            clear,
            showing,
            queued: Rc::default(),
            downloads: Rc::default(),
        };
        lists.select.set_visible(false);
        lists
    }

    /// Ask for the section showing's Clear when it is clicked.
    pub(super) fn connect_clear(&self, output: &Output<ContentOutput>) {
        let (output, showing) = (output.clone(), self.showing.clone());
        let (queued, downloads) = (self.queued.clone(), self.downloads.clone());
        self.clear.connect_clicked(move |_| match showing.get() {
            Location::Queue => {
                output.emit(ContentOutput::ClearQueue(queued.borrow().clone()));
            }
            Location::Downloaded => {
                let ids = downloads.borrow().clone();
                output.emit(ContentOutput::DeleteAllDownloads(ids));
            }
            _ => {}
        });
    }

    /// The page for `location`, if it is one of these sections.
    pub(super) fn page(&self, location: Location) -> Option<gtk::Widget> {
        Some(match location {
            Location::New => self.new.clone().upcast(),
            Location::Queue => self.queue.clone().upcast(),
            Location::Favorites => self.favorites.clone().upcast(),
            Location::Downloaded => self.downloaded.clone().upcast(),
            Location::Shows => return None,
        })
    }

    /// `location` shows: selection follows it, leaving selection mode, and
    /// the header shows its Select and Clear.
    pub(super) fn show(&self, location: Location) {
        self.showing.set(location);
        self.selection.set_scope(location);
        self.select.set_visible(location != Location::Shows);
        self.update_clear();
    }

    fn update_clear(&self) {
        let (label, empty) = match self.showing.get() {
            Location::Queue => (gettext("Clear Queue"), self.queued.borrow().is_empty()),
            Location::Downloaded => (
                gettext("Delete All Downloads"),
                self.downloads.borrow().is_empty(),
            ),
            _ => {
                self.clear.set_visible(false);
                return;
            }
        };
        self.clear.set_visible(true);
        self.clear.set_sensitive(!empty);
        self.clear.set_tooltip_text(Some(&label));
        self.clear
            .update_property(&[gtk::accessible::Property::Label(&label)]);
    }

    pub(super) fn set_queue(&self, data: Box<QueueData>) {
        self.queued
            .replace(data.queued.iter().map(|i| i.episode.id).collect());
        self.update_clear();
        self.queue.send(QueueViewInput::Set(data));
    }

    pub(super) fn set_favorites(&self, items: Vec<EpisodeItem>) {
        self.favorites.send(EpisodesViewInput::Set(items));
    }

    pub(super) fn set_downloaded(&self, items: Vec<EpisodeItem>) {
        self.downloads
            .replace(items.iter().map(|i| i.episode.id).collect());
        self.update_clear();
        self.downloaded.send(EpisodesViewInput::Set(items));
    }

    pub(super) fn set_new(&self, input: NewViewInput) {
        self.new.send(input);
    }
}
