// SPDX-License-Identifier: GPL-3.0-or-later

//! A podcast page, pushed over the current section, built like GNOME
//! Podcasts' show page: the hero, then the episodes in a `boxed-list`
//! `GtkListBox`. When the feed has season tags, each season is its own list
//! inside a `GtkExpander`, so it can be folded; the page remembers which
//! seasons are folded. Rows are added lazily and updated in place.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use beacon_core::model::{DownloadFilter, EpisodeFilter, EpisodeId, ShowId, ShowPrefs};
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::gtk::{self, gdk, gio, pango};
use relm4::{Component, ComponentParts, ComponentSender, adw};

use crate::ui::episode_list::{LazyFill, ids, new_list, update_in_place};
use crate::ui::format;
use crate::ui::models::{EpisodeItem, RowAction, ShowPageData};
use crate::ui::page_filters;
use crate::ui::show_hero::ShowHero;
use crate::ui::widgets::episode_row::OnRowAction;

pub const SHOW_TAG: &str = "show";

#[derive(Debug)]
pub enum ShowPageInput {
    Set(Box<ShowPageData>),
    /// A choice in the filter menu: the group's action name and the value.
    Choose(page_filters::Choice),
    MarkAllPlayed,
    CopyFeedAddress,
    Unsubscribe,
}

#[derive(Debug)]
pub enum ShowPageOutput {
    Action(Box<EpisodeItem>, RowAction),
    PrefsChanged(ShowId, ShowPrefs),
    MarkAllPlayed(ShowId),
    Unsubscribe(ShowId),
    FeedAddressCopied,
}

/// A season, by number; `None` for the episodes without a season tag.
type SeasonKey = Option<u32>;

/// What a page lists: its show and each season's episodes.
type Shape = (ShowId, Vec<(SeasonKey, Vec<EpisodeId>)>);

pub struct ShowPage {
    data: Option<ShowPageData>,
    hero: ShowHero,
    /// The filter menu's radio groups, by action name.
    filter: page_filters::FilterButton,
    on_action: OnRowAction,
    fill: LazyFill,
    /// What the lists show now, to update in place when only states change.
    shown: Option<Shape>,
    lists: Vec<gtk::ListBox>,
    /// Folded seasons, kept while the app runs.
    folded: Rc<RefCell<HashSet<(ShowId, SeasonKey)>>>,
}

#[relm4::component(pub)]
impl Component for ShowPage {
    /// The page's details toggle, kept in step by the content pane.
    type Init = gtk::ToggleButton;
    type Input = ShowPageInput;
    type Output = ShowPageOutput;
    type CommandOutput = ();

    view! {
        adw::NavigationPage {
            set_tag: Some(SHOW_TAG),

            #[wrap(Some)]
            set_child = &adw::ToolbarView {
                #[name = "header"]
                add_top_bar = &adw::HeaderBar {
                    set_show_title: false,
                },

                #[wrap(Some)]
                set_content = &gtk::ScrolledWindow {
                    set_hscrollbar_policy: gtk::PolicyType::Never,

                    adw::Clamp {
                        set_maximum_size: 900,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_margin_bottom: 24,
                            set_margin_start: 12,
                            set_margin_end: 12,

                            append: model.hero.widget(),

                            #[name = "episodes"]
                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 6,
                                set_margin_top: 12,
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(
        details_toggle: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let s = sender.clone();
        let on_action: OnRowAction = Rc::new(move |item, action| {
            let _ = s.output(ShowPageOutput::Action(Box::new(item.clone()), action));
        });
        let model = ShowPage {
            data: None,
            hero: ShowHero::new(),
            filter: page_filters::FilterButton::new({
                let s = sender.clone();
                move |choice| s.input(ShowPageInput::Choose(choice))
            }),
            on_action,
            fill: LazyFill::default(),
            shown: None,
            lists: Vec::new(),
            folded: Rc::default(),
        };
        let widgets = view_output!();

        // Sort and filter, as in GNOME Podcasts, left of the details toggle.
        widgets.header.pack_end(&details_toggle);
        widgets.header.pack_end(model.filter.widget());

        let actions = gio::SimpleActionGroup::new();
        let mark_all = gio::SimpleAction::new("mark-all-played", None);
        let s = sender.clone();
        mark_all.connect_activate(move |_, _| s.input(ShowPageInput::MarkAllPlayed));
        actions.add_action(&mark_all);
        let copy = gio::SimpleAction::new("copy-feed", None);
        let s = sender.clone();
        copy.connect_activate(move |_, _| s.input(ShowPageInput::CopyFeedAddress));
        actions.add_action(&copy);
        root.insert_action_group("show", Some(&actions));

        model
            .hero
            .unsubscribe
            .connect_clicked(move |_| sender.input(ShowPageInput::Unsubscribe));

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        match message {
            ShowPageInput::Set(data) => {
                root.set_title(&data.show.info.title);
                widgets.header.set_show_title(true);
                self.hero.update(&data);
                self.filter.set_prefs(&data.prefs);
                self.show_episodes(&widgets.episodes, &data);
                self.data = Some(*data);
            }
            ShowPageInput::Choose(choice) => {
                self.change_prefs(&sender, |p| *p = choice.apply(*p));
            }
            ShowPageInput::MarkAllPlayed => {
                if let Some(data) = &self.data {
                    let _ = sender.output(ShowPageOutput::MarkAllPlayed(data.show.id));
                }
            }
            ShowPageInput::CopyFeedAddress => {
                if let (Some(data), Some(display)) = (&self.data, gdk::Display::default()) {
                    display.clipboard().set_text(data.show.feed_url.as_str());
                    let _ = sender.output(ShowPageOutput::FeedAddressCopied);
                }
            }
            ShowPageInput::Unsubscribe => {
                if let Some(data) = &self.data {
                    let _ = sender.output(ShowPageOutput::Unsubscribe(data.show.id));
                }
            }
        }
    }
}

impl ShowPage {
    fn change_prefs(&self, sender: &ComponentSender<Self>, change: impl FnOnce(&mut ShowPrefs)) {
        let Some(data) = &self.data else {
            return;
        };
        let mut prefs = data.prefs;
        change(&mut prefs);
        if prefs != data.prefs {
            let _ = sender.output(ShowPageOutput::PrefsChanged(data.show.id, prefs));
        }
    }

    /// Show the page's episodes: update the rows in place when the same
    /// episodes are listed, otherwise rebuild the lists and fill them lazily.
    fn show_episodes(&mut self, container: &gtk::Box, data: &ShowPageData) {
        let show = data.show.id;
        let groups: Vec<(SeasonKey, Option<String>, &[EpisodeItem])> = match &data.seasons {
            Some(seasons) => seasons
                .iter()
                .map(|s| {
                    let key = s.season.as_ref().map(|s| s.number);
                    let title = format::season_title(s.season.as_ref());
                    (key, Some(title), s.episodes.as_slice())
                })
                .collect(),
            None => vec![(None, None, data.episodes.as_slice())],
        };
        let shape: Vec<_> = groups
            .iter()
            .map(|(key, _, items)| (*key, ids(*items)))
            .collect();
        let all: Vec<&EpisodeItem> = groups.iter().flat_map(|(_, _, items)| *items).collect();
        if self.shown.as_ref() == Some(&(show, shape.clone())) && update_in_place(&self.lists, &all)
        {
            return;
        }

        self.fill.cancel();
        while let Some(child) = container.first_child() {
            container.remove(&child);
        }
        self.lists.clear();

        let mut work = Vec::new();
        if all.is_empty() {
            container.append(&empty_label(&data.prefs));
        }
        for (key, title, items) in groups {
            if items.is_empty() {
                continue;
            }
            let list = new_list(&self.on_action);
            match title {
                Some(title) => container.append(&self.season(show, key, &title, &list)),
                None => container.append(&list),
            }
            work.extend(items.iter().map(|item| (list.clone(), item.clone())));
            self.lists.push(list);
        }
        self.shown = Some((show, shape));
        self.fill.start(work, self.on_action.clone());
    }

    /// A season: its title in a `GtkExpander`, unfolding to its list.
    fn season(
        &self,
        show: ShowId,
        key: SeasonKey,
        title: &str,
        list: &gtk::ListBox,
    ) -> gtk::Expander {
        let label = gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .ellipsize(pango::EllipsizeMode::End)
            .css_classes(["heading"])
            .accessible_role(gtk::AccessibleRole::Heading)
            .build();
        let expander = gtk::Expander::builder()
            .label_widget(&label)
            .child(list)
            .expanded(!self.folded.borrow().contains(&(show, key)))
            .margin_top(12)
            .build();
        list.set_margin_top(6);
        let folded = self.folded.clone();
        expander.connect_expanded_notify(move |expander| {
            let mut folded = folded.borrow_mut();
            if expander.is_expanded() {
                folded.remove(&(show, key));
            } else {
                folded.insert((show, key));
            }
        });
        expander
    }
}

fn empty_label(prefs: &ShowPrefs) -> gtk::Label {
    let only_unplayed =
        prefs.played == EpisodeFilter::Unplayed && prefs.downloaded == DownloadFilter::All;
    let message = if only_unplayed {
        gettext("No unplayed episodes: you are all caught up")
    } else if prefs.is_filtered() {
        gettext("No episodes match these filters")
    } else {
        gettext("This podcast has no episodes yet")
    };
    gtk::Label::builder()
        .label(message)
        .wrap(true)
        .margin_top(24)
        .css_classes(["dim-label"])
        .build()
}
