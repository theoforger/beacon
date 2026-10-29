// SPDX-License-Identifier: GPL-3.0-or-later

//! An episode in a list: cover, a title of at most two lines with the blue
//! dot before it and the played check after it, the subtitle, a progress bar
//! when partly played, a flat play button and the row menu. Two controls,
//! as the HIG checklist allows; the row itself opens the details.

use std::cell::{Cell, OnceCell};
use std::rc::Rc;

use beacon_core::model::EpisodeId;
use gettextrs::gettext;
use relm4::gtk::subclass::prelude::*;
use relm4::gtk::{self, gio, glib, pango, prelude::*};

use crate::ui::covers::Cover;
use crate::ui::format;
use crate::ui::models::{EpisodeItem, RowAction};

/// Called with the row's episode when one of its actions is chosen.
pub type OnRowAction = Rc<dyn Fn(EpisodeId, RowAction)>;

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
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for EpisodeRow {}
    impl WidgetImpl for EpisodeRow {}
    impl BoxImpl for EpisodeRow {}
}

glib::wrapper! {
    /// A `GtkBox` that keeps its parts, so a recycled list row can be bound
    /// to another episode.
    pub struct EpisodeRow(ObjectSubclass<imp::EpisodeRow>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

pub struct Parts {
    cover: Cover,
    dot: gtk::Box,
    title: gtk::Label,
    check: gtk::Image,
    subtitle: gtk::Label,
    progress: gtk::ProgressBar,
    menu: gtk::MenuButton,
    episode: Rc<Cell<Option<EpisodeId>>>,
}

impl EpisodeRow {
    pub fn new(on_action: OnRowAction) -> Self {
        let root: Self = glib::Object::builder()
            .property("spacing", 12)
            .property("margin-top", 6)
            .property("margin-bottom", 6)
            .property("margin-start", 6)
            .property("margin-end", 6)
            .build();
        let cover = Cover::new(COVER_SIZE);

        let dot = gtk::Box::builder()
            .css_classes(["new-dot"])
            .valign(gtk::Align::Center)
            .accessible_role(gtk::AccessibleRole::Img)
            .build();
        dot.update_property(&[gtk::accessible::Property::Label(&gettext("New"))]);
        let title = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(pango::WrapMode::WordChar)
            .lines(2)
            .ellipsize(pango::EllipsizeMode::End)
            .css_classes(["heading"])
            .build();
        let check = gtk::Image::builder()
            .icon_name("object-select-symbolic")
            .css_classes(["dim-label"])
            .valign(gtk::Align::Center)
            .accessible_role(gtk::AccessibleRole::Img)
            .build();
        check.update_property(&[gtk::accessible::Property::Label(&gettext("Played"))]);
        let title_line = gtk::Box::builder().spacing(6).build();
        title_line.append(&dot);
        title_line.append(&title);
        title_line.append(&check);

        let subtitle = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(pango::EllipsizeMode::End)
            .css_classes(["dim-label", "caption"])
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
            // Enabled with playback (M4).
            .action_name("row.play")
            .tooltip_text(gettext("Play"))
            .valign(gtk::Align::Center)
            .css_classes(["flat", "circular"])
            .build();
        play.update_property(&[gtk::accessible::Property::Label(&gettext("Play"))]);
        let menu = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text(gettext("More Options"))
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .build();
        menu.update_property(&[gtk::accessible::Property::Label(&gettext("More Options"))]);

        root.append(cover.widget());
        root.append(&text);
        root.append(&play);
        root.append(&menu);

        let episode: Rc<Cell<Option<EpisodeId>>> = Rc::default();
        let actions = gio::SimpleActionGroup::new();
        for (name, action) in [
            ("details", RowAction::ShowDetails),
            ("queue", RowAction::ToggleQueue),
            ("favorite", RowAction::ToggleFavorite),
            ("played", RowAction::TogglePlayed),
        ] {
            let entry = gio::SimpleAction::new(name, None);
            let episode = episode.clone();
            let on_action = on_action.clone();
            entry.connect_activate(move |_, _| {
                if let Some(id) = episode.get() {
                    on_action(id, action);
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
            episode,
        };
        let _ = root.imp().parts.set(parts);
        root
    }

    pub fn bind(&self, item: &EpisodeItem) {
        let Some(parts) = self.imp().parts.get() else {
            return;
        };
        let episode = &item.episode;
        let state = &episode.state;
        parts.episode.set(Some(episode.id));
        parts.cover.set(item.cover.as_deref());
        parts.dot.set_visible(state.is_new());
        parts.title.set_label(&episode.info.title);
        parts.check.set_visible(state.played);
        parts
            .subtitle
            .set_label(&format::subtitle(episode, &item.show_title));
        parts.progress.set_visible(state.is_partly_played());
        if let Some(total) = episode.info.duration.filter(|d| !d.is_zero()) {
            parts
                .progress
                .set_fraction((state.position.as_secs_f64() / total.as_secs_f64()).min(1.0));
        }
        parts.menu.set_menu_model(Some(&menu(item)));
    }
}

/// The row menu, worded for the episode's current state.
fn menu(item: &EpisodeItem) -> gio::Menu {
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
