// SPDX-License-Identifier: GPL-3.0-or-later

//! The expandable right pane, laid out as in the mockup: cover, a link to the
//! show, title, the long subtitle, then Play, Pause or Resume in the accent
//! colour, About This Episode and the chapters. The episode's other actions
//! (queue, download, favourite, played, Reset Progress) are in More Options
//! in the pane's header, as in a row's menu (changed 2026-11-08 from four
//! icon toggles beside Play). The current
//! chapter is grey and bold; choosing one plays from it.
//!
//! Widgets are set when what they show changes, not on every message: the
//! pane hears where playback is four times a second.

use std::path::PathBuf;
use std::time::Duration;

use beacon_core::model::{Chapter, EpisodeId, ShowId};
use beacon_core::{playback, text};
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{Component, ComponentParts, ComponentSender, adw, gtk};

use crate::player::PlaybackState;
use crate::ui::chapter_list;
use crate::ui::covers::Cover;
use crate::ui::episode_actions;
use crate::ui::format;
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

pub struct DetailsPane {
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

impl Component for DetailsPane {
    type Init = ();
    type Input = DetailsPaneInput;
    type Output = DetailsPaneOutput;
    type CommandOutput = ();
    type Root = adw::ToolbarView;
    type Widgets = DetailsPaneWidgets;

    fn init_root() -> Self::Root {
        adw::ToolbarView::new()
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = build::widgets(&root, &sender);
        let model = DetailsPane {
            episode: None,
            own_cover: None,
            chapters: Vec::new(),
            current: None,
            playing: None,
        };
        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            DetailsPaneInput::Pressed(action) => {
                if let Some(item) = &self.episode {
                    let _ =
                        sender.output(DetailsPaneOutput::Action(Box::new(item.clone()), action));
                }
            }
            DetailsPaneInput::ShowClicked => {
                if let Some(item) = &self.episode {
                    let _ = sender.output(DetailsPaneOutput::OpenShow(item.episode.show_id));
                }
            }
            DetailsPaneInput::ChapterChosen(index) => {
                if let (Some(item), Some(chapter)) = (&self.episode, self.chapters.get(index)) {
                    let _ = sender.output(DetailsPaneOutput::PlayFrom(
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
                widgets.more.set_visible(false);
                widgets.stack.set_visible_child_name("empty");
            }
        }
    }
}

impl DetailsPane {
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
        widgets.more.set_visible(true);
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
