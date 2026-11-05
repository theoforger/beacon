// SPDX-License-Identifier: GPL-3.0-or-later

//! A page's selection mode, as the HIG's Selection & Edit Modes page lays
//! it out: "The header bar should show a cancel button and the heading
//! should indicate the number of selected items", and "An action bar
//! should show available actions for the selection, at the bottom of the
//! screen". A Select button in the page's own header enters it; so do
//! selecting a row and Ctrl+A. Escape and Cancel leave it.

use beacon_core::model::{EpisodeChange, EpisodeId};
use gettextrs::{gettext, ngettext};
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk::{self, glib};

use crate::ui::selection::Selection;

/// Called with the selected episodes, in the page's order, and the change
/// chosen for them.
pub type OnChange = Box<dyn Fn(Vec<EpisodeId>, EpisodeChange)>;

/// An icon-only button with its tooltip and accessible label.
fn icon_button(icon: &str, label: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(label)
        .build();
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button
}

fn set_label(button: &gtk::Button, icon: &str, label: &str) {
    button.set_icon_name(icon);
    button.set_tooltip_text(Some(label));
    button.update_property(&[gtk::accessible::Property::Label(label)]);
}

/// The Select button, which enters selection mode, for a page's header.
pub fn select_button(selection: &Selection) -> gtk::Button {
    let select = icon_button("selection-mode-symbolic", &gettext("Select"));
    let s = selection.clone();
    select.connect_clicked(move |_| s.set_active(true));
    select
}

/// Add selection mode to the page whose content is `toolbar` and whose
/// header is `header`, holding `select` (from `select_button`): the
/// selection header, the action bar and the shortcuts, which work while
/// `select` shows.
pub fn attach(
    toolbar: &adw::ToolbarView,
    header: &adw::HeaderBar,
    select: &gtk::Button,
    selection: &Selection,
    on_change: OnChange,
) {
    {
        let title = adw::WindowTitle::new("", "");
        let all = icon_button("edit-select-all-symbolic", &gettext("Select All"));
        let s = selection.clone();
        all.connect_clicked(move |_| {
            if s.all_selected() {
                s.select_none();
            } else {
                s.select_all();
            }
        });
        let cancel = gtk::Button::builder()
            .label(gettext("_Cancel"))
            .use_underline(true)
            .build();
        let s = selection.clone();
        cancel.connect_clicked(move |_| s.set_active(false));
        let selection_header = adw::HeaderBar::builder()
            .title_widget(&title)
            .show_back_button(false)
            .visible(false)
            .build();
        selection_header.pack_start(&all);
        selection_header.pack_end(&cancel);
        toolbar.add_top_bar(&selection_header);

        // The toggles say what choosing them does to the whole selection,
        // with the details pane's icons.
        let queue = icon_button("queue-add-symbolic", &gettext("Add to Queue"));
        let favorite = icon_button("non-starred-symbolic", &gettext("Add to Favorites"));
        // Downloads arrive in M5.
        let download = icon_button("download-symbolic", &gettext("Download"));
        download.set_sensitive(false);
        let played = icon_button("mark-played-symbolic", &gettext("Mark as Played"));
        let actions = gtk::Box::builder().spacing(6).build();
        for button in [&queue, &favorite, &download, &played] {
            actions.append(button);
        }
        let action_bar = gtk::ActionBar::builder().revealed(false).build();
        action_bar.set_center_widget(Some(&actions));
        toolbar.add_bottom_bar(&action_bar);
        // Set apart from the list, as the header is.
        toolbar.set_bottom_bar_style(adw::ToolbarStyle::Raised);

        let on_change = std::rc::Rc::new(on_change);
        for (index, button) in [&queue, &favorite, &played].into_iter().enumerate() {
            let s = selection.clone();
            let on_change = on_change.clone();
            button.connect_clicked(move |_| {
                let items = s.selected();
                let offered =
                    EpisodeChange::offered(items.iter().map(|i| (i.in_queue, &i.episode.state)));
                on_change(items.iter().map(|i| i.episode.id).collect(), offered[index]);
            });
        }

        let header = header.clone();
        let select_button = select.clone();
        selection.connect_changed(move |s| {
            let active = s.is_active();
            // Focus would be lost with the header it is in: move it to the
            // header that replaces it.
            let (from, to): (&adw::HeaderBar, &gtk::Button) = if active {
                (&header, &cancel)
            } else {
                (&selection_header, &select_button)
            };
            let focus = from.root().and_then(|root| root.focus());
            let refocus = from.is_visible() && focus.is_some_and(|f| f.is_ancestor(from));
            header.set_visible(!active);
            selection_header.set_visible(active);
            action_bar.set_revealed(active);
            let count = s.count();
            title.set_title(&if count == 0 {
                gettext("No Episodes Selected")
            } else {
                ngettext("{n} Selected", "{n} Selected", count as u32)
                    .replace("{n}", &count.to_string())
            });
            set_label(
                &all,
                "edit-select-all-symbolic",
                &if s.all_selected() {
                    gettext("Select None")
                } else {
                    gettext("Select All")
                },
            );
            let items = s.selected();
            for button in [&queue, &favorite, &played] {
                button.set_sensitive(count > 0);
            }
            let [q, f, p] =
                EpisodeChange::offered(items.iter().map(|i| (i.in_queue, &i.episode.state)));
            let (icon, label) = match q {
                EpisodeChange::Queued(false) => {
                    ("queue-added-symbolic", gettext("Remove From Queue"))
                }
                _ => ("queue-add-symbolic", gettext("Add to Queue")),
            };
            set_label(&queue, icon, &label);
            let (icon, label) = match f {
                EpisodeChange::Favorite(false) => {
                    ("starred-symbolic", gettext("Remove From Favorites"))
                }
                _ => ("non-starred-symbolic", gettext("Add to Favorites")),
            };
            set_label(&favorite, icon, &label);
            let (icon, label) = match p {
                EpisodeChange::Played(false) => {
                    ("mark-unplayed-symbolic", gettext("Mark as Unplayed"))
                }
                _ => ("mark-played-symbolic", gettext("Mark as Played")),
            };
            set_label(&played, icon, &label);
            if refocus {
                to.grab_focus();
            }
        });

        // Ctrl+A selects everything, Shift+Ctrl+A nothing (the HIG's
        // standard shortcuts), Escape leaves selection mode; all only while
        // the Select button shows.
        let shortcuts = gtk::ShortcutController::new();
        for (trigger, action) in [("<Control>a", 0), ("<Shift><Control>a", 1), ("Escape", 2)] {
            let s = selection.clone();
            let select = select.clone();
            let callback = gtk::CallbackAction::new(move |_, _| {
                if !select.is_visible() || (action != 0 && !s.is_active()) {
                    return glib::Propagation::Proceed;
                }
                match action {
                    0 => s.select_all(),
                    1 => s.select_none(),
                    _ => s.set_active(false),
                }
                glib::Propagation::Stop
            });
            shortcuts.add_shortcut(gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string(trigger),
                Some(callback),
            ));
        }
        toolbar.add_controller(shortcuts);
    }
}
