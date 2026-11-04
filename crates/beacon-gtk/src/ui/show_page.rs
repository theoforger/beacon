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
    /// Mark All as Played, or as Unplayed when every episode is played.
    MarkAll,
    CopyFeedAddress,
    Unsubscribe,
}

#[derive(Debug)]
pub enum ShowPageOutput {
    Action(Box<EpisodeItem>, RowAction),
    PrefsChanged(ShowId, ShowPrefs),
    /// Mark every episode of the show played (`true`) or unplayed.
    MarkAll(ShowId, bool),
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
    /// More Options: Mark All, Download All Episodes, Copy Feed Address.
    more: gtk::MenuButton,
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
            // Until the show's own title arrives: the page is pushed first,
            // and a page needs a title for Back and screen readers.
            set_title: &gettext("Podcast"),

            #[wrap(Some)]
            set_child = &adw::ToolbarView {
                #[name = "header"]
                add_top_bar = &adw::HeaderBar {
                    set_show_title: false,
                },

                #[wrap(Some)]
                #[name = "scrolled"]
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
            more: more_button(),
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
        model.fill.follow(&widgets.scrolled);

        // The details toggle at the far end, as on every page; then the
        // show's More Options menu, which the HIG puts in the header bar
        // ("Secondary menus are located in the header bar"), as GNOME
        // Podcasts does; then sort and filter.
        widgets.header.pack_end(&details_toggle);
        widgets.header.pack_end(&model.more);
        widgets.header.pack_end(model.filter.widget());

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
                root.set_title(&data.show.info.title);
                widgets.header.set_show_title(true);
                self.hero.update(&data);
                if self.data.as_ref().map(|d| d.all_played) != Some(data.all_played) {
                    self.more.set_menu_model(Some(&more_menu(data.all_played)));
                }
                self.filter.set_prefs(&data.prefs);
                self.show_episodes(&widgets.episodes, &data);
                self.data = Some(*data);
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
        if self.shown.as_ref() == Some(&(show, shape.clone()))
            && update_in_place(&self.lists, &all, &self.fill)
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
            // Translators: how a screen reader names a show's episode list
            // when the show has no seasons.
            let name = title.clone().unwrap_or_else(|| gettext("Episodes"));
            let list = new_list(&self.on_action, &name);
            match title {
                Some(title) => container.append(&self.season(show, key, &title, &list)),
                None => container.append(&list),
            }
            work.extend(items.iter().map(|item| (list.clone(), item.clone())));
            self.lists.push(list);
        }
        self.shown = Some((show, shape));
        // The page is the show's own: its name and cover would repeat on
        // every row.
        self.fill.start(work, self.on_action.clone(), false);
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
