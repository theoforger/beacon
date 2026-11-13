// SPDX-License-Identifier: GPL-3.0-or-later

//! Export Subscriptions (`docs/specs/opml.md`): a dialog listing every
//! subscription with its cover, title and feed address, all checked, to
//! choose which go in the file (decided by the user 2026-11-13). Export…
//! then asks where to save it. As the import dialog, Cancel comes first and
//! Export is insensitive while nothing is chosen (the HIG's Dialogs page).

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;

use crate::ui::dialogs::import::selected_message;
use crate::ui::feed_picker::{FeedPicker, FeedPickerInput, FeedPickerOutput, PickItem};
use crate::ui::messages::Output;

#[derive(Debug)]
pub enum ExportInput {
    Shows(Vec<PickItem>),
    /// The file could not be written.
    Problem(String),
}

#[derive(Debug)]
pub enum ExportOutput {
    /// Export…: the chosen podcasts, by their place in the list.
    Export(Vec<usize>),
}

mod imp {
    use std::cell::OnceCell;

    use super::*;

    #[derive(Default)]
    pub struct ExportDialog {
        pub widgets: OnceCell<super::Widgets>,
        pub output: Output<ExportOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ExportDialog {
        const NAME: &'static str = "BeaconExportDialog";
        type Type = super::ExportDialog;
        type ParentType = adw::Dialog;
    }

    impl ObjectImpl for ExportDialog {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }
    impl WidgetImpl for ExportDialog {}
    impl AdwDialogImpl for ExportDialog {}
}

glib::wrapper! {
    pub struct ExportDialog(ObjectSubclass<imp::ExportDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for ExportDialog {
    fn default() -> Self {
        glib::Object::new()
    }
}

pub struct Widgets {
    export: gtk::Button,
    picker: FeedPicker,
    toasts: adw::ToastOverlay,
}

impl ExportDialog {
    fn build(&self) {
        self.set_title(&gettext("Export Subscriptions"));
        self.set_content_width(520);
        self.set_content_height(640);

        let title = adw::WindowTitle::new(&gettext("Export Subscriptions"), "");
        let cancel = gtk::Button::builder()
            .label(gettext("_Cancel"))
            .use_underline(true)
            .build();
        // An ellipsis: the file's place is asked next (Writing Style).
        let export = gtk::Button::builder()
            .label(gettext("_Export…"))
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
        header.pack_end(&export);
        let picker = FeedPicker::default();
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&picker));
        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&toolbar));
        self.set_child(Some(&toasts));
        self.set_focus(Some(&export));

        cancel.connect_clicked({
            let this = self.downgrade();
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.close();
                }
            }
        });
        export.connect_clicked({
            let (output, picker) = (self.imp().output.clone(), picker.clone());
            move |_| output.emit(ExportOutput::Export(picker.selected()))
        });
        picker.connect_output({
            let (title, export) = (title.clone(), export.clone());
            move |FeedPickerOutput::Selected(selected)| {
                export.set_sensitive(!selected.is_empty());
                title.set_subtitle(&selected_message(selected.len()));
            }
        });
        let _ = self.imp().widgets.set(Widgets {
            export,
            picker,
            toasts,
        });
    }

    fn widgets(&self) -> &Widgets {
        self.imp().widgets.get().expect("built")
    }

    pub fn connect_output(&self, receive: impl Fn(ExportOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    pub fn send(&self, input: ExportInput) {
        let w = self.widgets();
        match input {
            ExportInput::Shows(items) => {
                w.picker.send(FeedPickerInput::Set { items, note: None });
                self.set_focus(Some(&w.export));
            }
            ExportInput::Problem(message) => w.toasts.add_toast(adw::Toast::new(&message)),
        }
    }
}

#[cfg(test)]
impl ExportDialog {
    pub fn picker(&self) -> FeedPicker {
        self.widgets().picker.clone()
    }

    /// Export's sensitivity. For tests.
    pub fn can_export(&self) -> bool {
        self.widgets().export.is_sensitive()
    }

    pub fn press_export(&self) {
        self.widgets().export.emit_clicked();
    }
}
