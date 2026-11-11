// SPDX-License-Identifier: GPL-3.0-or-later

//! Search on the root page: while the search bar is open with something
//! typed, the section gives way to the results and the page is titled
//! Search Results, as in the mockup; closing the bar or emptying it brings
//! the section back.

use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;

use super::build::ContentWidgets;

/// What the field holds while the bar is open, trimmed; `None` when there
/// is nothing to search for.
pub(crate) fn query(bar: &gtk::SearchBar, entry: &gtk::SearchEntry) -> Option<String> {
    let text = entry.text();
    let text = text.trim();
    (bar.is_search_mode() && !text.is_empty()).then(|| text.to_owned())
}

/// Call `changed` with the query whenever it changes, or the bar opens or
/// closes.
pub(crate) fn connect_query(
    bar: &gtk::SearchBar,
    entry: &gtk::SearchEntry,
    changed: impl Fn(Option<String>) + 'static,
) {
    let changed = Rc::new(changed);
    let last: Rc<std::cell::RefCell<Option<String>>> = Rc::default();
    let report = {
        let (bar, entry) = (bar.downgrade(), entry.downgrade());
        move || {
            let (Some(bar), Some(entry)) = (bar.upgrade(), entry.upgrade()) else {
                return;
            };
            let query = query(&bar, &entry);
            if *last.borrow() != query {
                last.replace(query.clone());
                changed(query);
            }
        }
    };
    let report = Rc::new(report);
    entry.connect_search_changed({
        let report = report.clone();
        move |_| report()
    });
    bar.connect_search_mode_enabled_notify(move |_| report());
}

/// The root page's title and stack for `query`: the results, or the
/// section `section_id` titled `section_title`.
pub(super) fn show_results(
    widgets: &ContentWidgets,
    searching: bool,
    section_id: &str,
    section_title: &str,
) {
    if searching {
        widgets.stack.set_visible_child_name(RESULTS);
        widgets.root_page.set_title(&gettext("Search Results"));
    } else {
        widgets.stack.set_visible_child_name(section_id);
        widgets.root_page.set_title(section_title);
    }
}

/// The root stack's page of results.
pub(super) const RESULTS: &str = "search";
