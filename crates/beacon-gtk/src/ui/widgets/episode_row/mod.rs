// SPDX-License-Identifier: GPL-3.0-or-later

//! An episode in a list: cover, a title of at most two lines with the blue
//! dot before it and the played check and favourite star after it, the
//! subtitle (without the show, which the cover shows), a progress bar when
//! partly played, a flat play button and the row menu. Two controls, as the
//! HIG checklist allows; activating the row opens the details. The
//! child of a `GtkListView` row (`ui/episode_list.rs`), bound again to
//! another episode as the list scrolls; drawn as part of its group's card.
//!
//! In selection mode the play button and menu give way to a check box.
//! The menu's Select, a secondary click or a long press selects.

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

use adw::{prelude::BinExt, subclass::prelude::*};
use beacon_core::model::EpisodeId;
use gettextrs::gettext;
use gtk::{self, gio, glib, pango, prelude::*};

use crate::player::PlaybackState;
use crate::ui::covers::Cover;
use crate::ui::episode_actions;
use crate::ui::format;
use crate::ui::models::{EpisodeItem, RowAction};
use crate::ui::play_button;
use crate::ui::selection::Selection;

mod downloading;
mod menu;
mod reorder;
mod selecting;
use menu::menu_for;

/// Called with the row's episode when one of its actions is chosen.
pub type OnRowAction = Rc<dyn Fn(&EpisodeItem, RowAction)>;

/// What a page's rows share: where their actions go, whether they show
/// their show (not on the show's own page), and the page's selection, if
/// it has selection mode (the Queue view does not).
#[derive(Clone)]
pub struct RowSetup {
    pub on_action: OnRowAction,
    pub show_identity: bool,
    pub selection: Selection,
    pub selectable: bool,
}

thread_local! {
    /// The episode in the player, if any, and what the player is doing.
    static PLAYING: Cell<Option<(EpisodeId, PlaybackState)>> = const { Cell::new(None) };
    /// Whether the window has the narrow layout: the cover is smaller and
    /// Play is left out.
    static NARROW: Cell<bool> = const { Cell::new(false) };
    /// Every row made, so the playing one can show Pause without a reload.
    static ROWS: RefCell<Vec<glib::WeakRef<EpisodeRow>>> = const { RefCell::new(Vec::new()) };
}

/// Tell the rows whether the window has the narrow layout, where the
/// cover is smaller and Play is left out, as the details page a tap
/// away has it (asked by the user 2026-11-08): at 360px the row's fixed
/// parts left the title about 144px.
pub fn set_narrow(narrow: bool) {
    NARROW.set(narrow);
    ROWS.with_borrow(|rows| {
        for row in rows.iter().filter_map(glib::WeakRef::upgrade) {
            row.fit_narrow();
        }
    });
}

/// Tell the rows which episode is in the player and whether it plays: its
/// row's button shows Pause while it does.
pub fn show_playing(playing: Option<(EpisodeId, PlaybackState)>) {
    PLAYING.set(playing);
    ROWS.with_borrow_mut(|rows| {
        rows.retain(|row| match row.upgrade() {
            Some(row) => {
                row.show_play_state();
                true
            }
            None => false,
        });
    });
}

/// Show the downloads under way (`ui/downloads.rs`) on the rows.
pub fn show_downloads() {
    ROWS.with_borrow(|rows| {
        for row in rows.iter().filter_map(glib::WeakRef::upgrade) {
            row.show_download();
        }
    });
}

/// GNOME Podcasts' row cover (`episode_widget.ui`).
const COVER_SIZE: i32 = 64;
/// The cover in the narrow layout.
const NARROW_COVER_SIZE: i32 = 48;
/// Every row's content is at least this tall, as in GNOME Podcasts.
const CONTENT_HEIGHT: i32 = 64;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct EpisodeRow {
        pub parts: OnceCell<super::Parts>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for EpisodeRow {
        const NAME: &'static str = "BeaconEpisodeRow";
        type Type = super::EpisodeRow;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for EpisodeRow {}
    impl WidgetImpl for EpisodeRow {}
    impl BinImpl for EpisodeRow {}
}

glib::wrapper! {
    /// A row that keeps its parts, so it can show another episode, or the
    /// same one changed, without being built again.
    pub struct EpisodeRow(ObjectSubclass<imp::EpisodeRow>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

/// A list view keeps about 200 rows, every widget of which is restyled
/// whenever the list moves to another part of the window (the narrow
/// layout's split views do that), so parts most rows never show are made
/// only when one does: the check box, the favourite star, the progress bar
/// and the cover.
pub struct Parts {
    content: gtk::Box,
    /// The selection check box, made when selection mode first needs it.
    select: OnceCell<gtk::CheckButton>,
    selection: Selection,
    /// Only where the row shows its show.
    cover: Option<Cover>,
    dot: gtk::Box,
    title: gtk::Label,
    check: gtk::Image,
    /// After the played check, made when the row first shows a favourite.
    star: OnceCell<gtk::Image>,
    subtitle: gtk::Label,
    text: gtk::Box,
    /// Made when the row first shows a partly played episode.
    progress: OnceCell<gtk::ProgressBar>,
    /// Made when the row first shows a download under way.
    download: OnceCell<gtk::ProgressBar>,
    menu: gtk::MenuButton,
    play: gtk::Button,
    /// Play with its loading spinner: hidden whole, so its spacing goes too.
    play_holder: gtk::Overlay,
    episode: Rc<RefCell<Option<EpisodeItem>>>,
    /// What a screen reader says for the row (see `bind`).
    name: RefCell<String>,
    /// Whether the row shows its show, by cover (and in its spoken name);
    /// not on the show's own page.
    show_identity: bool,
    /// Whether the row is queued in the Queue view, with a drag handle and
    /// Move Up and Move Down (see `reorder.rs`).
    movable: Rc<Cell<bool>>,
    /// The drag handle, made when the row is first movable.
    handle: OnceCell<gtk::Image>,
    /// Move Up and Move Down, unavailable at the queue's ends.
    move_up: gio::SimpleAction,
    move_down: gio::SimpleAction,
}

impl EpisodeRow {
    /// A row that shows its show, by cover, when
    /// `setup.show_identity` is set; on the show's own page it would only
    /// repeat on every row.
    pub fn new(setup: &RowSetup) -> Self {
        Self::build(setup, glib::Object::new())
    }

    /// A row the queue's card holds itself, outside a list view: an item
    /// of the card's list for screen readers, and focusable, since no list
    /// item takes the focus for it.
    pub fn new_in_card(setup: &RowSetup) -> Self {
        let row: Self = glib::Object::builder()
            .property("accessible-role", gtk::AccessibleRole::ListItem)
            .property("focusable", true)
            .build();
        Self::build(setup, row)
    }

    fn build(setup: &RowSetup, root: Self) -> Self {
        let show_identity = setup.show_identity;
        let selectable = setup.selectable;
        root.add_css_class("episode-card");
        let content = gtk::Box::builder()
            .spacing(12)
            .height_request(CONTENT_HEIGHT)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(12)
            .build();
        root.set_child(Some(&content));
        // GNOME Podcasts shows the cover on every row; Beacon leaves it out
        // on a podcast page, where it is the page's own (asked by the user).
        let size = if NARROW.get() {
            NARROW_COVER_SIZE
        } else {
            COVER_SIZE
        };
        let cover = show_identity.then(|| Cover::new(size));

        // The dot, title, check and subtitle are left out of the
        // accessibility tree: the row's name says what they show (see
        // `bind`), and exposed too they were read a second time, the
        // subtitle's separators as "middle dot".
        let dot = gtk::Box::builder()
            .css_classes(["new-dot"])
            .valign(gtk::Align::Center)
            .accessible_role(gtk::AccessibleRole::Presentation)
            .build();
        let title = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(pango::WrapMode::WordChar)
            .lines(1)
            .ellipsize(pango::EllipsizeMode::End)
            // The list sets the width; see `show_hero.rs`.
            .natural_wrap_mode(gtk::NaturalWrapMode::None)
            .css_classes(["heading"])
            .accessible_role(gtk::AccessibleRole::Presentation)
            .build();
        let check = gtk::Image::builder()
            .icon_name("object-select-symbolic")
            .css_classes(["dim-label"])
            .valign(gtk::Align::Center)
            .accessible_role(gtk::AccessibleRole::Presentation)
            .build();
        let title_line = gtk::Box::builder().spacing(6).build();
        title_line.append(&dot);
        title_line.append(&title);
        title_line.append(&check);

        // Body size, a step up from GNOME Podcasts' caption (asked by the
        // user 2026-11-08), told from the title by weight and dimming.
        let subtitle = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(pango::EllipsizeMode::End)
            .css_classes(["dim-label", "body"])
            .accessible_role(gtk::AccessibleRole::Presentation)
            .build();
        // GNOME Podcasts' spacing between title and subtitle.
        let text = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();
        text.append(&title_line);
        text.append(&subtitle);

        let play = gtk::Button::builder()
            .icon_name("media-playback-start-symbolic")
            .action_name("row.play")
            .tooltip_text(gettext("Play"))
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .build();
        play.update_property(&[gtk::accessible::Property::Label(&gettext("Play"))]);
        let menu = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text(gettext("More Options"))
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .build();
        menu.update_property(&[gtk::accessible::Property::Label(&gettext("More Options"))]);

        if let Some(cover) = &cover {
            content.append(cover.widget());
        }
        content.append(&text);
        let play_holder = play_button::with_spinner(&play);
        content.append(&play_holder);
        content.append(&menu);

        let episode: Rc<RefCell<Option<EpisodeItem>>> = Rc::default();
        // A popover menu builds a widget per item, too much for every row of
        // a long feed; build it on first open, as GNOME Podcasts does.
        let shown = episode.clone();
        let movable = Rc::new(Cell::new(false));
        let moves = movable.clone();
        menu.set_create_popup_func(move |button| {
            if let Some(item) = shown.borrow().as_ref() {
                button.set_menu_model(Some(&menu_for(item, selectable, moves.get())));
            }
        });
        let run = {
            let (episode, on_action) = (episode.clone(), setup.on_action.clone());
            move |action| {
                // Clone first: the action may rebind this row.
                let item = episode.borrow().clone();
                if let Some(item) = item {
                    on_action(&item, action);
                }
            }
        };
        let actions = gio::SimpleActionGroup::new();
        let entry = |name, action| {
            let entry = gio::SimpleAction::new(name, None);
            let run = run.clone();
            entry.connect_activate(move |_, _| run(action));
            actions.add_action(&entry);
            entry
        };
        entry("play", RowAction::Play);
        entry("details", RowAction::ShowDetails);
        let move_up = entry("move-up", RowAction::MoveUp);
        let move_down = entry("move-down", RowAction::MoveDown);
        // Only on the queue's rows (`set_movable`).
        move_up.set_enabled(false);
        move_down.set_enabled(false);
        episode_actions::add_to(&actions, run);
        // The menu's Select: select this episode, starting selection mode.
        let select_action = gio::SimpleAction::new("select", None);
        let (shown, selection) = (episode.clone(), setup.selection.clone());
        select_action.connect_activate(move |_, _| {
            let id = shown.borrow().as_ref().map(|item| item.episode.id);
            if let Some(id) = id {
                selection.set(id, true);
            }
        });
        actions.add_action(&select_action);
        root.insert_action_group("row", Some(&actions));

        let parts = Parts {
            content,
            select: OnceCell::new(),
            selection: setup.selection.clone(),
            cover,
            dot,
            title,
            check,
            star: OnceCell::new(),
            subtitle,
            text,
            progress: OnceCell::new(),
            download: OnceCell::new(),
            menu,
            play,
            play_holder,
            episode,
            name: RefCell::default(),
            show_identity,
            movable,
            handle: OnceCell::new(),
            move_up,
            move_down,
        };
        let _ = root.imp().parts.set(parts);
        if selectable {
            root.connect_selection();
        }
        setup.selection.watch(&root);
        ROWS.with_borrow_mut(|rows| {
            // Rows go with their list's page; drop the gone ones whenever
            // the list doubles, so it never grows without end.
            if rows.len() >= 64 && rows.len().is_power_of_two() {
                rows.retain(|row| row.upgrade().is_some());
            }
            rows.push(root.downgrade());
        });
        root
    }

    /// Two lines of title, or one when the progress bar needs the room
    /// (GNOME Podcasts' `update_progressbar_spacing`); no longer one in wide
    /// windows (asked by the user 2026-11-08).
    fn fit_title(&self) {
        let Some(parts) = self.imp().parts.get() else {
            return;
        };
        let progress = [parts.progress.get(), parts.download.get()]
            .into_iter()
            .flatten()
            .any(gtk::ProgressBar::is_visible);
        parts.title.set_lines(if progress { 1 } else { 2 });
    }

    /// The narrow layout's smaller cover, and Play shown only outside it
    /// and outside selection mode.
    fn fit_narrow(&self) {
        let Some(parts) = self.imp().parts.get() else {
            return;
        };
        let narrow = NARROW.get();
        if let Some(cover) = &parts.cover {
            cover.set_size(if narrow {
                NARROW_COVER_SIZE
            } else {
                COVER_SIZE
            });
        }
        parts
            .play_holder
            .set_visible(!narrow && !parts.selection.is_active());
    }

    /// Play, or Pause while this row's episode plays.
    fn show_play_state(&self) {
        let Some(parts) = self.imp().parts.get() else {
            return;
        };
        let id = parts.episode.borrow().as_ref().map(|item| item.episode.id);
        let state = PLAYING
            .get()
            .filter(|(now, _)| Some(*now) == id)
            .map(|(_, state)| state);
        play_button::set_icon(&parts.play, state);
    }

    /// The episode the row shows.
    pub fn item(&self) -> Option<EpisodeItem> {
        self.imp().parts.get()?.episode.borrow().clone()
    }

    /// What a screen reader says for the row's episode.
    pub fn accessible_name(&self) -> String {
        self.imp()
            .parts
            .get()
            .map(|parts| parts.name.borrow().clone())
            .unwrap_or_default()
    }

    /// Round the top of its group's card when `first`, the bottom when
    /// `last`; a separator under every other row.
    pub fn set_edges(&self, first: bool, last: bool) {
        for (class, on) in [("first", first), ("last", last)] {
            if on {
                self.add_css_class(class);
            } else {
                self.remove_css_class(class);
            }
        }
    }

    pub fn bind(&self, item: &EpisodeItem) {
        let Some(parts) = self.imp().parts.get() else {
            return;
        };
        let episode = &item.episode;
        let state = &episode.state;
        parts.episode.replace(Some(item.clone()));
        self.show_play_state();
        self.show_selection();
        if let Some(cover) = &parts.cover {
            cover.set(item.cover.as_deref());
        }
        parts.dot.set_visible(state.is_new());
        parts.title.set_label(&episode.info.title);
        parts.check.set_visible(state.played);
        // A favourite's star, symbolic and dimmed like the played check
        // (asked by the user 2026-11-08): the Favorites section's icon.
        if state.favorite {
            parts
                .star
                .get_or_init(|| {
                    // 12px: the star fills its square where the check's
                    // glyph is about 12px inside 16, so at 16 it looked
                    // the heavier of the two.
                    let star = gtk::Image::builder()
                        .icon_name("starred-symbolic")
                        .pixel_size(12)
                        .css_classes(["dim-label"])
                        .valign(gtk::Align::Center)
                        .accessible_role(gtk::AccessibleRole::Presentation)
                        .build();
                    if let Some(line) = parts.check.parent().and_downcast::<gtk::Box>() {
                        line.insert_child_after(&star, Some(&parts.check));
                    }
                    star
                })
                .set_visible(true);
        } else if let Some(star) = parts.star.get() {
            star.set_visible(false);
        }
        // Played titles are dimmed as well as checked, as in GNOME Podcasts
        // (`EpisodeWidget::set_played`).
        if state.played {
            parts.title.add_css_class("dim-label");
        } else {
            parts.title.remove_css_class("dim-label");
        }
        let subtitle = format::subtitle(episode);
        parts.subtitle.set_label(&subtitle);
        // Screen readers read a row from its text, " • " included
        // ("bullet"), unless the row has a name: give it one that says it all,
        // for the list to put on the list item that has focus.
        let mut name = vec![episode.info.title.clone()];
        if state.is_new() {
            name.push(gettext("New"));
        }
        if state.played {
            name.push(gettext("Played"));
        }
        if state.favorite {
            name.push(gettext("Favorite"));
        }
        // The show, which only the cover shows, and the cover is not read.
        if parts.show_identity && !item.show_title.is_empty() {
            name.push(item.show_title.clone());
        }
        name.push(format::spoken(&subtitle));
        // Translators: separates the parts of an episode row when a screen
        // reader reads it; a pause, not a symbol.
        parts.name.replace(name.join(&gettext(", ")));
        // Only with a length to measure it against: otherwise the bar could
        // only stay empty (found in the M5 review).
        let shown = episode
            .info
            .duration
            .filter(|total| state.is_partly_played() && !total.is_zero());
        if let Some(total) = shown {
            let progress = parts.progress.get_or_init(|| {
                // Narrower than Adwaita lets a progress bar be (see
                // `style.css`). Added, not set: setting the classes would
                // drop `horizontal`, which Adwaita draws the bar by.
                let progress = gtk::ProgressBar::new();
                progress.add_css_class("episode-progress");
                parts.text.append(&progress);
                progress
            });
            progress.set_visible(true);
            progress.set_fraction((state.position.as_secs_f64() / total.as_secs_f64()).min(1.0));
        } else if let Some(progress) = parts.progress.get() {
            progress.set_visible(false);
        }
        // Over the playback progress, which it hides while it shows.
        self.show_download();
        // Built when opened (below), so drop one worded for the old state.
        if parts.menu.menu_model().is_some() {
            parts.menu.set_menu_model(None::<&gio::MenuModel>);
        }
    }
}
