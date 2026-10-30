// SPDX-License-Identifier: GPL-3.0-or-later

//! A podcast page, pushed over the current section. One `GtkListView` over a
//! `GtkTreeListModel`: the hero is the first row, seasons are collapsible
//! rows with their episodes beneath, and a show without season tags lists
//! its episodes directly. Long feeds stay fast because only visible rows get
//! widgets.

use std::cell::RefCell;
use std::rc::Rc;

use beacon_core::model::{DownloadFilter, EpisodeFilter, ShowId, ShowPrefs};
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::gtk::{self, gdk, gio, glib, pango};
use relm4::{Component, ComponentParts, ComponentSender, adw};

use crate::ui::episode_list::{boxed, row_of};
use crate::ui::format;
use crate::ui::models::{EpisodeItem, RowAction, SeasonItems, ShowPageData};
use crate::ui::page_filters;
use crate::ui::show_hero::ShowHero;
use crate::ui::widgets::episode_row::OnRowAction;

pub const SHOW_TAG: &str = "show";

#[derive(Debug)]
pub enum ShowPageInput {
    Set(Box<ShowPageData>),
    /// A choice in the filter menu: the group's action name and the value.
    Choose(&'static str, String),
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

enum Entry {
    Hero,
    Season(SeasonItems),
    Episode(Box<EpisodeItem>),
    /// Shown under the hero when the filters leave nothing.
    Empty(String),
}

pub struct ShowPage {
    data: Option<ShowPageData>,
    hero: Rc<ShowHero>,
    entries: gio::ListStore,
    /// The filter menu's radio groups, by action name.
    choices: Vec<gio::SimpleAction>,
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
            #[name = "toolbar"]
            set_child = &adw::ToolbarView {
                #[name = "header"]
                add_top_bar = &adw::HeaderBar {
                    set_show_title: false,
                },

                #[wrap(Some)]
                set_content = &gtk::ScrolledWindow {
                    set_hscrollbar_policy: gtk::PolicyType::Never,

                    #[wrap(Some)]
                    set_child = &adw::ClampScrollable {
                        set_maximum_size: 900,

                        #[wrap(Some)]
                        #[name = "list"]
                        set_child = &gtk::ListView {
                            set_single_click_activate: true,
                            add_css_class: "flat-view",
                            update_property: &[gtk::accessible::Property::Label(&gettext("Episodes"))],
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
        let hero = Rc::new(ShowHero::new());
        let model = ShowPage {
            data: None,
            hero: hero.clone(),
            entries: gio::ListStore::new::<glib::BoxedAnyObject>(),
            choices: [
                page_filters::SORT,
                page_filters::PLAYED,
                page_filters::DOWNLOADED,
            ]
            .into_iter()
            .map(|name| {
                gio::SimpleAction::new_stateful(
                    name,
                    Some(glib::VariantTy::STRING),
                    &"".to_variant(),
                )
            })
            .collect(),
        };
        let widgets = view_output!();

        // Sort and filter, as in GNOME Podcasts, left of the details toggle.
        let filter = gtk::MenuButton::builder()
            .icon_name("funnel-symbolic")
            .menu_model(&page_filters::menu())
            .tooltip_text(gettext("Sort and Filter"))
            .build();
        filter.update_property(&[gtk::accessible::Property::Label(&gettext(
            "Sort and Filter",
        ))]);
        widgets.header.pack_end(&details_toggle);
        widgets.header.pack_end(&filter);

        let actions = gio::SimpleActionGroup::new();
        for (choice, name) in model.choices.iter().zip([
            page_filters::SORT,
            page_filters::PLAYED,
            page_filters::DOWNLOADED,
        ]) {
            let s = sender.clone();
            choice.connect_activate(move |action, target| {
                if let Some(value) = target.and_then(|t| t.get::<String>()) {
                    action.set_state(&value.to_variant());
                    s.input(ShowPageInput::Choose(name, value));
                }
            });
            actions.add_action(choice);
        }
        let mark_all = gio::SimpleAction::new("mark-all-played", None);
        let s = sender.clone();
        mark_all.connect_activate(move |_, _| s.input(ShowPageInput::MarkAllPlayed));
        actions.add_action(&mark_all);
        let copy = gio::SimpleAction::new("copy-feed", None);
        let s = sender.clone();
        copy.connect_activate(move |_, _| s.input(ShowPageInput::CopyFeedAddress));
        actions.add_action(&copy);
        root.insert_action_group("show", Some(&actions));

        let s = sender.clone();
        hero.unsubscribe
            .connect_clicked(move |_| s.input(ShowPageInput::Unsubscribe));

        let tree = gtk::TreeListModel::new(model.entries.clone(), false, true, |object| {
            let boxed = object.downcast_ref::<glib::BoxedAnyObject>()?;
            let Entry::Season(season) = &*boxed.borrow::<Entry>() else {
                return None;
            };
            let children = gio::ListStore::new::<glib::BoxedAnyObject>();
            for episode in &season.episodes {
                children.append(&glib::BoxedAnyObject::new(Entry::Episode(Box::new(
                    episode.clone(),
                ))));
            }
            Some(children.upcast())
        });
        widgets
            .list
            .set_model(Some(&gtk::NoSelection::new(Some(tree.clone()))));

        let on_action: OnRowAction = Rc::new(move |item, action| {
            let _ = sender.output(ShowPageOutput::Action(Box::new(item.clone()), action));
        });
        widgets.list.set_factory(Some(&factory(
            hero.clone(),
            on_action.clone(),
            tree.upcast(),
        )));
        widgets.list.connect_activate(move |list, position| {
            let item = list.model().and_then(|m| m.item(position));
            let episode = item
                .as_ref()
                .and_then(boxed)
                .and_then(|b| match &*b.borrow::<Entry>() {
                    Entry::Episode(episode) => Some((**episode).clone()),
                    _ => None,
                });
            if let Some(episode) = episode {
                on_action(&episode, RowAction::ShowDetails);
            }
        });

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
                for (choice, (_, value)) in
                    self.choices.iter().zip(page_filters::states(&data.prefs))
                {
                    choice.set_state(&value.to_variant());
                }
                self.entries
                    .splice(0, self.entries.n_items(), &entries(&data));
                self.data = Some(*data);
            }
            ShowPageInput::Choose(action, value) => {
                self.change_prefs(&sender, |p| *p = page_filters::apply(*p, action, &value));
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
}

fn entries(data: &ShowPageData) -> Vec<glib::BoxedAnyObject> {
    let mut entries = vec![Entry::Hero];
    match &data.seasons {
        Some(seasons) => entries.extend(seasons.iter().cloned().map(Entry::Season)),
        None => entries.extend(
            data.episodes
                .iter()
                .cloned()
                .map(|e| Entry::Episode(Box::new(e))),
        ),
    }
    let nothing = match &data.seasons {
        Some(seasons) => seasons.is_empty(),
        None => data.episodes.is_empty(),
    };
    if nothing {
        let only_unplayed = data.prefs.played == EpisodeFilter::Unplayed
            && data.prefs.downloaded == DownloadFilter::All;
        entries.push(Entry::Empty(if only_unplayed {
            gettext("No unplayed episodes: you are all caught up")
        } else if data.prefs.is_filtered() {
            gettext("No episodes match these filters")
        } else {
            gettext("This podcast has no episodes yet")
        }));
    }
    entries.into_iter().map(glib::BoxedAnyObject::new).collect()
}

fn factory(
    hero: Rc<ShowHero>,
    on_action: OnRowAction,
    model: gio::ListModel,
) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    // The one list item holding the hero, so a rebinding elsewhere can take
    // it back first: a widget has only one parent.
    let holder: Rc<RefCell<glib::WeakRef<gtk::ListItem>>> = Rc::default();

    let hero_widget = hero.widget().clone();
    let releasing = holder.clone();
    factory.connect_unbind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        if item.child().as_ref() == Some(hero_widget.upcast_ref()) {
            item.set_child(None::<&gtk::Widget>);
            releasing.replace(glib::WeakRef::new());
        }
    });

    factory.connect_bind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let Some(object) = item.item() else {
            return;
        };
        let Some(entry) = boxed(&object) else {
            return;
        };
        item.set_activatable(false);
        match &*entry.borrow::<Entry>() {
            Entry::Hero => {
                let widget = hero.widget();
                if let Some(old) = holder.borrow().upgrade()
                    && old != *item
                    && old.child().as_ref() == Some(widget.upcast_ref())
                {
                    old.set_child(None::<&gtk::Widget>);
                }
                item.set_child(Some(widget));
                holder.replace(item.downgrade());
            }
            Entry::Season(season) => {
                let label = gtk::Label::builder()
                    .label(format::season_title(season.season.as_ref()))
                    .xalign(0.0)
                    .ellipsize(pango::EllipsizeMode::End)
                    .css_classes(["heading"])
                    .build();
                let expander = gtk::TreeExpander::builder()
                    .child(&label)
                    .margin_top(12)
                    .build();
                expander.set_list_row(object.downcast_ref::<gtk::TreeListRow>());
                item.set_child(Some(&expander));
            }
            Entry::Episode(episode) => {
                item.set_activatable(true);
                let row = row_of(item, &on_action);
                row.bind(episode);
                // A season's episodes, or a flat list, form one boxed list.
                let is_episode = |offset: i64| {
                    u32::try_from(i64::from(item.position()) + offset)
                        .ok()
                        .and_then(|p| model.item(p))
                        .as_ref()
                        .and_then(boxed)
                        .is_some_and(|b| matches!(&*b.borrow::<Entry>(), Entry::Episode(_)))
                };
                row.set_position(!is_episode(-1), !is_episode(1));
            }
            Entry::Empty(message) => {
                let label = gtk::Label::builder()
                    .label(message)
                    .wrap(true)
                    .margin_top(24)
                    .css_classes(["dim-label"])
                    .build();
                item.set_child(Some(&label));
            }
        }
    });
    factory
}
