// SPDX-License-Identifier: GPL-3.0-or-later

//! The expandable right pane. Its contents arrive with M4; for now it says
//! that nothing is selected.

use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};

#[derive(Debug)]
pub enum DetailsPaneOutput {
    /// The pane's own Back, shown in the narrow layout.
    Close,
}

pub struct DetailsPane;

#[relm4::component(pub)]
impl SimpleComponent for DetailsPane {
    type Init = ();
    type Input = ();
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
            set_content = &adw::StatusPage {
                set_icon_name: Some("audio-x-generic-symbolic"),
                set_title: &gettext("No Episode Selected"),
                set_description: Some(&gettext("Choose an episode to see its details here")),
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = DetailsPane;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
}

impl DetailsPaneWidgets {
    /// Shown only in the narrow layout, where the pane covers the window.
    pub fn back_button(&self) -> &gtk::Button {
        &self.back
    }
}
