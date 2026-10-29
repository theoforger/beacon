// SPDX-License-Identifier: GPL-3.0-or-later

//! The New section: recent episodes of every show, grouped by recency under
//! headers, with an All / Unplayed toggle group above that the window
//! remembers.

use std::cmp::Ordering;
use std::rc::Rc;

use beacon_core::browse::RecencyGroup;
use beacon_core::model::{EpisodeFilter, EpisodeId};
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::gtk::{self, gio, glib};
use relm4::{Component, ComponentParts, ComponentSender, adw};

use crate::ui::episode_list::{boxed, row_of};
use crate::ui::format;
use crate::ui::models::{EpisodeItem, RowAction};
use crate::ui::widgets::episode_row::OnRowAction;

#[derive(Debug)]
pub enum NewViewInput {
    Set {
        items: Vec<(RecencyGroup, EpisodeItem)>,
        filter: EpisodeFilter,
        has_shows: bool,
    },
}

#[derive(Debug)]
pub enum NewViewOutput {
    Action(EpisodeId, RowAction),
    FilterChanged(EpisodeFilter),
}

type Entry = (RecencyGroup, EpisodeItem);

pub struct NewView {
    entries: gio::ListStore,
}

#[relm4::component(pub)]
impl Component for NewView {
    type Init = ();
    type Input = NewViewInput;
    type Output = NewViewOutput;
    type CommandOutput = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            #[name = "filter"]
            adw::ToggleGroup {
                set_halign: gtk::Align::Center,
                set_margin_top: 12,
                set_margin_bottom: 6,
                set_homogeneous: true,
            },

            #[name = "stack"]
            gtk::Stack {
                set_vexpand: true,

                add_named[Some("list")] = &gtk::ScrolledWindow {
                    set_hscrollbar_policy: gtk::PolicyType::Never,

                    #[wrap(Some)]
                    set_child = &adw::ClampScrollable {
                        set_maximum_size: 900,

                        #[wrap(Some)]
                        #[name = "list"]
                        set_child = &gtk::ListView {
                            set_single_click_activate: true,
                            add_css_class: "flat-view",
                            update_property: &[gtk::accessible::Property::Label(&gettext("New Episodes"))],
                        },
                    },
                },

                #[name = "empty"]
                add_named[Some("empty")] = &adw::StatusPage {
                    set_icon_name: Some("view-list-bullet-symbolic"),
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = NewView {
            entries: gio::ListStore::new::<glib::BoxedAnyObject>(),
        };
        let widgets = view_output!();

        widgets
            .filter
            .upcast_ref::<gtk::Widget>()
            .update_property(&[gtk::accessible::Property::Label(&gettext("Show Episodes"))]);
        widgets.filter.add(
            adw::Toggle::builder()
                .name("all")
                .label(gettext("All"))
                .build(),
        );
        widgets.filter.add(
            adw::Toggle::builder()
                .name("unplayed")
                .label(gettext("Unplayed"))
                .build(),
        );
        let s = sender.clone();
        widgets.filter.connect_active_name_notify(move |group| {
            let filter = filter_from(group.active_name().as_deref());
            let _ = s.output(NewViewOutput::FilterChanged(filter));
        });

        // Sections are runs of the same recency group; the items arrive in
        // order, so only the section sorter is needed.
        let sections = gtk::SortListModel::new(Some(model.entries.clone()), None::<gtk::Sorter>);
        sections.set_section_sorter(Some(&gtk::CustomSorter::new(|a, b| {
            match (group_of(a), group_of(b)) {
                (Some(a), Some(b)) => a.cmp(&b).into(),
                _ => Ordering::Equal.into(),
            }
        })));
        widgets
            .list
            .set_model(Some(&gtk::NoSelection::new(Some(sections))));

        let on_action: OnRowAction = Rc::new(move |id, action| {
            let _ = sender.output(NewViewOutput::Action(id, action));
        });
        widgets
            .list
            .set_factory(Some(&row_factory(on_action.clone())));
        widgets.list.set_header_factory(Some(&header_factory()));
        widgets.list.connect_activate(move |list, position| {
            if let Some(id) = list
                .model()
                .and_then(|m| m.item(position))
                .as_ref()
                .and_then(boxed)
                .map(|b| b.borrow::<Entry>().1.episode.id)
            {
                on_action(id, RowAction::ShowDetails);
            }
        });

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        let NewViewInput::Set {
            items,
            filter,
            has_shows,
        } = message;
        let name = match filter {
            EpisodeFilter::All => "all",
            EpisodeFilter::Unplayed => "unplayed",
        };
        if widgets.filter.active_name().as_deref() != Some(name) {
            widgets.filter.set_active_name(Some(name));
        }

        let (title, description) = if !has_shows {
            (
                gettext("No Episodes"),
                gettext("Subscribe to a podcast to see its episodes here"),
            )
        } else if filter == EpisodeFilter::Unplayed {
            (
                gettext("No Unplayed Episodes"),
                gettext("You are all caught up"),
            )
        } else {
            (
                gettext("No Recent Episodes"),
                gettext("Episodes from the last month appear here"),
            )
        };
        widgets.empty.set_title(&title);
        widgets.empty.set_description(Some(&description));
        widgets
            .stack
            .set_visible_child_name(if items.is_empty() { "empty" } else { "list" });

        let objects: Vec<glib::BoxedAnyObject> =
            items.into_iter().map(glib::BoxedAnyObject::new).collect();
        self.entries.splice(0, self.entries.n_items(), &objects);
    }
}

pub fn filter_from(name: Option<&str>) -> EpisodeFilter {
    match name {
        Some("unplayed") => EpisodeFilter::Unplayed,
        _ => EpisodeFilter::All,
    }
}

fn group_of(object: &glib::Object) -> Option<RecencyGroup> {
    boxed(object).map(|b| b.borrow::<Entry>().0)
}

fn row_factory(on_action: OnRowAction) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_bind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let Some(entry) = item.item().as_ref().and_then(boxed) else {
            return;
        };
        row_of(item, &on_action).bind(&entry.borrow::<Entry>().1);
    });
    factory
}

fn header_factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, header| {
        if let Some(header) = header.downcast_ref::<gtk::ListHeader>() {
            let label = gtk::Label::builder()
                .xalign(0.0)
                .margin_top(12)
                .margin_start(6)
                .css_classes(["heading"])
                .accessible_role(gtk::AccessibleRole::Heading)
                .build();
            header.set_child(Some(&label));
        }
    });
    factory.connect_bind(|_, header| {
        let Some(header) = header.downcast_ref::<gtk::ListHeader>() else {
            return;
        };
        let (Some(label), Some(group)) = (
            header.child().and_downcast::<gtk::Label>(),
            header.item().as_ref().and_then(group_of),
        ) else {
            return;
        };
        label.set_label(&format::recency_title(group));
    });
    factory
}
