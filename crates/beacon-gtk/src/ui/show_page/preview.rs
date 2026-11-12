// SPDX-License-Identifier: GPL-3.0-or-later

//! What only a preview does: say while its feed loads or why it could not,
//! Subscribe, and open the show's page on Apple Podcasts.

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::gio;

use super::{PageKind, ShowPage, ShowPageOutput};

/// Whether a preview's feed has loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewStatus {
    Loading,
    Loaded,
    /// It could not be loaded: why.
    Failed(String),
}

impl ShowPage {
    /// What a preview's list says in place of episodes while its feed is
    /// not loaded.
    pub(super) fn preview_message(&self) -> Option<String> {
        match self.imp().preview.borrow().as_ref()?.1 {
            PreviewStatus::Loading => Some(gettext("Loading episodes…")),
            PreviewStatus::Failed(ref why) => Some(why.clone()),
            PreviewStatus::Loaded => None,
        }
    }

    /// Subscribe, once the feed has loaded and while it is not already
    /// subscribing.
    pub(super) fn show_subscribe(&self) {
        let imp = self.imp();
        let loaded = matches!(
            imp.preview.borrow().as_ref(),
            Some((_, PreviewStatus::Loaded))
        );
        let busy = imp.subscribing.get();
        let subscribe = &self.parts().hero.subscribe;
        subscribe.set_sensitive(loaded && !busy);
        subscribe.set_label(&if busy {
            gettext("Subscribing…")
        } else {
            gettext("_Subscribe")
        });
    }

    pub(super) fn subscribe(&self) {
        let imp = self.imp();
        if self.parts().kind != PageKind::Preview || imp.subscribing.get() {
            return;
        }
        let Some(feed) = self.feed() else {
            return;
        };
        imp.subscribing.set(true);
        self.show_subscribe();
        imp.output.emit(ShowPageOutput::Subscribe(feed));
    }

    pub(super) fn view_in_store(&self) {
        let store = self
            .imp()
            .preview
            .borrow()
            .as_ref()
            .and_then(|(s, _)| s.clone());
        let Some(url) = store else {
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
