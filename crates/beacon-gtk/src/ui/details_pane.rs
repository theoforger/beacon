// SPDX-License-Identifier: GPL-3.0-or-later

//! The expandable right pane. M3 shows the chosen episode's cover, show,
//! title, subtitle and description; actions and chapters arrive with M4.

use beacon_core::text;
use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::gtk::pango;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};

use crate::ui::covers::Cover;
use crate::ui::format;
use crate::ui::models::EpisodeItem;

const COVER_SIZE: i32 = 200;

#[derive(Debug)]
pub enum DetailsPaneInput {
    Show(Box<EpisodeItem>),
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
                                set_label: &model.episode.as_ref().map(|e| format::subtitle(&e.episode, "")).unwrap_or_default(),
                            },
                            gtk::Label {
                                set_margin_top: 12,
                                set_xalign: 0.0,
                                set_wrap: true,
                                set_wrap_mode: pango::WrapMode::WordChar,
                                set_selectable: true,
                                #[watch]
                                set_label: &model.episode.as_ref().and_then(|e| e.episode.info.description.as_deref()).map(text::plain).unwrap_or_default(),
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
        };
        let widgets = view_output!();
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
                self.cover.set(item.cover.as_deref());
                self.episode = Some(*item);
            }
            DetailsPaneInput::Clear => self.episode = None,
        }
    }
}

impl DetailsPaneWidgets {
    /// Shown only in the narrow layout, where the pane covers the window.
    pub fn back_button(&self) -> &gtk::Button {
        &self.back
    }
}
