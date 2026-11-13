// SPDX-License-Identifier: GPL-3.0-or-later

//! The podcasts an import or export would take, to choose from
//! (`docs/specs/opml.md`): each with a check box, its cover, its title and
//! its feed address, in a boxed list, with Select All or Select None in the
//! list's header. The HIG's Boxed Lists page: "When there is a control,
//! clicking the list background should trigger the control", so a row
//! toggles its check box.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::glib;

use crate::ui::covers::Cover;
use crate::ui::messages::Output;

/// GNOME Podcasts' small list cover.
const COVER_SIZE: i32 = 48;

/// A podcast to choose.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PickItem {
    pub title: String,
    /// The feed address, as the row's subtitle.
    pub url: String,
    pub cover: Option<PathBuf>,
    /// Why it may not go through (the feed failed to load, say): shown on
    /// the row, which starts unchecked.
    pub problem: Option<String>,
}

#[derive(Debug)]
pub enum FeedPickerInput {
    /// Show `items`, all checked but those with a problem, under the
    /// list's `note`, if any.
    Set {
        items: Vec<PickItem>,
        note: Option<String>,
    },
    /// What is known of an item now. One that gains a problem is
    /// unchecked; otherwise its check box is left as it is.
    Update(usize, PickItem),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedPickerOutput {
    /// Which items are checked now, by their place in the list.
    Selected(Vec<usize>),
}

mod imp {
    use std::cell::OnceCell;

    use super::*;

    #[derive(Default)]
    pub struct FeedPicker {
        pub group: OnceCell<adw::PreferencesGroup>,
        pub list: OnceCell<gtk::ListBox>,
        pub toggle_all: OnceCell<gtk::Button>,
        pub rows: RefCell<Vec<super::Row>>,
        /// Set while the picker changes check boxes itself.
        pub updating: Cell<bool>,
        pub output: Output<FeedPickerOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FeedPicker {
        const NAME: &'static str = "BeaconFeedPicker";
        type Type = super::FeedPicker;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for FeedPicker {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }
    impl WidgetImpl for FeedPicker {}
    impl BinImpl for FeedPicker {}
}

glib::wrapper! {
    pub struct FeedPicker(ObjectSubclass<imp::FeedPicker>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for FeedPicker {
    fn default() -> Self {
        glib::Object::new()
    }
}

/// A row's widgets.
pub struct Row {
    row: adw::ActionRow,
    check: gtk::CheckButton,
    cover: Cover,
    warning: gtk::Image,
}

impl FeedPicker {
    fn build(&self) {
        let toggle_all = gtk::Button::builder()
            .css_classes(["flat"])
            .valign(gtk::Align::Center)
            .use_underline(true)
            .build();
        let list = gtk::ListBox::builder()
            .css_classes(["boxed-list"])
            .selection_mode(gtk::SelectionMode::None)
            .build();
        // Translators: how a screen reader names the list of podcasts to
        // import or export.
        list.update_property(&[gtk::accessible::Property::Label(&gettext("Podcasts"))]);
        let group = adw::PreferencesGroup::builder()
            .header_suffix(&toggle_all)
            .build();
        group.add(&list);
        let page = adw::PreferencesPage::new();
        page.add(&group);
        self.set_child(Some(&page));

        let this = self.downgrade();
        toggle_all.connect_clicked(move |_| {
            if let Some(this) = this.upgrade() {
                let all = this.selected().len() < this.imp().rows.borrow().len();
                this.check_all(all);
            }
        });
        let imp = self.imp();
        let _ = imp.group.set(group);
        let _ = imp.list.set(list);
        let _ = imp.toggle_all.set(toggle_all);
        self.update_toggle_all();
    }

    pub fn connect_output(&self, receive: impl Fn(FeedPickerOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    pub fn send(&self, input: FeedPickerInput) {
        match input {
            FeedPickerInput::Set { items, note } => self.set_items(&items, note.as_deref()),
            FeedPickerInput::Update(i, item) => {
                let rows = self.imp().rows.borrow();
                let Some(row) = rows.get(i) else {
                    return;
                };
                let gained_problem = item.problem.is_some() && !row.warning.is_visible();
                fill(row, &item);
                drop(rows);
                if gained_problem {
                    self.imp().updating.set(true);
                    self.imp().rows.borrow()[i].check.set_active(false);
                    self.imp().updating.set(false);
                    self.changed();
                }
            }
        }
    }

    /// The checked items, by their place in the list.
    pub fn selected(&self) -> Vec<usize> {
        self.imp()
            .rows
            .borrow()
            .iter()
            .enumerate()
            .filter(|(_, row)| row.check.is_active())
            .map(|(i, _)| i)
            .collect()
    }

    fn set_items(&self, items: &[PickItem], note: Option<&str>) {
        let imp = self.imp();
        let list = imp.list.get().expect("built");
        list.remove_all();
        let group = imp.group.get().expect("built");
        group.set_description(note);
        group.set_title(
            &ngettext("{n} Podcast", "{n} Podcasts", items.len() as u32)
                .replace("{n}", &items.len().to_string()),
        );
        let rows: Vec<_> = items
            .iter()
            .map(|item| {
                let row = self.row();
                fill(&row, item);
                row.check.set_active(item.problem.is_none());
                list.append(&row.row);
                row
            })
            .collect();
        imp.rows.replace(rows);
        self.changed();
    }

    fn row(&self) -> Row {
        let check = gtk::CheckButton::builder()
            .valign(gtk::Align::Center)
            .build();
        let cover = Cover::new(COVER_SIZE);
        cover.widget().set_valign(gtk::Align::Center);
        cover.widget().set_margin_top(6);
        cover.widget().set_margin_bottom(6);
        let warning = gtk::Image::builder()
            .icon_name("dialog-warning-symbolic")
            .css_classes(["warning"])
            .visible(false)
            .build();
        let row = adw::ActionRow::builder()
            // Feed text, not markup.
            .use_markup(false)
            .title_lines(2)
            .subtitle_lines(1)
            .build();
        // Prefixes are packed from the start inwards: the check box first.
        row.add_prefix(cover.widget());
        row.add_prefix(&check);
        row.add_suffix(&warning);
        row.set_activatable_widget(Some(&check));
        let this = self.downgrade();
        check.connect_toggled(move |_| {
            if let Some(this) = this.upgrade()
                && !this.imp().updating.get()
            {
                this.changed();
            }
        });
        Row {
            row,
            check,
            cover,
            warning,
        }
    }

    fn check_all(&self, checked: bool) {
        self.imp().updating.set(true);
        for row in self.imp().rows.borrow().iter() {
            row.check.set_active(checked);
        }
        self.imp().updating.set(false);
        self.changed();
    }

    fn changed(&self) {
        self.update_toggle_all();
        self.imp()
            .output
            .emit(FeedPickerOutput::Selected(self.selected()));
    }

    /// Select All while any is unchecked, else Select None.
    fn update_toggle_all(&self) {
        let imp = self.imp();
        let Some(button) = imp.toggle_all.get() else {
            return;
        };
        let rows = imp.rows.borrow().len();
        button.set_visible(rows > 0);
        button.set_label(&if self.selected().len() < rows {
            gettext("Select _All")
        } else {
            gettext("Select _None")
        });
    }
}

/// Show `item` on `row`.
fn fill(row: &Row, item: &PickItem) {
    row.row.set_title(&item.title);
    row.row.set_subtitle(&item.url);
    row.cover.set(item.cover.as_deref());
    row.warning.set_visible(item.problem.is_some());
    if let Some(problem) = &item.problem {
        row.warning.set_tooltip_text(Some(problem));
        row.warning
            .update_property(&[gtk::accessible::Property::Label(problem)]);
        // Read with the row, as it cannot be focused itself.
        row.row
            .upcast_ref::<gtk::Widget>()
            .update_property(&[gtk::accessible::Property::Description(problem)]);
    } else {
        gtk::prelude::AccessibleExt::reset_property(
            row.row.upcast_ref::<gtk::Widget>(),
            gtk::AccessibleProperty::Description,
        );
    }
}

#[cfg(test)]
impl FeedPicker {
    /// Each row's title, subtitle, checked, and problem shown. For tests.
    pub fn rows(&self) -> Vec<(String, String, bool, bool)> {
        self.imp()
            .rows
            .borrow()
            .iter()
            .map(|r| {
                (
                    r.row.title().to_string(),
                    r.row.subtitle().unwrap_or_default().to_string(),
                    r.check.is_active(),
                    r.warning.is_visible(),
                )
            })
            .collect()
    }

    /// Click row `i`, as on its background. For tests.
    pub fn activate_row(&self, i: usize) {
        adw::prelude::ActionRowExt::activate(&self.imp().rows.borrow()[i].row);
    }

    /// Press Select All or Select None, returning its label. For tests.
    pub fn press_toggle_all(&self) -> String {
        let button = self.imp().toggle_all.get().expect("built").clone();
        let label = button.label().unwrap_or_default().to_string();
        button.emit_clicked();
        label
    }
}
