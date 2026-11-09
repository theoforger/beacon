// SPDX-License-Identifier: GPL-3.0-or-later

//! The main window. It holds the `App` and handles what reaches it, one
//! message or result at a time, in the order they arrived.

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use super::sender::{AppSender, Event};
use super::{App, AppWidgets};

mod imp {
    use std::cell::{OnceCell, RefCell};

    use super::*;

    #[derive(Default)]
    pub struct Window {
        pub app: RefCell<Option<App>>,
        pub widgets: OnceCell<AppWidgets>,
        pub sender: OnceCell<AppSender>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "BeaconWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;
    }

    impl ObjectImpl for Window {}
    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new(application: &adw::Application) -> Self {
        let window: Self = glib::Object::builder()
            .property("application", application)
            .build();
        let (sender, events) = AppSender::new();
        let (app, widgets) = App::init(window.upcast_ref(), &sender);
        let imp = window.imp();
        imp.app.replace(Some(app));
        let _ = imp.widgets.set(widgets);
        let _ = imp.sender.set(sender);

        let this = window.downgrade();
        glib::spawn_future_local(async move {
            while let Ok(event) = events.recv().await {
                let Some(window) = this.upgrade() else {
                    break;
                };
                window.handle(event);
            }
        });
        window
    }

    fn handle(&self, event: Event) {
        let imp = self.imp();
        let (Some(widgets), Some(sender)) = (imp.widgets.get(), imp.sender.get()) else {
            return;
        };
        let mut app = imp.app.borrow_mut();
        let Some(app) = app.as_mut() else {
            return;
        };
        match event {
            Event::Message(message) => app.update(message, widgets, sender, self.upcast_ref()),
            Event::Command(command) => app.update_cmd(command, widgets, sender),
        }
    }
}
