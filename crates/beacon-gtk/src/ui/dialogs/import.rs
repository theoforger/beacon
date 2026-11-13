// SPDX-License-Identifier: GPL-3.0-or-later

//! Import Subscriptions (`docs/specs/opml.md`): a dialog that first asks
//! for an OPML file, dropped on it or chosen, then lists the podcasts the
//! file adds, each filling in its cover and title as its feed loads, to
//! choose which to import (decided by the user 2026-11-13). The HIG's
//! Dialogs page: action dialogs have "two primary buttons", the cancel
//! button "first", and "the affirmative dialog button should be
//! insensitive until the required options have been selected". The dialog
//! only shows state; the window reads the file and loads the feeds.

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gdk, gio, glib};

use crate::ui::feed_picker::{FeedPicker, FeedPickerInput, FeedPickerOutput, PickItem};
use crate::ui::messages::Output;

#[derive(Debug)]
pub enum ImportInput {
    /// The file's new podcasts, with a note on those left out.
    Feeds {
        items: Vec<PickItem>,
        note: Option<String>,
    },
    /// What a feed's preview showed.
    Feed(usize, PickItem),
    /// How many feeds have loaded (done of total), or `None` once all
    /// have.
    Loading(Option<(usize, usize)>),
    /// A file that cannot be imported, or holds nothing new.
    Problem(String),
}

#[derive(Debug)]
pub enum ImportOutput {
    /// Choose File…: open the file chooser.
    ChooseFile,
    /// A file dropped on the dialog.
    Dropped(gio::File),
    /// Import: the chosen podcasts, by their place in the list.
    Import(Vec<usize>),
    /// The dialog has closed, imported or not.
    Closed,
}

const CHOOSE: &str = "choose";
const PICK: &str = "pick";

mod imp {
    use std::cell::OnceCell;

    use super::*;

    #[derive(Default)]
    pub struct ImportDialog {
        pub widgets: OnceCell<super::Widgets>,
        pub output: Output<ImportOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ImportDialog {
        const NAME: &'static str = "BeaconImportDialog";
        type Type = super::ImportDialog;
        type ParentType = adw::Dialog;
    }

    impl ObjectImpl for ImportDialog {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }
    impl WidgetImpl for ImportDialog {}
    impl AdwDialogImpl for ImportDialog {}
}

glib::wrapper! {
    pub struct ImportDialog(ObjectSubclass<imp::ImportDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for ImportDialog {
    fn default() -> Self {
        glib::Object::new()
    }
}

pub struct Widgets {
    #[cfg_attr(not(test), expect(dead_code, reason = "read by the display tests"))]
    title: adw::WindowTitle,
    import: gtk::Button,
    progress: gtk::ProgressBar,
    stack: gtk::Stack,
    #[cfg_attr(not(test), expect(dead_code, reason = "read by the display tests"))]
    drop_area: gtk::Box,
    #[cfg_attr(not(test), expect(dead_code, reason = "read by the display tests"))]
    choose: gtk::Button,
    picker: FeedPicker,
    toasts: adw::ToastOverlay,
}

impl ImportDialog {
    fn build(&self) {
        self.set_title(&gettext("Import Subscriptions"));
        self.set_content_width(520);
        self.set_content_height(640);
        let output = self.imp().output.clone();

        let title = adw::WindowTitle::new(&gettext("Import Subscriptions"), "");
        let cancel = gtk::Button::builder()
            .label(gettext("_Cancel"))
            .use_underline(true)
            .build();
        let import = gtk::Button::builder()
            .label(gettext("_Import"))
            .use_underline(true)
            .css_classes(["suggested-action"])
            .sensitive(false)
            .build();
        let header = adw::HeaderBar::builder()
            .title_widget(&title)
            .show_start_title_buttons(false)
            .show_end_title_buttons(false)
            .build();
        header.pack_start(&cancel);
        header.pack_end(&import);
        // One bar for every feed loading, not a spinner each (the HIG's
        // Spinners page: "Avoid showing numerous spinners simultaneously").
        let progress = gtk::ProgressBar::builder()
            .valign(gtk::Align::Start)
            .visible(false)
            .build();
        // As the content's refresh bar; added, not set, which would drop
        // `horizontal`.
        progress.add_css_class("refresh-progress");
        progress.update_property(&[gtk::accessible::Property::Label(&gettext(
            "Loading podcasts",
        ))]);

        let (drop_area, choose) = drop_area();
        let picker = FeedPicker::default();
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::SlideLeft)
            .build();
        stack.add_named(&drop_area, Some(CHOOSE));
        stack.add_named(&picker, Some(PICK));
        let overlay = gtk::Overlay::builder().child(&stack).build();
        overlay.add_overlay(&progress);
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&overlay));
        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&toolbar));
        self.set_child(Some(&toasts));
        self.set_focus(Some(&choose));

        cancel.connect_clicked({
            let this = self.downgrade();
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.close();
                }
            }
        });
        choose.connect_clicked({
            let output = output.clone();
            move |_| output.emit(ImportOutput::ChooseFile)
        });
        let drop = gtk::DropTarget::new(gio::File::static_type(), gdk::DragAction::COPY);
        drop.connect_drop({
            let output = output.clone();
            move |_, value, _, _| {
                let Ok(file) = value.get::<gio::File>() else {
                    return false;
                };
                output.emit(ImportOutput::Dropped(file));
                true
            }
        });
        drop_area.add_controller(drop);
        import.connect_clicked({
            let (output, picker) = (output.clone(), picker.clone());
            move |_| output.emit(ImportOutput::Import(picker.selected()))
        });
        picker.connect_output({
            let (title, import) = (title.clone(), import.clone());
            move |FeedPickerOutput::Selected(selected)| {
                import.set_sensitive(!selected.is_empty());
                title.set_subtitle(&selected_message(selected.len()));
            }
        });
        self.connect_closed({
            let output = output.clone();
            move |_| output.emit(ImportOutput::Closed)
        });

        let _ = self.imp().widgets.set(Widgets {
            title,
            import,
            progress,
            stack,
            drop_area,
            choose,
            picker,
            toasts,
        });
    }

    fn widgets(&self) -> &Widgets {
        self.imp().widgets.get().expect("built")
    }

    pub fn connect_output(&self, receive: impl Fn(ImportOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    pub fn send(&self, input: ImportInput) {
        let w = self.widgets();
        match input {
            ImportInput::Feeds { items, note } => {
                w.picker.send(FeedPickerInput::Set { items, note });
                w.stack.set_visible_child_name(PICK);
                self.set_focus(Some(&w.import));
            }
            ImportInput::Feed(i, item) => w.picker.send(FeedPickerInput::Update(i, item)),
            ImportInput::Loading(progress) => {
                w.progress.set_visible(progress.is_some());
                if let Some((done, total)) = progress {
                    w.progress.set_fraction(done as f64 / total.max(1) as f64);
                    w.progress
                        .update_property(&[gtk::accessible::Property::ValueText(
                            &progress_message(done, total),
                        )]);
                }
            }
            ImportInput::Problem(message) => w.toasts.add_toast(adw::Toast::new(&message)),
        }
    }
}

/// The drop box, with Choose File… inside it. Dashed like the Add Podcast
/// tile, and tinted while a file is dragged over it (`style.css`).
fn drop_area() -> (gtk::Box, gtk::Button) {
    let icon = gtk::Image::builder()
        .icon_name("document-open-symbolic")
        .pixel_size(64)
        .css_classes(["dim-label"])
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    let heading = gtk::Label::builder()
        .label(gettext("Drop an OPML File Here"))
        .css_classes(["title-2"])
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();
    let description = gtk::Label::builder()
        .label(gettext(
            "Export your subscriptions from another podcast app, then drop the file here or choose it",
        ))
        .css_classes(["dim-label"])
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();
    let choose = gtk::Button::builder()
        .label(gettext("Choose _File…"))
        .use_underline(true)
        .css_classes(["pill"])
        .halign(gtk::Align::Center)
        .margin_top(12)
        .build();
    let area = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .valign(gtk::Align::Fill)
        .vexpand(true)
        .margin_top(12)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .css_classes(["drop-area"])
        .build();
    let inner = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .valign(gtk::Align::Center)
        .vexpand(true)
        .margin_start(24)
        .margin_end(24)
        .build();
    inner.append(&icon);
    inner.append(&heading);
    inner.append(&description);
    inner.append(&choose);
    area.append(&inner);
    (area, choose)
}

/// How many of the listed podcasts are chosen, under the dialog's title.
pub fn selected_message(n: usize) -> String {
    if n == 0 {
        return gettext("None selected");
    }
    ngettext("{n} selected", "{n} selected", n as u32).replace("{n}", &n.to_string())
}

/// How far loading or importing has got, for a progress bar's value: the
/// HIG's Progress Bars page asks for one naming what is counted.
pub fn progress_message(done: usize, total: usize) -> String {
    // Translators: how far an import has got, such as "12 of 44 podcasts".
    ngettext(
        "{done} of {total} podcast",
        "{done} of {total} podcasts",
        total as u32,
    )
    .replace("{done}", &done.to_string())
    .replace("{total}", &total.to_string())
}

/// How many of the chosen podcasts are in the library now.
pub fn imported_message(imported: usize, total: usize) -> String {
    if imported == total {
        ngettext(
            "Imported {n} podcast",
            "Imported {n} podcasts",
            total as u32,
        )
    } else {
        // Translators: {n} podcasts of {total} were imported.
        ngettext(
            "Imported {n} of {total} podcast",
            "Imported {n} of {total} podcasts",
            total as u32,
        )
    }
    .replace("{n}", &imported.to_string())
    .replace("{total}", &total.to_string())
}

#[cfg(test)]
impl ImportDialog {
    pub fn picker(&self) -> FeedPicker {
        self.widgets().picker.clone()
    }

    /// Showing the file page (else the list), Import's sensitivity, the
    /// subtitle, and whether the loading bar shows. For tests.
    pub fn state(&self) -> (bool, bool, String, bool) {
        let w = self.widgets();
        (
            w.stack.visible_child_name().as_deref() == Some(CHOOSE),
            w.import.is_sensitive(),
            w.title.subtitle().to_string(),
            w.progress.is_visible(),
        )
    }

    pub fn press_choose(&self) {
        self.widgets().choose.emit_clicked();
    }

    pub fn press_import(&self) {
        self.widgets().import.emit_clicked();
    }

    pub fn drop_area_has_target(&self) -> bool {
        let area = &self.widgets().drop_area;
        let controllers = area.observe_controllers();
        (0..controllers.n_items()).any(|i| {
            controllers
                .item(i)
                .is_some_and(|c| c.is::<gtk::DropTarget>())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_messages_count_podcasts() {
        assert_eq!(imported_message(1, 1), "Imported 1 podcast");
        assert_eq!(imported_message(44, 44), "Imported 44 podcasts");
        assert_eq!(imported_message(41, 44), "Imported 41 of 44 podcasts");
        assert_eq!(progress_message(12, 44), "12 of 44 podcasts");
        assert_eq!(progress_message(0, 1), "0 of 1 podcast");
        assert_eq!(selected_message(0), "None selected");
        assert_eq!(selected_message(3), "3 selected");
    }
}
