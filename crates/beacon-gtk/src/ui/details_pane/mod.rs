// SPDX-License-Identifier: GPL-3.0-or-later

//! The expandable right pane, laid out as in the mockup: cover, a link to the
//! show, title, the long subtitle, then Play, Pause or Resume in the accent
//! colour with More Options beside it, About This Episode and the chapters.
//! More Options holds the episode's other actions (queue, download,
//! favourite, played, Reset Progress), as a row's menu does (changed
//! 2026-11-08 from four icon toggles beside Play). The current
//! chapter is grey with its time in the accent colour; choosing one plays
//! from it.
//!
//! Widgets are set when what they show changes, not on every message: the
//! pane hears where playback is four times a second.

use std::path::PathBuf;
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use beacon_core::model::{Chapter, EpisodeId, ShowId};
use beacon_core::{playback, text};
use gettextrs::gettext;
use gtk::glib;

use crate::player::PlaybackState;
use crate::ui::chapter_list;
use crate::ui::covers::Cover;
use crate::ui::episode_actions;
use crate::ui::format;
use crate::ui::messages::{Inbox, Output, ViewSender};
use crate::ui::models::{EpisodeItem, RowAction};
use crate::ui::play_button;

mod build;

/// As in the mockup at 1440px wide.
const COVER_SIZE: i32 = 176;
/// The action group More Options' items are in.
const ACTIONS: &str = "episode";

#[derive(Debug)]
pub enum DetailsPaneInput {
    Show(Box<EpisodeItem>),
    /// The episode's own cover, once fetched; the show's stands in until
    /// then, and for episodes without one.
    EpisodeCover(EpisodeId, PathBuf),
    /// The episode's chapters, once loaded.
    Chapters(EpisodeId, Vec<Chapter>),
    /// The episode in the player, whether it plays, and where it is.
    Playback(Option<(EpisodeId, PlaybackState, Duration)>),
    Clear,
    /// From the pane's own widgets: Play or More Options, the show link, a
    /// chapter.
    Pressed(RowAction),
    ShowClicked,
    ChapterChosen(usize),
}

#[derive(Debug)]
pub enum DetailsPaneOutput {
    /// The pane's own Back, shown in the narrow layout.
    Close,
    /// Play or pause, or one of More Options' actions, as a row's menu does.
    Action(Box<EpisodeItem>, RowAction),
    /// The show link.
    OpenShow(ShowId),
    /// Play the episode from a chapter.
    PlayFrom(Box<EpisodeItem>, Duration),
}

/// What the pane's widgets report through.
type Sender = ViewSender<DetailsPaneInput, DetailsPaneOutput>;

#[derive(Default)]
pub struct State {
    episode: Option<EpisodeItem>,
    /// The shown episode's own cover, when it has one and it has arrived.
    own_cover: Option<(EpisodeId, PathBuf)>,
    chapters: Vec<Chapter>,
    /// The chapter shown as playing.
    current: Option<usize>,
    /// The episode in the player and whether it plays.
    playing: Option<(EpisodeId, PlaybackState)>,
}

pub struct DetailsPaneWidgets {
    stack: gtk::Stack,
    back: gtk::Button,
    cover: Cover,
    show: gtk::Button,
    show_label: gtk::Label,
    title: gtk::Label,
    subtitle: gtk::Label,
    play: gtk::Button,
    more: gtk::MenuButton,
    about: gtk::Label,
    description: gtk::Label,
    chapters_heading: gtk::Label,
    chapters: gtk::ListBox,
}

impl DetailsPaneWidgets {
    /// Shown only in the narrow layout, where the pane covers the window.
    pub fn back_button(&self) -> &gtk::Button {
        &self.back
    }

    /// More Options, for tests.
    #[cfg(test)]
    pub fn more(&self) -> &gtk::MenuButton {
        &self.more
    }

    /// Where focus goes when a row opens the pane: its first action.
    pub fn play_button(&self) -> &gtk::Button {
        &self.play
    }
}

mod imp {
    use std::cell::{OnceCell, RefCell};

    use super::*;

    #[derive(Default)]
    pub struct DetailsPane {
        pub widgets: OnceCell<DetailsPaneWidgets>,
        pub state: RefCell<State>,
        pub inbox: Inbox<DetailsPaneInput>,
        pub output: Output<DetailsPaneOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DetailsPane {
        const NAME: &'static str = "BeaconDetailsPane";
        type Type = super::DetailsPane;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for DetailsPane {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            let root = adw::ToolbarView::new();
            let pane = obj.downgrade();
            let sender = Sender::new(
                move |message| {
                    if let Some(pane) = pane.upgrade() {
                        pane.send(message);
                    }
                },
                self.output.clone(),
            );
            let _ = self.widgets.set(build::widgets(&root, &sender));
            obj.set_child(Some(&root));
        }
    }
    impl WidgetImpl for DetailsPane {}
    impl BinImpl for DetailsPane {}
}

glib::wrapper! {
    pub struct DetailsPane(ObjectSubclass<imp::DetailsPane>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for DetailsPane {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl DetailsPane {
    pub fn connect_output(&self, receive: impl Fn(DetailsPaneOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    pub fn send(&self, input: DetailsPaneInput) {
        let imp = self.imp();
        imp.inbox.deliver(input, |message| {
            let widgets = self.widgets();
            imp.state.borrow_mut().update(widgets, message, &imp.output);
        });
    }

    pub fn widgets(&self) -> &DetailsPaneWidgets {
        self.imp().widgets.get().expect("built when constructed")
    }
}

impl State {
    fn update(
        &mut self,
        widgets: &DetailsPaneWidgets,
        message: DetailsPaneInput,
        output: &Output<DetailsPaneOutput>,
    ) {
        match message {
            DetailsPaneInput::Pressed(action) => {
                if let Some(item) = &self.episode {
                    output.emit(DetailsPaneOutput::Action(Box::new(item.clone()), action));
                }
            }
            DetailsPaneInput::ShowClicked => {
                if let Some(item) = &self.episode {
                    output.emit(DetailsPaneOutput::OpenShow(item.episode.show_id));
                }
            }
            DetailsPaneInput::ChapterChosen(index) => {
                if let (Some(item), Some(chapter)) = (&self.episode, self.chapters.get(index)) {
                    output.emit(DetailsPaneOutput::PlayFrom(
                        Box::new(item.clone()),
                        chapter.start,
                    ));
                }
            }
            DetailsPaneInput::Show(item) => self.show(widgets, *item),
            DetailsPaneInput::EpisodeCover(id, path) => {
                if self.shown() == Some(id) {
                    widgets.cover.set(Some(&path));
                    self.own_cover = Some((id, path));
                }
            }
            DetailsPaneInput::Chapters(id, chapters) => {
                if self.shown() == Some(id) {
                    self.chapters = chapters;
                    self.current = None;
                    chapter_list::fill(&widgets.chapters, &self.chapters);
                    let any = !self.chapters.is_empty();
                    widgets.chapters_heading.set_visible(any);
                    widgets.chapters.set_visible(any);
                    self.mark_current(widgets, None);
                }
            }
            DetailsPaneInput::Playback(state) => {
                let playing = state.map(|(id, playing, _)| (id, playing));
                if playing != self.playing {
                    self.playing = playing;
                    self.label_play(widgets);
                }
                let position = state
                    .filter(|(id, _, _)| Some(*id) == self.shown())
                    .map(|(_, _, position)| position);
                self.mark_current(widgets, position);
            }
            DetailsPaneInput::Clear => {
                self.episode = None;
                self.own_cover = None;
                self.chapters.clear();
                self.current = None;
                widgets.stack.set_visible_child_name("empty");
            }
        }
    }
}

impl State {
    fn shown(&self) -> Option<EpisodeId> {
        self.episode.as_ref().map(|item| item.episode.id)
    }

    fn show(&mut self, widgets: &DetailsPaneWidgets, item: EpisodeItem) {
        let id = item.episode.id;
        let same = self.shown() == Some(id);
        if !same {
            self.own_cover = None;
            self.chapters.clear();
            self.current = None;
            chapter_list::fill(&widgets.chapters, &[]);
            widgets.chapters_heading.set_visible(false);
            widgets.chapters.set_visible(false);
        }
        // A reload of the same episode keeps the cover it has.
        let cover = self.own_cover.as_ref().map(|(_, path)| path.as_path());
        widgets.cover.set(cover.or(item.cover.as_deref()));
        let info = &item.episode.info;
        if self
            .episode
            .as_ref()
            .map(|e| (&e.show_title, &e.episode.info))
            != Some((&item.show_title, info))
        {
            widgets.show_label.set_label(&item.show_title);
            widgets
                .show
                .update_property(&[gtk::accessible::Property::Label(
                    &gettext("Open {show}").replace("{show}", &item.show_title),
                )]);
            widgets.title.set_label(&info.title);
            let subtitle = format::long_subtitle(&item.episode);
            widgets.subtitle.set_label(&subtitle);
            widgets
                .subtitle
                .update_property(&[gtk::accessible::Property::Label(&format::spoken(&subtitle))]);
            let description = info.description.as_deref().map(text::markup);
            widgets.about.set_visible(description.is_some());
            widgets.description.set_visible(description.is_some());
            widgets
                .description
                .set_markup(description.as_deref().unwrap_or_default());
        }
        widgets
            .more
            .set_menu_model(Some(&episode_actions::menu(&item, ACTIONS)));
        self.episode = Some(item);
        self.label_play(widgets);
        widgets.stack.set_visible_child_name("episode");
    }

    /// Play, Pause or Resume, as fits the episode shown.
    fn label_play(&self, widgets: &DetailsPaneWidgets) {
        let Some(item) = &self.episode else {
            return;
        };
        let state = self
            .playing
            .filter(|(id, _)| *id == item.episode.id)
            .map(|(_, state)| state);
        let page = match state {
            Some(_) if play_button::is_playing(state) => "pause",
            Some(_) => "resume",
            None if item.episode.state.is_partly_played() => "resume",
            None => "play",
        };
        play_button::set_label(&widgets.play, play_button::is_loading(state), page);
    }

    /// Show which chapter plays at `position`, if this episode is playing.
    fn mark_current(&mut self, widgets: &DetailsPaneWidgets, position: Option<Duration>) {
        let current = position.and_then(|p| playback::current_chapter(&self.chapters, p));
        if current == self.current {
            return;
        }
        for (index, mark) in [(self.current, false), (current, true)] {
            if let Some(row) = index.and_then(|i| widgets.chapters.row_at_index(i as i32)) {
                chapter_list::mark(&row, mark);
            }
        }
        self.current = current;
    }
}
