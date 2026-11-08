// SPDX-License-Identifier: GPL-3.0-or-later

//! A page's selection mode, laid out after the HIG's Selection & Edit
//! Modes page: "The header bar should show a cancel button and the heading
//! should indicate the number of selected items." The page asks for the
//! actions in an action bar at the bottom; Beacon puts them in the header
//! instead, at its start (asked by the user, 2026-11-07): as icons, or one
//! menu where the page is too narrow for them. The heading is a menu with
//! Select All and Select None, the classic GNOME selection title. A Select
//! button in the page's own header enters selection mode; so do selecting
//! a row and Ctrl+A. Escape and Cancel leave it.

use std::rc::Rc;

use beacon_core::model::{EpisodeChange, EpisodeId};
use gettextrs::{gettext, ngettext};
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk::{self, gio, glib};

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

/// Below this page width the actions fold into one menu: four icons,
/// Cancel, the count and the window's close button do not fit in 360px.
const ICONS_MIN_WIDTH_SP: f64 = 520.0;

/// The icon, label and menu label (with its access key, as the row menu
/// has them) of the three toggles, for what choosing each does to the
/// selection as it is.
fn toggle_labels(selection: &Selection) -> [(&'static str, String, String); 3] {
    let items = selection.selected();
    let [q, f, p] = EpisodeChange::offered(items.iter().map(|i| (i.in_queue, &i.episode.state)));
    [
        match q {
            EpisodeChange::Queued(false) => (
                "queue-added-symbolic",
                gettext("Remove From Queue"),
                gettext("Remove From _Queue"),
            ),
            _ => (
                "queue-add-symbolic",
                gettext("Add to Queue"),
                gettext("Add to _Queue"),
            ),
        },
        match f {
            EpisodeChange::Favorite(false) => (
                "starred-symbolic",
                gettext("Remove From Favorites"),
                gettext("Remove From _Favorites"),
            ),
            _ => (
                "non-starred-symbolic",
                gettext("Add to Favorites"),
                gettext("Add to _Favorites"),
            ),
        },
        match p {
            EpisodeChange::Played(false) => (
                "mark-unplayed-symbolic",
                gettext("Mark as Unplayed"),
                gettext("Mark as _Unplayed"),
            ),
            _ => (
                "mark-played-symbolic",
                gettext("Mark as Played"),
                gettext("Mark as _Played"),
            ),
        },
    ]
}

/// The selection's actions: the three toggles and Download, as `selection.*`.
const ACTIONS: [&str; 4] = ["queue", "favorite", "played", "download"];

/// Add selection mode to the page whose content is `toolbar` and whose
/// header is `header`, holding `select` (from `select_button`): the
/// selection header and the shortcuts, which work while `select` shows.
pub fn attach(
    toolbar: &adw::ToolbarView,
    header: &adw::HeaderBar,
    select: &gtk::Button,
    selection: &Selection,
    on_change: OnChange,
) {
    let group = gio::SimpleActionGroup::new();
    let on_change = Rc::new(on_change);
    for (index, name) in ACTIONS.into_iter().enumerate() {
        let action = gio::SimpleAction::new(name, None);
        let (s, on_change) = (selection.clone(), on_change.clone());
        action.connect_activate(move |_, _| {
            let items = s.selected();
            let offered =
                EpisodeChange::offered(items.iter().map(|i| (i.in_queue, &i.episode.state)));
            if let Some(change) = offered.get(index) {
                on_change(items.iter().map(|i| i.episode.id).collect(), *change);
            }
        });
        group.add_action(&action);
    }
    for (name, all) in [("all", true), ("none", false)] {
        let action = gio::SimpleAction::new(name, None);
        let s = selection.clone();
        action.connect_activate(move |_, _| {
            if all {
                s.select_all();
            } else {
                s.select_none();
            }
        });
        group.add_action(&action);
    }

    // The heading: the count, opening Select All and Select None.
    let selecting = gio::Menu::new();
    let item = gio::MenuItem::new(Some(&gettext("Select _All")), Some("selection.all"));
    item.set_attribute_value("accel", Some(&"<Control>a".to_variant()));
    selecting.append_item(&item);
    let item = gio::MenuItem::new(Some(&gettext("Select _None")), Some("selection.none"));
    item.set_attribute_value("accel", Some(&"<Shift><Control>a".to_variant()));
    selecting.append_item(&item);
    let title = gtk::MenuButton::builder()
        .always_show_arrow(true)
        .menu_model(&selecting)
        .css_classes(["flat"])
        .build();

    let cancel = gtk::Button::builder()
        .label(gettext("_Cancel"))
        .use_underline(true)
        .build();
    let s = selection.clone();
    cancel.connect_clicked(move |_| s.set_active(false));

    // The actions, as icons, or folded into one menu on a narrow page.
    let buttons: Vec<gtk::Button> = ACTIONS
        .into_iter()
        .map(|name| {
            let button = icon_button("", "");
            button.set_action_name(Some(&format!("selection.{name}")));
            button
        })
        .collect();
    set_label(&buttons[3], "download-symbolic", &gettext("Download"));
    let icons = gtk::Box::builder().spacing(6).build();
    // In the order of the details pane and the row menu.
    for index in [0, 1, 3, 2] {
        icons.append(&buttons[index]);
    }
    let folded = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(gettext("Selection Actions"))
        .visible(false)
        .build();
    folded.update_property(&[gtk::accessible::Property::Label(&gettext(
        "Selection Actions",
    ))]);

    let selection_header = adw::HeaderBar::builder()
        .title_widget(&title)
        .show_back_button(false)
        .visible(false)
        .build();
    selection_header.pack_start(&icons);
    selection_header.pack_start(&folded);
    selection_header.pack_end(&cancel);
    selection_header.insert_action_group("selection", Some(&group));
    toolbar.add_top_bar(&selection_header);
    fold_when_narrow(toolbar, &icons, &folded);

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
        let count = s.count();
        title.set_label(&if count == 0 {
            gettext("No Episodes Selected")
        } else {
            ngettext("{n} Selected", "{n} Selected", count as u32)
                .replace("{n}", &count.to_string())
        });
        // Downloads arrive in M5.
        for name in ACTIONS {
            if let Some(action) = group
                .lookup_action(name)
                .and_downcast::<gio::SimpleAction>()
            {
                action.set_enabled(count > 0 && name != "download");
            }
        }
        let labels = toggle_labels(s);
        let menu = gio::Menu::new();
        for (index, (icon, label, item)) in labels.iter().enumerate() {
            set_label(&buttons[index], icon, label);
            menu.append(Some(item), Some(&format!("selection.{}", ACTIONS[index])));
        }
        menu.insert(2, Some(&gettext("_Download")), Some("selection.download"));
        folded.set_menu_model(Some(&menu));
        if refocus {
            to.grab_focus();
        }
    });
    // Ctrl+A selects everything, Shift+Ctrl+A nothing (the HIG's
    // standard shortcuts: "Selects all content in the focused control or
    // view"), Escape leaves selection mode; all only while the Select
    // button shows. Caught before the focused widget sees them, since a
    // list box takes Ctrl+A for itself, but text fields keep theirs.
    let shortcuts = gtk::ShortcutController::new();
    shortcuts.set_propagation_phase(gtk::PropagationPhase::Capture);
    for (trigger, action) in [("<Control>a", 0u8), ("<Shift><Control>a", 1), ("Escape", 2)] {
        let s = selection.clone();
        let select = select.clone();
        let callback = gtk::CallbackAction::new(move |widget, _| {
            let typing = widget
                .root()
                .and_then(|root| root.focus())
                .is_some_and(|focus| focus.is::<gtk::Text>());
            if typing || !takes_shortcut(&select, &s, action) {
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

/// Whether the page's selection shortcut `action` (0 Ctrl+A, 1 Shift+Ctrl+A,
/// 2 Escape) applies: Ctrl+A where the page offers selection (its Select
/// button is set to show, though hidden with its header in selection mode)
/// or selection mode is on; the others only in selection mode.
pub(crate) fn takes_shortcut(select: &gtk::Button, selection: &Selection, action: u8) -> bool {
    // `get_visible`, not `is_visible`, which also asks the parents: in
    // selection mode the header Select is in is hidden, and Ctrl+A did
    // nothing there.
    match action {
        0 => select.get_visible() || selection.is_active(),
        _ => selection.is_active(),
    }
}

/// Fold the actions into the menu while the page is narrower than
/// `ICONS_MIN_WIDTH_SP`: the page's content goes into a breakpoint bin,
/// which measures the page rather than the window, so a page squeezed by
/// the details pane folds them too.
fn fold_when_narrow(toolbar: &adw::ToolbarView, icons: &gtk::Box, folded: &gtk::MenuButton) {
    let Some(page) = toolbar.parent().and_downcast::<adw::NavigationPage>() else {
        return;
    };
    // A breakpoint bin needs a minimum size; the narrowest window's page.
    let bin = adw::BreakpointBin::builder()
        .width_request(300)
        .height_request(200)
        .build();
    page.set_child(None::<&gtk::Widget>);
    bin.set_child(Some(toolbar));
    page.set_child(Some(&bin));
    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        ICONS_MIN_WIDTH_SP,
        adw::LengthUnit::Sp,
    ));
    breakpoint.add_setter(icons, "visible", Some(&false.to_value()));
    breakpoint.add_setter(folded, "visible", Some(&true.to_value()));
    bin.add_breakpoint(breakpoint);
}
