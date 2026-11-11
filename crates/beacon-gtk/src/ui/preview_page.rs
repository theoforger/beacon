// SPDX-License-Identifier: GPL-3.0-or-later

//! A show from Apple's directory, before subscribing: pushed over the Add
//! page, laid out as a podcast page is, with the accent Subscribe where
//! Unsubscribe goes. It opens at once with what the directory said, then
//! fills in from the show's own feed (its cover, description and newest
//! episodes) once that has loaded. Nothing is stored until Subscribe.

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};
use url::Url;

use crate::ui::messages::Output;
use crate::ui::show_hero::{HeroData, ShowHero};

pub const PREVIEW_TAG: &str = "preview";
/// How many of the newest episodes the preview lists.
pub const PREVIEW_EPISODES: usize = 10;

/// The feed, as the preview shows it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreviewData {
    pub hero: HeroData,
    /// The newest episodes' titles and subtitles.
    pub episodes: Vec<(String, String)>,
}

#[derive(Debug)]
pub enum PreviewPageInput {
    /// Open on a show: what the directory said, while its feed loads.
    Start {
        hero: Box<HeroData>,
        feed: Url,
        store: Option<Url>,
    },
    Loaded(Box<PreviewData>),
    /// The feed could not be loaded: why.
    Failed(String),
    /// The subscription finished, either way.
    Done,
}

#[derive(Debug)]
pub enum PreviewPageOutput {
    Subscribe(Url),
    FeedAddressCopied,
}

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};

    use super::*;

    #[derive(Default)]
    pub struct PreviewPage {
        pub parts: OnceCell<super::Parts>,
        pub feed: RefCell<Option<Url>>,
        pub store: RefCell<Option<Url>>,
        pub loaded: Cell<bool>,
        pub subscribing: Cell<bool>,
        pub output: Output<PreviewPageOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreviewPage {
        const NAME: &'static str = "BeaconPreviewPage";
        type Type = super::PreviewPage;
        type ParentType = adw::NavigationPage;
    }

    impl ObjectImpl for PreviewPage {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }
    impl WidgetImpl for PreviewPage {}
    impl NavigationPageImpl for PreviewPage {}
}

glib::wrapper! {
    pub struct PreviewPage(ObjectSubclass<imp::PreviewPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

pub struct Parts {
    header: adw::HeaderBar,
    more: gtk::MenuButton,
    hero: ShowHero,
    episodes: gtk::ListBox,
    heading: gtk::Label,
    stack: gtk::Stack,
    failed: adw::StatusPage,
    view_in_store: gio::SimpleAction,
}

impl Default for PreviewPage {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl PreviewPage {
    fn build(&self) {
        self.set_tag(Some(PREVIEW_TAG));
        self.set_title(&gettext("Podcast"));
        let hero = ShowHero::new();
        hero.set_subscribed(false);
        let this = self.downgrade();
        hero.subscribe.connect_clicked(move |_| {
            if let Some(page) = this.upgrade() {
                page.subscribe();
            }
        });

        let heading = gtk::Label::builder()
            .label(gettext("Recent Episodes"))
            .xalign(0.0)
            .css_classes(["heading"])
            .accessible_role(gtk::AccessibleRole::Heading)
            .margin_top(12)
            .margin_bottom(6)
            .build();
        let episodes = gtk::ListBox::builder()
            .css_classes(["boxed-list"])
            .selection_mode(gtk::SelectionMode::None)
            .build();
        episodes.update_property(&[gtk::accessible::Property::Label(&gettext(
            "Recent Episodes",
        ))]);
        let loading = adw::Spinner::builder()
            .height_request(32)
            .margin_top(24)
            .build();
        loading
            .upcast_ref::<gtk::Widget>()
            .update_property(&[gtk::accessible::Property::Label(&gettext(
                "Loading episodes",
            ))]);
        let failed = adw::StatusPage::builder()
            .icon_name("network-error-symbolic")
            .title(gettext("Could Not Load Episodes"))
            .css_classes(["compact"])
            .build();
        let list = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        list.append(&heading);
        list.append(&episodes);
        let stack = gtk::Stack::new();
        stack.add_named(&loading, Some("loading"));
        stack.add_named(&list, Some("episodes"));
        stack.add_named(&failed, Some("failed"));

        let column = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_start(12)
            .margin_end(12)
            .margin_bottom(24)
            .build();
        column.append(hero.widget());
        column.append(&stack);
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(
                &adw::Clamp::builder()
                    .maximum_size(900)
                    .child(&column)
                    .build(),
            )
            .build();

        let actions = gio::SimpleActionGroup::new();
        let view_in_store = gio::SimpleAction::new("view-in-store", None);
        let this = self.downgrade();
        view_in_store.connect_activate(move |_, _| {
            if let Some(page) = this.upgrade() {
                page.view_in_store();
            }
        });
        actions.add_action(&view_in_store);
        let copy = gio::SimpleAction::new("copy-feed", None);
        let this = self.downgrade();
        copy.connect_activate(move |_, _| {
            if let Some(page) = this.upgrade() {
                page.copy_feed();
            }
        });
        actions.add_action(&copy);
        self.insert_action_group("preview", Some(&actions));
        let menu = gio::Menu::new();
        menu.append(
            Some(&gettext("_View on Apple Podcasts")),
            Some("preview.view-in-store"),
        );
        menu.append(
            Some(&gettext("_Copy Feed Address")),
            Some("preview.copy-feed"),
        );
        let more = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text(gettext("More Options"))
            .menu_model(&menu)
            .build();
        more.update_property(&[gtk::accessible::Property::Label(&gettext("More Options"))]);

        let header = adw::HeaderBar::new();
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&scrolled));
        self.set_child(Some(&toolbar));
        let _ = self.imp().parts.set(Parts {
            header,
            more,
            hero,
            episodes,
            heading,
            stack,
            failed,
            view_in_store,
        });
    }

    pub fn connect_output(&self, receive: impl Fn(PreviewPageOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    /// Put `details_toggle` at the header's far end, as on every page,
    /// then More Options.
    pub fn pack_end_buttons(&self, details_toggle: &gtk::ToggleButton) {
        let parts = self.parts();
        parts.header.pack_end(details_toggle);
        parts.header.pack_end(&parts.more);
    }

    fn parts(&self) -> &Parts {
        self.imp().parts.get().expect("built when constructed")
    }

    /// The feed shown, if any.
    pub fn feed(&self) -> Option<Url> {
        self.imp().feed.borrow().clone()
    }

    pub fn send(&self, input: PreviewPageInput) {
        let imp = self.imp();
        let parts = self.parts();
        match input {
            PreviewPageInput::Start { hero, feed, store } => {
                self.set_title(&hero.title);
                parts.hero.update(&hero);
                parts.episodes.remove_all();
                parts.stack.set_visible_child_name("loading");
                parts.view_in_store.set_enabled(store.is_some());
                imp.feed.replace(Some(feed));
                imp.store.replace(store);
                imp.loaded.set(false);
                imp.subscribing.set(false);
            }
            PreviewPageInput::Loaded(data) => {
                self.set_title(&data.hero.title);
                parts.hero.update(&data.hero);
                parts.episodes.remove_all();
                for (title, subtitle) in &data.episodes {
                    parts.episodes.append(
                        &adw::ActionRow::builder()
                            .title(title)
                            .subtitle(subtitle)
                            .use_markup(false)
                            .title_lines(2)
                            .build(),
                    );
                }
                let none = data.episodes.is_empty();
                parts.heading.set_label(&if none {
                    gettext("No Episodes Yet")
                } else {
                    gettext("Recent Episodes")
                });
                parts.episodes.set_visible(!none);
                parts.stack.set_visible_child_name("episodes");
                imp.loaded.set(true);
            }
            PreviewPageInput::Failed(message) => {
                parts.failed.set_description(Some(&message));
                parts.stack.set_visible_child_name("failed");
            }
            PreviewPageInput::Done => imp.subscribing.set(false),
        }
        let busy = imp.subscribing.get();
        parts
            .hero
            .subscribe
            .set_sensitive(imp.loaded.get() && !busy);
        parts.hero.subscribe.set_label(&if busy {
            gettext("Subscribing…")
        } else {
            gettext("_Subscribe")
        });
    }

    fn subscribe(&self) {
        let imp = self.imp();
        let Some(feed) = self.feed() else {
            return;
        };
        if imp.subscribing.replace(true) {
            return;
        }
        self.parts().hero.subscribe.set_sensitive(false);
        self.parts()
            .hero
            .subscribe
            .set_label(&gettext("Subscribing…"));
        imp.output.emit(PreviewPageOutput::Subscribe(feed));
    }

    fn view_in_store(&self) {
        let Some(url) = self.imp().store.borrow().clone() else {
            return;
        };
        let window = self.root().and_downcast::<gtk::Window>();
        gtk::UriLauncher::new(url.as_str()).launch(
            window.as_ref(),
            gio::Cancellable::NONE,
            |result| {
                if let Err(err) = result {
                    tracing::warn!("could not open Apple Podcasts: {err}");
                }
            },
        );
    }

    fn copy_feed(&self) {
        if let (Some(url), Some(display)) = (self.feed(), gtk::gdk::Display::default()) {
            display.clipboard().set_text(url.as_str());
            self.imp().output.emit(PreviewPageOutput::FeedAddressCopied);
        }
    }

    /// Whether Subscribe can be pressed, for tests.
    #[cfg(test)]
    pub fn can_subscribe(&self) -> bool {
        self.parts().hero.subscribe.is_sensitive()
    }

    /// Press Subscribe, for tests.
    #[cfg(test)]
    pub fn press_subscribe(&self) {
        self.parts().hero.subscribe.emit_clicked();
    }
}
