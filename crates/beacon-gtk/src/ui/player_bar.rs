// SPDX-License-Identifier: GPL-3.0-or-later

//! The persistent bar across the bottom of the window. Playback arrives with
//! M4; until then it only says that nothing is playing.

use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, gtk};

pub struct PlayerBar;

#[relm4::component(pub)]
impl SimpleComponent for PlayerBar {
    type Init = ();
    type Input = ();
    type Output = ();

    view! {
        gtk::CenterBox {
            add_css_class: "toolbar",
            set_height_request: 64,
            update_property: &[gtk::accessible::Property::Label(&gettext("Playback Controls"))],

            #[wrap(Some)]
            set_center_widget = &gtk::Label {
                set_label: &gettext("Nothing Playing"),
                add_css_class: "dim-label",
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = PlayerBar;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
}
