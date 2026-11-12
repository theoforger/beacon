// SPDX-License-Identifier: GPL-3.0-or-later

//! A podcast page's header title, shown only once the hero's large title
//! has scrolled out of view (decided by the user 2026-11-11). Beside Back,
//! Search, Filter, More Options and three window buttons, a 360px header
//! cut the show's name to a few letters, just above the same name in large
//! type. GNOME Podcasts always shows it, but its page has no title of its
//! own under the header (`show_widget.ui`); Beacon's hero has, as the
//! mockup asks. The page keeps its title for Back and navigation, and the
//! hero's title is a heading for screen readers.

use std::cell::Cell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

/// Keep `header`'s title hidden while `title`, scrolled in `scrolled`, is
/// in view. Returns what checks again, for when the page's content changes.
pub fn follow_scroll(
    header: &adw::HeaderBar,
    scrolled: &gtk::ScrolledWindow,
    title: &gtk::Label,
) -> Rc<dyn Fn()> {
    // Where the title's bottom is in the list, measured while it is truly
    // in view: once scrolled away, the list keeps its widget for reuse and
    // its reported place means nothing.
    let bottom: Rc<Cell<Option<f64>>> = Rc::default();
    let check: Rc<dyn Fn()> = Rc::new({
        let (header, scrolled, title) =
            (header.downgrade(), scrolled.downgrade(), title.downgrade());
        move || {
            let (Some(header), Some(scrolled), Some(title)) =
                (header.upgrade(), scrolled.upgrade(), title.upgrade())
            else {
                return;
            };
            let value = scrolled.vadjustment().value();
            let in_view = value == 0.0 || bottom.get().is_some_and(|b| value < b);
            if in_view && let Some(measured) = measure(&scrolled, &title, value) {
                bottom.set(Some(measured));
            }
            let past = bottom.get().is_some_and(|b| value >= b);
            if header.shows_title() != past {
                header.set_show_title(past);
            }
        }
    });
    let adjustment = scrolled.vadjustment();
    adjustment.connect_value_changed({
        let check = check.clone();
        move |_| check()
    });
    adjustment.connect_changed({
        let check = check.clone();
        move |_| check()
    });
    check
}

/// Check once the page has been laid out.
pub fn check_later(check: &Rc<dyn Fn()>) {
    let check = check.clone();
    glib::idle_add_local_once(move || check());
}

/// Where `title`'s bottom is in the list scrolled to `value`, if it is laid
/// out.
fn measure(scrolled: &gtk::ScrolledWindow, title: &gtk::Label, value: f64) -> Option<f64> {
    if !title.is_mapped() {
        return None;
    }
    let bounds = title.compute_bounds(scrolled)?;
    Some(value + f64::from(bounds.y() + bounds.height()))
}
