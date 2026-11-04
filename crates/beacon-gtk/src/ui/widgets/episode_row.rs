// SPDX-License-Identifier: GPL-3.0-or-later

//! An episode in a list: cover, a title of at most two lines with the blue
//! dot before it and the played check after it, the subtitle, a progress bar
//! when partly played, a flat play button and the row menu. Two controls,
//! as the HIG checklist allows; activating the row opens the details. A
//! `GtkListBoxRow` for a `boxed-list`, as GNOME Podcasts' episode row is.

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

use beacon_core::model::EpisodeId;
use gettextrs::gettext;
use relm4::gtk::subclass::prelude::*;
use relm4::gtk::{self, gio, glib, pango, prelude::*};

use crate::ui::covers::Cover;
use crate::ui::format;
use crate::ui::models::{EpisodeItem, RowAction};

/// Called with the row's episode when one of its actions is chosen.
pub type OnRowAction = Rc<dyn Fn(&EpisodeItem, RowAction)>;

thread_local! {
    /// The episode playing, if any, and whether it is playing or paused.
    static PLAYING: Cell<Option<(EpisodeId, bool)>> = const { Cell::new(None) };
    /// Every row made, so the playing one can show Pause without a reload.
    static ROWS: RefCell<Vec<glib::WeakRef<EpisodeRow>>> = const { RefCell::new(Vec::new()) };
}

/// Tell the rows which episode is in the player and whether it plays: its
/// row's button shows Pause while it does.
pub fn show_playing(playing: Option<(EpisodeId, bool)>) {
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

const COVER_SIZE: i32 = 48;

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
        type ParentType = gtk::ListBoxRow;
    }

    impl ObjectImpl for EpisodeRow {}
    impl WidgetImpl for EpisodeRow {}
    impl ListBoxRowImpl for EpisodeRow {}
}

glib::wrapper! {
    /// A list box row that keeps its parts, so it can be updated in place
    /// when the episode's state changes.
    pub struct EpisodeRow(ObjectSubclass<imp::EpisodeRow>)
        @extends gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

pub struct Parts {
    cover: Cover,
    dot: gtk::Box,
    title: gtk::Label,
    check: gtk::Image,
    subtitle: gtk::Label,
    progress: gtk::ProgressBar,
    menu: gtk::MenuButton,
    play: gtk::Button,
    episode: Rc<RefCell<Option<EpisodeItem>>>,
    /// Whether the row shows its show, by cover and in the subtitle; not
    /// on the show's own page.
    show_identity: bool,
}

impl EpisodeRow {
    /// A row that shows its show, by cover and in the subtitle, when
    /// `show_identity` is set; on the show's own page it would only repeat
    /// on every row.
    pub fn new(on_action: OnRowAction, show_identity: bool) -> Self {
        let root: Self = glib::Object::new();
        let content = gtk::Box::builder()
            .spacing(12)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(12)
            .margin_end(12)
            .build();
        root.set_child(Some(&content));
        let cover = Cover::new(COVER_SIZE);

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
            .lines(2)
            .ellipsize(pango::EllipsizeMode::End)
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

        let subtitle = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(pango::EllipsizeMode::End)
            .css_classes(["dim-label", "caption"])
            .accessible_role(gtk::AccessibleRole::Presentation)
            .build();
        let progress = gtk::ProgressBar::builder().margin_top(4).build();
        let text = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();
        text.append(&title_line);
        text.append(&subtitle);
        text.append(&progress);

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

        // GNOME Podcasts shows the cover on every row; Beacon leaves it out
        // on a podcast page, where it is the page's own (asked by the user).
        cover.widget().set_visible(show_identity);
        content.append(cover.widget());
        content.append(&text);
        content.append(&play);
        content.append(&menu);

        let episode: Rc<RefCell<Option<EpisodeItem>>> = Rc::default();
        // A popover menu builds a widget per item, too much for every row of
        // a long feed; build it on first open, as GNOME Podcasts does.
        let shown = episode.clone();
        menu.set_create_popup_func(move |button| {
            if let Some(item) = shown.borrow().as_ref() {
                button.set_menu_model(Some(&menu_for(item)));
            }
        });
        let actions = gio::SimpleActionGroup::new();
        for (name, action) in [
            ("play", RowAction::Play),
            ("details", RowAction::ShowDetails),
            ("queue", RowAction::ToggleQueue),
            ("favorite", RowAction::ToggleFavorite),
            ("played", RowAction::TogglePlayed),
        ] {
            let entry = gio::SimpleAction::new(name, None);
            let episode = episode.clone();
            let on_action = on_action.clone();
            entry.connect_activate(move |_, _| {
                // Clone first: the action may rebind this row.
                let item = episode.borrow().clone();
                if let Some(item) = item {
                    on_action(&item, action);
                }
            });
            actions.add_action(&entry);
        }
        root.insert_action_group("row", Some(&actions));

        let parts = Parts {
            cover,
            dot,
            title,
            check,
            subtitle,
            progress,
            menu,
            play,
            episode,
            show_identity,
        };
        let _ = root.imp().parts.set(parts);
        ROWS.with_borrow_mut(|rows| rows.push(root.downgrade()));
        root
    }

    /// Play, or Pause while this row's episode plays.
    fn show_play_state(&self) {
        let Some(parts) = self.imp().parts.get() else {
            return;
        };
        let id = parts.episode.borrow().as_ref().map(|item| item.episode.id);
        let playing = matches!(PLAYING.get(), Some((now, true)) if Some(now) == id);
        let (icon, label) = if playing {
            ("media-playback-pause-symbolic", gettext("Pause"))
        } else {
            ("media-playback-start-symbolic", gettext("Play"))
        };
        parts.play.set_icon_name(icon);
        parts.play.set_tooltip_text(Some(&label));
        parts
            .play
            .update_property(&[gtk::accessible::Property::Label(&label)]);
    }

    /// The episode the row shows.
    pub fn item(&self) -> Option<EpisodeItem> {
        self.imp().parts.get()?.episode.borrow().clone()
    }

    pub fn bind(&self, item: &EpisodeItem) {
        let Some(parts) = self.imp().parts.get() else {
            return;
        };
        let episode = &item.episode;
        let state = &episode.state;
        parts.episode.replace(Some(item.clone()));
        self.show_play_state();
        if parts.show_identity {
            parts.cover.set(item.cover.as_deref());
        }
        parts.dot.set_visible(state.is_new());
        parts.title.set_label(&episode.info.title);
        parts.check.set_visible(state.played);
        // Played titles are dimmed as well as checked, as in GNOME Podcasts
        // (`EpisodeWidget::set_played`).
        if state.played {
            parts.title.add_css_class("dim-label");
        } else {
            parts.title.remove_css_class("dim-label");
        }
        let show_title = if parts.show_identity {
            item.show_title.as_str()
        } else {
            ""
        };
        let subtitle = format::subtitle(episode, show_title);
        parts.subtitle.set_label(&subtitle);
        // Screen readers read a row from its text, " · " included ("middle
        // dot"), unless the row has a name: give it one that says it all.
        let mut name = vec![episode.info.title.clone()];
        if state.is_new() {
            name.push(gettext("New"));
        }
        if state.played {
            name.push(gettext("Played"));
        }
        name.push(format::spoken(&subtitle));
        // Translators: separates the parts of an episode row when a screen
        // reader reads it; a pause, not a symbol.
        self.update_property(&[gtk::accessible::Property::Label(&name.join(&gettext(", ")))]);
        parts.progress.set_visible(state.is_partly_played());
        if let Some(total) = episode.info.duration.filter(|d| !d.is_zero()) {
            parts
                .progress
                .set_fraction((state.position.as_secs_f64() / total.as_secs_f64()).min(1.0));
        }
        // Built when opened (below), so drop one worded for the old state.
        if parts.menu.menu_model().is_some() {
            parts.menu.set_menu_model(None::<&gio::MenuModel>);
        }
    }
}

/// The row menu, worded for the episode's current state.
fn menu_for(item: &EpisodeItem) -> gio::Menu {
    let state = &item.episode.state;
    let details = gio::Menu::new();
    details.append(Some(&gettext("Show _Details")), Some("row.details"));

    let changes = gio::Menu::new();
    changes.append(
        Some(&if item.in_queue {
            gettext("Remove From _Queue")
        } else {
            gettext("Add to _Queue")
        }),
        Some("row.queue"),
    );
    // Downloads arrive in M5; the item stays disabled until then.
    changes.append(Some(&gettext("_Download")), Some("row.download"));
    changes.append(
        Some(&if state.favorite {
            gettext("Remove From _Favorites")
        } else {
            gettext("Add to _Favorites")
        }),
        Some("row.favorite"),
    );
    changes.append(
        Some(&if state.played {
            gettext("Mark as _Unplayed")
        } else {
            gettext("Mark as _Played")
        }),
        Some("row.played"),
    );

    let menu = gio::Menu::new();
    menu.append_section(None, &details);
    menu.append_section(None, &changes);
    menu
}
