// SPDX-License-Identifier: GPL-3.0-or-later

//! The top of a podcast page: cover, category, title, author, description,
//! Unsubscribe and More Options, then the All / Unplayed toggle group and the
//! sort menu. Its menus use the page's `show.*` actions.

use beacon_core::model::{EpisodeFilter, SortOrder};
use beacon_core::text;
use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk::{self, gio, pango};

use crate::ui::covers::Cover;
use crate::ui::models::ShowPageData;

const COVER_SIZE: i32 = 160;

pub struct ShowHero {
    root: gtk::Box,
    cover: Cover,
    category: gtk::Label,
    title: gtk::Label,
    author: gtk::Label,
    description: gtk::Label,
    pub unsubscribe: gtk::Button,
    pub filter: adw::ToggleGroup,
}

impl ShowHero {
    pub fn new() -> Self {
        let cover = Cover::new(COVER_SIZE);
        let label = |classes: &[&str], lines: i32| {
            gtk::Label::builder()
                .wrap(true)
                .wrap_mode(pango::WrapMode::WordChar)
                .justify(gtk::Justification::Center)
                .lines(lines)
                .ellipsize(pango::EllipsizeMode::End)
                .css_classes(classes)
                .build()
        };
        let category = label(&["dim-label", "caption-heading"], 1);
        let title = label(&["title-1"], 3);
        title.set_accessible_role(gtk::AccessibleRole::Heading);
        let author = label(&["dim-label"], 2);
        let description = label(&[], 6);
        description.set_margin_top(6);
        description.set_selectable(true);

        let unsubscribe = gtk::Button::builder()
            .label(gettext("_Unsubscribe"))
            .use_underline(true)
            .build();
        let more = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text(gettext("More Options"))
            .menu_model(&more_menu())
            .valign(gtk::Align::Center)
            .build();
        more.update_property(&[gtk::accessible::Property::Label(&gettext("More Options"))]);
        let actions = gtk::Box::builder()
            .spacing(12)
            .halign(gtk::Align::Center)
            .margin_top(12)
            .build();
        actions.append(&unsubscribe);
        actions.append(&more);

        let filter = adw::ToggleGroup::builder().homogeneous(true).build();
        filter.add(
            adw::Toggle::builder()
                .name("all")
                .label(gettext("All"))
                .build(),
        );
        filter.add(
            adw::Toggle::builder()
                .name("unplayed")
                .label(gettext("Unplayed"))
                .build(),
        );
        filter
            .upcast_ref::<gtk::Widget>()
            .update_property(&[gtk::accessible::Property::Label(&gettext("Show Episodes"))]);
        let sort = gtk::MenuButton::builder()
            .icon_name("view-sort-descending-symbolic")
            .tooltip_text(gettext("Sort Episodes"))
            .menu_model(&sort_menu())
            .css_classes(["flat"])
            .build();
        sort.update_property(&[gtk::accessible::Property::Label(&gettext("Sort Episodes"))]);
        let controls = gtk::Box::builder().spacing(6).margin_top(18).build();
        let spacer = gtk::Box::builder().hexpand(true).build();
        controls.append(&filter);
        controls.append(&spacer);
        controls.append(&sort);

        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .margin_top(24)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();
        root.append(cover.widget());
        category.set_margin_top(12);
        root.append(&category);
        root.append(&title);
        root.append(&author);
        root.append(&description);
        root.append(&actions);
        root.append(&controls);

        Self {
            root,
            cover,
            category,
            title,
            author,
            description,
            unsubscribe,
            filter,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn update(&self, data: &ShowPageData) {
        let info = &data.show.info;
        self.cover.set(data.show.cover_path.as_deref());
        set_text(&self.category, info.category.as_deref());
        self.title.set_label(&info.title);
        set_text(&self.author, info.author.as_deref());
        let description = info.description.as_deref().map(text::plain);
        set_text(&self.description, description.as_deref());
        let name = match data.prefs.filter {
            EpisodeFilter::All => "all",
            EpisodeFilter::Unplayed => "unplayed",
        };
        if self.filter.active_name().as_deref() != Some(name) {
            self.filter.set_active_name(Some(name));
        }
    }
}

fn set_text(label: &gtk::Label, text: Option<&str>) {
    let text = text.unwrap_or_default();
    label.set_label(text);
    label.set_visible(!text.is_empty());
}

fn more_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some(&gettext("Mark All as _Played")),
        Some("show.mark-all-played"),
    );
    // Downloads arrive in M5; the item stays disabled until then.
    menu.append(
        Some(&gettext("_Download All Episodes")),
        Some("show.download-all"),
    );
    menu.append(Some(&gettext("_Copy Feed Address")), Some("show.copy-feed"));
    menu
}

fn sort_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    for (label, order) in [
        (gettext("_Newest First"), SortOrder::NewestFirst),
        (gettext("_Oldest First"), SortOrder::OldestFirst),
    ] {
        let item = gio::MenuItem::new(Some(&label), None);
        item.set_action_and_target_value(Some("show.sort"), Some(&sort_name(order).to_variant()));
        menu.append_item(&item);
    }
    menu
}

pub fn sort_name(order: SortOrder) -> &'static str {
    match order {
        SortOrder::NewestFirst => "newest",
        SortOrder::OldestFirst => "oldest",
    }
}

pub fn sort_from(name: &str) -> SortOrder {
    match name {
        "oldest" => SortOrder::OldestFirst,
        _ => SortOrder::NewestFirst,
    }
}
