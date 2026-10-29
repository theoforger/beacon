// SPDX-License-Identifier: GPL-3.0-or-later

//! The Shows section: a grid of cover tiles with a badge for new episodes,
//! ending in an Add Podcast tile. With no subscriptions it is an empty state
//! with an Add Podcast button.

use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::gtk::{self, gio, glib, pango};
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw};

use beacon_core::model::ShowId;

use crate::ui::covers::Cover;
use crate::ui::models::ShowCard;

const COVER_SIZE: i32 = 160;

#[derive(Debug)]
pub enum ShowsGridInput {
    SetCards(Vec<ShowCard>),
}

#[derive(Debug)]
pub enum ShowsGridOutput {
    OpenShow(ShowId),
    AddPodcast,
}

/// A tile on the grid.
enum Tile {
    Show(ShowCard),
    Add,
}

pub struct ShowsGrid {
    tiles: gio::ListStore,
    empty: bool,
}

#[relm4::component(pub)]
impl SimpleComponent for ShowsGrid {
    type Init = ();
    type Input = ShowsGridInput;
    type Output = ShowsGridOutput;

    view! {
        gtk::Stack {
            #[watch]
            set_visible_child_name: if model.empty { "empty" } else { "grid" },

            add_named[Some("empty")] = &adw::StatusPage {
                set_icon_name: Some("com.theoforger.Beacon-symbolic"),
                set_title: &gettext("No Podcasts"),
                set_description: Some(&gettext("Add a podcast by searching for it or pasting its feed address")),

                #[wrap(Some)]
                set_child = &gtk::Button {
                    set_label: &gettext("Add Podcast"),
                    set_halign: gtk::Align::Center,
                    add_css_class: "pill",
                    add_css_class: "suggested-action",
                    connect_clicked[sender] => move |_| {
                        let _ = sender.output(ShowsGridOutput::AddPodcast);
                    },
                },
            },

            add_named[Some("grid")] = &gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Never,

                #[name = "grid"]
                gtk::GridView {
                    set_max_columns: 12,
                    set_single_click_activate: true,
                    add_css_class: "flat-view",
                    set_margin_top: 12,
                    set_margin_bottom: 12,
                    set_margin_start: 12,
                    set_margin_end: 12,
                    update_property: &[gtk::accessible::Property::Label(&gettext("Shows"))],
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = ShowsGrid {
            tiles: gio::ListStore::new::<glib::BoxedAnyObject>(),
            empty: true,
        };
        let widgets = view_output!();

        widgets
            .grid
            .set_model(Some(&gtk::NoSelection::new(Some(model.tiles.clone()))));
        widgets.grid.set_factory(Some(&tile_factory()));
        widgets.grid.connect_activate(move |grid, position| {
            let Some(object) = grid
                .model()
                .and_then(|m| m.item(position))
                .and_downcast::<glib::BoxedAnyObject>()
            else {
                return;
            };
            let output = match &*object.borrow::<Tile>() {
                Tile::Show(card) => ShowsGridOutput::OpenShow(card.id),
                Tile::Add => ShowsGridOutput::AddPodcast,
            };
            let _ = sender.output(output);
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        let ShowsGridInput::SetCards(cards) = message;
        self.empty = cards.is_empty();
        let tiles: Vec<glib::BoxedAnyObject> = cards
            .into_iter()
            .map(Tile::Show)
            .chain([Tile::Add])
            .map(glib::BoxedAnyObject::new)
            .collect();
        self.tiles.splice(0, self.tiles.n_items(), &tiles);
    }
}

fn tile_factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_bind(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let Some(object) = item.item().and_downcast::<glib::BoxedAnyObject>() else {
            return;
        };
        let child = match &*object.borrow::<Tile>() {
            Tile::Show(card) => show_tile(card),
            Tile::Add => add_tile(),
        };
        item.set_child(Some(&child));
    });
    factory
}

fn show_tile(card: &ShowCard) -> gtk::Box {
    let cover = Cover::new(COVER_SIZE);
    cover.set(card.cover.as_deref());
    let art = gtk::Overlay::builder().child(cover.widget()).build();
    if card.new_count > 0 {
        let badge = gtk::Label::builder()
            .label(card.new_count.to_string())
            .halign(gtk::Align::End)
            .valign(gtk::Align::Start)
            .margin_top(8)
            .margin_end(8)
            .css_classes(["new-badge", "numeric"])
            .build();
        art.add_overlay(&badge);
    }

    let tile = text_tile(&card.title, card.author.as_deref().unwrap_or_default());
    tile.prepend(&art);
    let label = match &card.author {
        // Translators: how a screen reader names a show tile.
        Some(author) => gettext("{title} by {author}")
            .replace("{title}", &card.title)
            .replace("{author}", author),
        None => card.title.clone(),
    };
    let label = if card.new_count > 0 {
        // Translators: a show tile with new episodes, for screen readers.
        gettext("{show}, {count} new")
            .replace("{show}", &label)
            .replace("{count}", &card.new_count.to_string())
    } else {
        label
    };
    tile.update_property(&[gtk::accessible::Property::Label(&label)]);
    tile
}

fn add_tile() -> gtk::Box {
    let icon = gtk::Image::builder()
        .icon_name("list-add-symbolic")
        .pixel_size(32)
        .width_request(COVER_SIZE)
        .height_request(COVER_SIZE)
        .css_classes(["add-tile", "dim-label"])
        .build();
    let tile = text_tile(
        &gettext("Add Podcast"),
        &gettext("Find a show or paste a feed"),
    );
    tile.prepend(&icon);
    tile.update_property(&[gtk::accessible::Property::Label(&gettext("Add Podcast"))]);
    tile
}

fn text_tile(title: &str, subtitle: &str) -> gtk::Box {
    let tile = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .width_request(COVER_SIZE)
        .build();
    let title = gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .margin_top(6)
        .max_width_chars(1)
        .ellipsize(pango::EllipsizeMode::End)
        .css_classes(["heading"])
        .build();
    let subtitle = gtk::Label::builder()
        .label(subtitle)
        .xalign(0.0)
        .max_width_chars(1)
        .ellipsize(pango::EllipsizeMode::End)
        .css_classes(["dim-label", "caption"])
        .build();
    tile.append(&title);
    tile.append(&subtitle);
    tile
}
