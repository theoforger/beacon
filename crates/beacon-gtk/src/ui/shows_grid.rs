// SPDX-License-Identifier: GPL-3.0-or-later

//! The Shows section: a grid of cover tiles with a badge for new episodes,
//! ending in an Add Podcast tile. With no subscriptions it is an empty state
//! with an Add Podcast button.

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{self, gio, glib, pango};

use beacon_core::model::ShowId;

use crate::ui::covers::{Cover, square};
use crate::ui::messages::Output;
use crate::ui::models::ShowCard;

/// The narrowest a cover gets, as in GNOME Podcasts. Covers grow to fill
/// their column; the gaps between them stay the same.
const COVER_MIN: i32 = 150;

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

mod imp {
    use std::cell::{OnceCell, RefCell};

    use super::*;

    #[derive(Default)]
    pub struct ShowsGrid {
        pub stack: OnceCell<gtk::Stack>,
        pub tiles: OnceCell<gio::ListStore>,
        /// The cards shown, so a reload that changes nothing rebuilds nothing.
        pub cards: RefCell<Option<Vec<ShowCard>>>,
        pub output: Output<ShowsGridOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ShowsGrid {
        const NAME: &'static str = "BeaconShowsGrid";
        type Type = super::ShowsGrid;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for ShowsGrid {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }
    impl WidgetImpl for ShowsGrid {}
    impl BinImpl for ShowsGrid {}
}

glib::wrapper! {
    pub struct ShowsGrid(ObjectSubclass<imp::ShowsGrid>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for ShowsGrid {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl ShowsGrid {
    fn build(&self) {
        let output = self.imp().output.clone();
        let add = gtk::Button::builder()
            .label(gettext("Add Podcast"))
            .halign(gtk::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .build();
        add.connect_clicked({
            let output = output.clone();
            move |_| output.emit(ShowsGridOutput::AddPodcast)
        });
        let empty = adw::StatusPage::builder()
            .icon_name("com.theoforger.Beacon-symbolic")
            .title(gettext("No Podcasts"))
            .description(gettext(
                "Add a podcast by searching for it or pasting its feed address",
            ))
            .child(&add)
            .build();

        // As in GNOME Podcasts: 2 to 7 columns, fixed margins around each
        // tile (`.shows-grid`), and rows as tall as their tiles' natural
        // height, so covers stay square.
        let tiles = gio::ListStore::new::<glib::BoxedAnyObject>();
        let grid = gtk::GridView::builder()
            .min_columns(2)
            .max_columns(7)
            .vscroll_policy(gtk::ScrollablePolicy::Natural)
            .single_click_activate(true)
            .css_classes(["shows-grid"])
            .model(&gtk::NoSelection::new(Some(tiles.clone())))
            .factory(&tile_factory())
            .build();
        grid.update_property(&[gtk::accessible::Property::Label(&gettext("Shows"))]);
        grid.connect_activate(move |grid, position| {
            let Some(object) = grid
                .model()
                .and_then(|m| m.item(position))
                .and_downcast::<glib::BoxedAnyObject>()
            else {
                return;
            };
            let message = match &*object.borrow::<Tile>() {
                Tile::Show(card) => ShowsGridOutput::OpenShow(card.id),
                Tile::Add => ShowsGridOutput::AddPodcast,
            };
            output.emit(message);
        });
        // The HIG asks for a maximum width, against long rows.
        let clamp = adw::ClampScrollable::builder()
            .maximum_size(1200)
            .vscroll_policy(gtk::ScrollablePolicy::Natural)
            .child(&grid)
            .build();
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&clamp)
            .build();

        let stack = gtk::Stack::new();
        stack.add_named(&empty, Some("empty"));
        stack.add_named(&scrolled, Some("grid"));
        stack.set_visible_child_name("empty");
        self.set_child(Some(&stack));
        let imp = self.imp();
        let _ = imp.stack.set(stack);
        let _ = imp.tiles.set(tiles);
    }

    pub fn connect_output(&self, receive: impl Fn(ShowsGridOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    pub fn send(&self, input: ShowsGridInput) {
        let ShowsGridInput::SetCards(cards) = input;
        let imp = self.imp();
        if imp.cards.borrow().as_ref() == Some(&cards) {
            return;
        }
        let empty = cards.is_empty();
        imp.cards.replace(Some(cards.clone()));
        let tiles: Vec<glib::BoxedAnyObject> = cards
            .into_iter()
            .map(Tile::Show)
            .chain([Tile::Add])
            .map(glib::BoxedAnyObject::new)
            .collect();
        let store = imp.tiles.get().expect("built when constructed");
        store.splice(0, store.n_items(), &tiles);
        imp.stack
            .get()
            .expect("built when constructed")
            .set_visible_child_name(if empty { "empty" } else { "grid" });
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

pub(crate) fn show_tile(card: &ShowCard) -> gtk::Box {
    let cover = Cover::filling(COVER_MIN);
    cover.set(card.cover.as_deref());
    let art = cover.widget();
    // On the cover itself, so the badge stays in its corner at any width.
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
    tile.prepend(art);
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
        .build();
    let art = gtk::Overlay::builder()
        .child(&square(COVER_MIN))
        .css_classes(["add-tile", "dim-label"])
        .build();
    art.add_overlay(&icon);
    let tile = text_tile(
        &gettext("Add Podcast"),
        &gettext("Find a show or paste a feed"),
    );
    tile.prepend(&art);
    tile.update_property(&[gtk::accessible::Property::Label(&gettext("Add Podcast"))]);
    tile
}

fn text_tile(title: &str, subtitle: &str) -> gtk::Box {
    let tile = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
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
