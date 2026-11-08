// SPDX-License-Identifier: GPL-3.0-or-later

//! A podcast page, pushed over the current section: the hero, then the
//! episodes, all in one list view (`ui/episode_list.rs`) so that long feeds
//! scroll smoothly. When the feed has season tags, each season has its own
//! card under a `GtkExpander` heading, so it can be folded; the page
//! remembers which seasons are folded.

use std::rc::Rc;

use beacon_core::model::{
    DownloadFilter, EpisodeChange, EpisodeFilter, EpisodeId, ShowId, ShowPrefs,
};
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::gtk::{self, gdk, gio};
use relm4::{Component, ComponentParts, ComponentSender, adw};

use crate::ui::episode_list::{Entry, EpisodeList, Folding};
use crate::ui::format;
use crate::ui::models::{EpisodeItem, RowAction, ShowPageData};
use crate::ui::page_filters;
use crate::ui::selection::Selection;
use crate::ui::selection_bar;
use crate::ui::show_hero::ShowHero;
use crate::ui::widgets::episode_row::RowSetup;

pub const SHOW_TAG: &str = "show";

#[derive(Debug)]
pub enum ShowPageInput {
    Set(Box<ShowPageData>),
    /// A choice in the filter menu: the group's action name and the value.
    Choose(page_filters::Choice),
    /// Mark All as Played, or as Unplayed when every episode is played.
    MarkAll,
    CopyFeedAddress,
    Unsubscribe,
    /// A season was folded or unfolded.
    Refold,
}

#[derive(Debug)]
pub enum ShowPageOutput {
    Action(Box<EpisodeItem>, RowAction),
    PrefsChanged(ShowId, ShowPrefs),
    /// Mark every episode of the show played (`true`) or unplayed.
    MarkAll(ShowId, bool),
    Unsubscribe(ShowId),
    FeedAddressCopied,
    /// A change to the selected episodes.
    Change(Vec<EpisodeId>, EpisodeChange),
}

pub struct ShowPage {
    data: Option<ShowPageData>,
    hero: ShowHero,
    /// The filter menu's radio groups, by action name.
    filter: page_filters::FilterButton,
    /// More Options: Mark All, Download All Episodes, Copy Feed Address.
    more: gtk::MenuButton,
    rows: RowSetup,
    list: EpisodeList,
    /// Folded seasons, kept while the app runs.
    folding: Folding,
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
            // Until the show's own title arrives: the page is pushed first,
            // and a page needs a title for Back and screen readers.
            set_title: &gettext("Podcast"),

            #[wrap(Some)]
            #[name = "toolbar"]
            set_child = &adw::ToolbarView {
                #[name = "header"]
                add_top_bar = &adw::HeaderBar {
                    set_show_title: false,
                },

                set_content: Some(model.list.widget()),
            },
        }
    }

    fn init(
        details_toggle: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let s = sender.clone();
        // The page is the show's own: its name and cover would repeat on
        // every row.
        let rows = RowSetup {
            on_action: Rc::new(move |item, action| {
                let _ = s.output(ShowPageOutput::Action(Box::new(item.clone()), action));
            }),
            show_identity: false,
            selection: Selection::default(),
        };
        let hero = ShowHero::new();
        let folding = Folding {
            folded: Rc::default(),
            changed: Rc::new({
                let s = sender.clone();
                move || s.input(ShowPageInput::Refold)
            }),
        };
        // Translators: how a screen reader names a show's episode list.
        let list = EpisodeList::new(
            &rows,
            &gettext("Episodes"),
            Some(hero.widget().clone().upcast()),
            Some(folding.clone()),
        );
        let model = ShowPage {
            data: None,
            hero,
            more: more_button(),
            filter: page_filters::FilterButton::new({
                let s = sender.clone();
                move |choice| s.input(ShowPageInput::Choose(choice))
            }),
            rows,
            list,
            folding,
        };
        let widgets = view_output!();

        // The details toggle at the far end, as on every page; then the
        // show's More Options menu, which the HIG puts in the header bar
        // ("Secondary menus are located in the header bar"), as GNOME
        // Podcasts does; then sort and filter.
        widgets.header.pack_end(&details_toggle);
        widgets.header.pack_end(&model.more);
        widgets.header.pack_end(model.filter.widget());
        let select = selection_bar::select_button(&model.rows.selection);
        widgets.header.pack_end(&select);
        let s = sender.clone();
        selection_bar::attach(
            &widgets.toolbar,
            &widgets.header,
            &select,
            &model.rows.selection,
            Box::new(move |ids, change| {
                let _ = s.output(ShowPageOutput::Change(ids, change));
            }),
        );
        // Leaving the page leaves selection mode.
        let selection = model.rows.selection.clone();
        root.connect_hidden(move |_| selection.set_active(false));

        let actions = gio::SimpleActionGroup::new();
        let mark_all = gio::SimpleAction::new("mark-all", None);
        let s = sender.clone();
        mark_all.connect_activate(move |_, _| s.input(ShowPageInput::MarkAll));
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
                if self.data.as_ref().map(|d| d.show.id) != Some(data.show.id) {
                    self.rows.selection.set_active(false);
                    self.list.set(Vec::new());
                }
                root.set_title(&data.show.info.title);
                widgets.header.set_show_title(true);
                self.hero.update(&data);
                if self.data.as_ref().map(|d| d.all_played) != Some(data.all_played) {
                    self.more.set_menu_model(Some(&more_menu(data.all_played)));
                }
                self.filter.set_prefs(&data.prefs);
                self.data = Some(*data);
                self.show_episodes();
            }
            ShowPageInput::Choose(choice) => {
                self.change_prefs(&sender, |p| *p = choice.apply(*p));
            }
            ShowPageInput::MarkAll => {
                if let Some(data) = &self.data {
                    let played = !data.all_played;
                    let _ = sender.output(ShowPageOutput::MarkAll(data.show.id, played));
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
            ShowPageInput::Refold => self.show_episodes(),
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

    /// List the hero, then each season's heading and, unless folded, its
    /// episodes; or the episodes alone when the feed has no seasons.
    fn show_episodes(&self) {
        let Some(data) = &self.data else {
            return;
        };
        let show = data.show.id;
        let mut all = Vec::new();
        let mut entries = vec![Entry::Header];
        match &data.seasons {
            Some(seasons) => {
                // Seasons 24px apart, as GNOME Podcasts spaces its groups.
                for (i, season) in seasons
                    .iter()
                    .filter(|s| !s.episodes.is_empty())
                    .enumerate()
                {
                    let key = (show, season.season.as_ref().map(|s| s.number));
                    entries.push(Entry::Heading {
                        title: format::season_title(season.season.as_ref()),
                        fold: Some(key),
                        top: if i == 0 { 12 } else { 24 },
                    });
                    if !self.folding.folded.borrow().contains(&key) {
                        entries.extend(Entry::card(&season.episodes));
                    }
                    all.extend(season.episodes.iter().cloned());
                }
            }
            None if !data.episodes.is_empty() => {
                entries.push(Entry::Space(12));
                entries.extend(Entry::card(&data.episodes));
                all.extend(data.episodes.iter().cloned());
            }
            None => {}
        }
        if all.is_empty() {
            entries.push(Entry::Message(empty_message(&data.prefs)));
        }
        entries.push(Entry::Space(24));
        self.rows.selection.set_items(all);
        self.list.set(entries);
    }
}

fn empty_message(prefs: &ShowPrefs) -> String {
    let only_unplayed =
        prefs.played == EpisodeFilter::Unplayed && prefs.downloaded == DownloadFilter::All;
    if only_unplayed {
        gettext("No unplayed episodes: you are all caught up")
    } else if prefs.is_filtered() {
        gettext("No episodes match these filters")
    } else {
        gettext("This podcast has no episodes yet")
    }
}

/// The page's More Options button, in its header bar.
fn more_button() -> gtk::MenuButton {
    let more = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(gettext("More Options"))
        .menu_model(&more_menu(false))
        .build();
    more.update_property(&[gtk::accessible::Property::Label(&gettext("More Options"))]);
    more
}

/// More Options, offering to mark every episode played, or unplayed when
/// every one already is.
fn more_menu(all_played: bool) -> gio::Menu {
    let menu = gio::Menu::new();
    let mark_all = if all_played {
        gettext("Mark All as _Unplayed")
    } else {
        gettext("Mark All as _Played")
    };
    menu.append(Some(&mark_all), Some("show.mark-all"));
    // Downloads arrive in M5; the item stays disabled until then.
    menu.append(
        Some(&gettext("_Download All Episodes")),
        Some("show.download-all"),
    );
    menu.append(Some(&gettext("_Copy Feed Address")), Some("show.copy-feed"));
    menu
}
