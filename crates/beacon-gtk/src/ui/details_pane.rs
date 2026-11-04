// SPDX-License-Identifier: GPL-3.0-or-later

//! The expandable right pane, laid out as in the mockup: cover, show, title,
//! subtitle, then About This Episode. The actions between the subtitle and
//! the description, and the chapters after it, arrive with M4.

use std::path::PathBuf;

use beacon_core::model::EpisodeId;
use beacon_core::text;
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::gtk::pango;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};

use crate::ui::covers::Cover;
use crate::ui::format;
use crate::ui::links;
use crate::ui::models::EpisodeItem;

/// As in the mockup at 1440px wide.
const COVER_SIZE: i32 = 176;

#[derive(Debug)]
pub enum DetailsPaneInput {
    Show(Box<EpisodeItem>),
    /// The episode's own cover, once fetched; the show's stands in until
    /// then, and for episodes without one.
    EpisodeCover(EpisodeId, PathBuf),
    Clear,
}

#[derive(Debug)]
pub enum DetailsPaneOutput {
    /// The pane's own Back, shown in the narrow layout.
    Close,
}

pub struct DetailsPane {
    episode: Option<EpisodeItem>,
    cover: Cover,
    /// The shown episode's own cover, when it has one and it has arrived.
    own_cover: Option<(EpisodeId, PathBuf)>,
    /// The long subtitle and the description's markup, made once per
    /// episode rather than on every update.
    subtitle: String,
    description: String,
}

#[relm4::component(pub)]
impl SimpleComponent for DetailsPane {
    type Init = ();
    type Input = DetailsPaneInput;
    type Output = DetailsPaneOutput;

    view! {
        adw::ToolbarView {
            add_top_bar = &adw::HeaderBar {
                set_title_widget: Some(&adw::WindowTitle::new(&gettext("Episode Details"), "")),

                #[name = "back"]
                pack_start = &gtk::Button {
                    set_icon_name: "go-previous-symbolic",
                    set_visible: false,
                    set_tooltip_text: Some(&gettext("Back")),
                    update_property: &[gtk::accessible::Property::Label(&gettext("Back"))],
                    connect_clicked[sender] => move |_| {
                        let _ = sender.output(DetailsPaneOutput::Close);
                    },
                },
            },

            #[wrap(Some)]
            #[name = "stack"]
            set_content = &gtk::Stack {

                add_named[Some("empty")] = &adw::StatusPage {
                    set_icon_name: Some("audio-x-generic-symbolic"),
                    set_title: &gettext("No Episode Selected"),
                    set_description: Some(&gettext("Choose an episode to see its details here")),
                },

                add_named[Some("episode")] = &gtk::ScrolledWindow {
                    set_hscrollbar_policy: gtk::PolicyType::Never,

                    adw::Clamp {
                        set_maximum_size: 600,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 6,
                            set_margin_top: 18,
                            set_margin_bottom: 18,
                            set_margin_start: 18,
                            set_margin_end: 18,

                            append: model.cover.widget(),

                            gtk::Label {
                                set_margin_top: 12,
                                set_xalign: 0.0,
                                set_wrap: true,
                                add_css_class: "dim-label",
                                add_css_class: "caption-heading",
                                #[watch]
                                set_label: model.episode.as_ref().map(|e| e.show_title.as_str()).unwrap_or_default(),
                            },
                            gtk::Label {
                                set_xalign: 0.0,
                                set_wrap: true,
                                set_wrap_mode: pango::WrapMode::WordChar,
                                add_css_class: "title-2",
                                set_accessible_role: gtk::AccessibleRole::Heading,
                                #[watch]
                                set_label: model.episode.as_ref().map(|e| e.episode.info.title.as_str()).unwrap_or_default(),
                            },
                            gtk::Label {
                                set_xalign: 0.0,
                                set_wrap: true,
                                add_css_class: "dim-label",
                                #[watch]
                                set_label: &model.subtitle,
                                #[watch]
                                update_property: &[gtk::accessible::Property::Label(&format::spoken(&model.subtitle))],
                            },
                            gtk::Label {
                                set_margin_top: 18,
                                set_xalign: 0.0,
                                set_label: &gettext("About This Episode"),
                                add_css_class: "heading",
                                set_accessible_role: gtk::AccessibleRole::Heading,
                                #[watch]
                                set_visible: model.episode.as_ref().is_some_and(|e| e.episode.info.description.is_some()),
                            },
                            #[name = "description"]
                            gtk::Label {
                                set_xalign: 0.0,
                                set_wrap: true,
                                set_wrap_mode: pango::WrapMode::WordChar,
                                // Links, bold, italics and lists, as GNOME
                                // Podcasts shows them, and like it not
                                // selectable: a selectable label selects all
                                // its text when it takes focus. Links open in
                                // the browser through the portal.
                                set_use_markup: true,
                                #[watch]
                                set_markup: &model.description,
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = DetailsPane {
            episode: None,
            cover: Cover::new(COVER_SIZE),
            own_cover: None,
            subtitle: String::new(),
            description: String::new(),
        };
        let widgets = view_output!();
        links::arrow_keys_between_links(&widgets.description);
        ComponentParts { model, widgets }
    }

    fn post_view() {
        stack.set_visible_child_name(if model.episode.is_some() {
            "episode"
        } else {
            "empty"
        });
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            DetailsPaneInput::Show(item) => {
                // A reload of the same episode keeps the cover it has.
                if self.own_cover.as_ref().map(|(id, _)| *id) != Some(item.episode.id) {
                    self.own_cover = None;
                }
                let cover = self.own_cover.as_ref().map(|(_, path)| path.as_path());
                self.cover.set(cover.or(item.cover.as_deref()));
                let info = &item.episode.info;
                let changed = self.episode.as_ref().map(|e| &e.episode.info) != Some(info);
                if changed {
                    self.subtitle = format::long_subtitle(&item.episode);
                    self.description = info
                        .description
                        .as_deref()
                        .map(text::markup)
                        .unwrap_or_default();
                }
                self.episode = Some(*item);
            }
            DetailsPaneInput::EpisodeCover(id, path) => {
                if self.episode.as_ref().map(|e| e.episode.id) == Some(id) {
                    self.cover.set(Some(&path));
                    self.own_cover = Some((id, path));
                }
            }
            DetailsPaneInput::Clear => {
                self.episode = None;
                self.own_cover = None;
                self.subtitle.clear();
                self.description.clear();
            }
        }
    }
}

impl DetailsPaneWidgets {
    /// Shown only in the narrow layout, where the pane covers the window.
    pub fn back_button(&self) -> &gtk::Button {
        &self.back
    }
}
