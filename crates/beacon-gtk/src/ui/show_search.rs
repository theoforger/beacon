// SPDX-License-Identifier: GPL-3.0-or-later

//! Search on a podcast page: the header's Search toggle and Ctrl+F open a
//! search bar under the header, with the mockup's This Show / Everywhere
//! toggle beside the field. This Show narrows the page's own episodes in
//! place; Everywhere puts the library's results where the episodes were.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;

use crate::ui::content::search;
use crate::ui::search_view::SearchView;

const THIS_SHOW: &str = "show";
const EVERYWHERE: &str = "everywhere";
const EPISODES: &str = "episodes";
const RESULTS: &str = "results";

/// What the page is searching for, and where.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShowQuery {
    /// Narrowing the page's own episodes.
    pub this_show: Option<String>,
    /// Searching the whole library.
    pub everywhere: Option<String>,
}

pub struct ShowSearch {
    pub toggle: gtk::ToggleButton,
    pub bar: gtk::SearchBar,
    entry: gtk::SearchEntry,
    scope: adw::ToggleGroup,
    pub view: SearchView,
    /// The page's episodes, or the Everywhere results.
    pub stack: gtk::Stack,
}

impl ShowSearch {
    /// Around `episodes`, the page's list; `changed` hears every change of
    /// the query or its scope.
    pub fn new(episodes: &impl IsA<gtk::Widget>, changed: impl Fn(ShowQuery) + 'static) -> Self {
        let toggle = gtk::ToggleButton::builder()
            .icon_name("system-search-symbolic")
            .tooltip_text(gettext("Search"))
            .build();
        toggle.update_property(&[gtk::accessible::Property::Label(&gettext("Search"))]);

        let entry = gtk::SearchEntry::builder()
            .hexpand(true)
            .placeholder_text(gettext("Search This Show"))
            .build();
        let scope = adw::ToggleGroup::builder()
            .valign(gtk::Align::Center)
            .build();
        scope.add(
            adw::Toggle::builder()
                .name(THIS_SHOW)
                .label(gettext("This Show"))
                .build(),
        );
        scope.add(
            adw::Toggle::builder()
                .name(EVERYWHERE)
                .label(gettext("Everywhere"))
                .build(),
        );
        scope.set_active_name(Some(THIS_SHOW));
        scope
            .upcast_ref::<gtk::Widget>()
            .update_property(&[gtk::accessible::Property::Label(&gettext("Search Scope"))]);
        // Beside the field when both fit, under it when not, so the scope's
        // labels are never cut short (found at 360px in the M7 review).
        let row = adw::WrapBox::builder()
            .child_spacing(6)
            .line_spacing(6)
            .justify(adw::JustifyMode::Fill)
            .build();
        entry.set_width_chars(16);
        row.append(&entry);
        row.append(&scope);
        let bar = gtk::SearchBar::builder()
            .child(&adw::Clamp::builder().maximum_size(500).child(&row).build())
            .build();
        bar.connect_entry(&entry);
        toggle
            .bind_property("active", &bar, "search-mode-enabled")
            .bidirectional()
            .sync_create()
            .build();

        let view = SearchView::default();
        let stack = gtk::Stack::builder().vexpand(true).build();
        stack.add_named(episodes, Some(EPISODES));
        stack.add_named(&view, Some(RESULTS));

        let search = Self {
            toggle,
            bar,
            entry,
            scope,
            view,
            stack,
        };
        search.connect(changed);
        search
    }

    fn connect(&self, changed: impl Fn(ShowQuery) + 'static) {
        let last: Rc<RefCell<ShowQuery>> = Rc::default();
        let report = {
            let (bar, entry) = (self.bar.downgrade(), self.entry.downgrade());
            let (scope, stack) = (self.scope.downgrade(), self.stack.downgrade());
            Rc::new(move || {
                let (Some(bar), Some(entry), Some(scope), Some(stack)) = (
                    bar.upgrade(),
                    entry.upgrade(),
                    scope.upgrade(),
                    stack.upgrade(),
                ) else {
                    return;
                };
                let everywhere = scope.active_name().as_deref() == Some(EVERYWHERE);
                entry.set_placeholder_text(Some(&if everywhere {
                    gettext("Search Shows and Episodes")
                } else {
                    gettext("Search This Show")
                }));
                let text = search::query(&bar, &entry);
                let query = if everywhere {
                    ShowQuery {
                        this_show: None,
                        everywhere: text,
                    }
                } else {
                    ShowQuery {
                        this_show: text,
                        everywhere: None,
                    }
                };
                stack.set_visible_child_name(if query.everywhere.is_some() {
                    RESULTS
                } else {
                    EPISODES
                });
                if *last.borrow() != query {
                    last.replace(query.clone());
                    changed(query);
                }
            })
        };
        search::connect_query(&self.bar, &self.entry, {
            let report = report.clone();
            move |_| report()
        });
        self.scope.connect_active_name_notify(move |_| report());
    }

    /// Close the bar, ending any search.
    pub fn close(&self) {
        self.bar.set_search_mode(false);
    }

    /// Ctrl+F.
    pub fn toggle(&self) {
        self.toggle.set_active(!self.toggle.is_active());
    }

    /// Choose Everywhere or This Show, for tests.
    #[cfg(test)]
    pub fn set_everywhere(&self, everywhere: bool) {
        self.scope
            .set_active_name(Some(if everywhere { EVERYWHERE } else { THIS_SHOW }));
    }

    /// Type into the field, for tests.
    #[cfg(test)]
    pub fn type_text(&self, text: &str) {
        self.bar.set_search_mode(true);
        self.entry.set_text(text);
    }

    /// Whether the Everywhere results show, for tests.
    #[cfg(test)]
    pub fn showing_results(&self) -> bool {
        self.stack.visible_child_name().as_deref() == Some(RESULTS)
    }
}
