// SPDX-License-Identifier: GPL-3.0-or-later

//! The details pane's widgets: built once, then set as episodes come and go.

use gettextrs::gettext;
use relm4::adw::prelude::*;
use relm4::gtk::{self, gio, pango};
use relm4::{ComponentSender, adw};

use super::{
    ACTIONS, COVER_SIZE, DetailsPane, DetailsPaneInput, DetailsPaneOutput, DetailsPaneWidgets,
};
use crate::ui::covers::Cover;
use crate::ui::episode_actions;
use crate::ui::links;
use crate::ui::models::RowAction;
use crate::ui::play_button;

fn heading(label: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(label)
        .xalign(0.0)
        .margin_top(18)
        .css_classes(["heading"])
        .accessible_role(gtk::AccessibleRole::Heading)
        .build()
}

pub(super) fn widgets(
    root: &adw::ToolbarView,
    sender: &ComponentSender<DetailsPane>,
) -> DetailsPaneWidgets {
    let header = adw::HeaderBar::builder()
        .title_widget(&adw::WindowTitle::new(&gettext("Episode Details"), ""))
        .build();
    let back = gtk::Button::builder()
        .icon_name("go-previous-symbolic")
        .visible(false)
        .tooltip_text(gettext("Back"))
        .build();
    back.update_property(&[gtk::accessible::Property::Label(&gettext("Back"))]);
    back.connect_clicked({
        let sender = sender.clone();
        move |_| {
            let _ = sender.output(DetailsPaneOutput::Close);
        }
    });
    header.pack_start(&back);
    // The episode's other actions, as the row menu has them, in the header
    // as the HIG places a content item's secondary menu, and as GNOME
    // Podcasts' episode details do (`episode_description.ui`).
    let more = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(gettext("More Options"))
        .visible(false)
        .build();
    more.update_property(&[gtk::accessible::Property::Label(&gettext("More Options"))]);
    let actions = gio::SimpleActionGroup::new();
    episode_actions::add_to(&actions, {
        let sender = sender.clone();
        move |action| sender.input(DetailsPaneInput::Pressed(action))
    });
    more.insert_action_group(ACTIONS, Some(&actions));
    header.pack_end(&more);
    root.add_top_bar(&header);

    let stack = gtk::Stack::new();
    let empty = adw::StatusPage::builder()
        .icon_name("audio-x-generic-symbolic")
        .title(gettext("No Episode Selected"))
        .description(gettext("Choose an episode to see its details here"))
        .build();
    stack.add_named(&empty, Some("empty"));

    let cover = Cover::new(COVER_SIZE);
    // The show, as a link to its page.
    let show_label = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label", "caption-heading"])
        .build();
    let show = gtk::Button::builder()
        .child(&show_label)
        .halign(gtk::Align::Start)
        .margin_top(12)
        .css_classes(["flat", "show-link"])
        .tooltip_text(gettext("Open Podcast"))
        .build();
    let title = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .css_classes(["title-2"])
        .accessible_role(gtk::AccessibleRole::Heading)
        .build();
    let subtitle = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();

    // Play, Pause or Resume, the one accent button; the other actions are
    // in the header's More Options.
    let play = gtk::Button::builder()
        .valign(gtk::Align::Center)
        .css_classes(["pill", "suggested-action"])
        .build();
    // One width whatever it says, or while it loads.
    play_button::stacked_labels(
        &play,
        &[
            ("play", gettext("_Play")),
            ("pause", gettext("_Pause")),
            ("resume", gettext("_Resume")),
        ],
    );
    let play_holder = play_button::with_spinner(&play);
    play_holder.set_halign(gtk::Align::Start);
    play_holder.set_margin_top(12);

    let about = heading(&gettext("About This Episode"));
    let description = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .use_markup(true)
        .build();
    // Links, bold, italics and lists, as GNOME Podcasts shows them, and like
    // it not selectable: a selectable label selects all its text when it
    // takes focus. Links open in the browser through the portal.
    links::arrow_keys_between_links(&description);

    let chapters_heading = heading(&gettext("Chapters"));
    chapters_heading.set_visible(false);
    // A boxed list, as GNOME Podcasts lists chapters (`chapters.ui`).
    let chapters = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .margin_top(6)
        .visible(false)
        .build();
    chapters.update_property(&[gtk::accessible::Property::Label(&gettext("Chapters"))]);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();
    content.append(cover.widget());
    for widget in [
        show.upcast_ref::<gtk::Widget>(),
        title.upcast_ref(),
        subtitle.upcast_ref(),
        play_holder.upcast_ref(),
        about.upcast_ref(),
        description.upcast_ref(),
        chapters_heading.upcast_ref(),
        chapters.upcast_ref(),
    ] {
        content.append(widget);
    }
    let clamp = adw::Clamp::builder()
        .maximum_size(600)
        .child(&content)
        .build();
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&clamp)
        .build();
    stack.add_named(&scrolled, Some("episode"));
    stack.set_visible_child_name("empty");
    root.set_content(Some(&stack));

    let widgets = DetailsPaneWidgets {
        stack,
        back,
        cover,
        show,
        show_label,
        title,
        subtitle,
        play,
        more,
        about,
        description,
        chapters_heading,
        chapters,
    };
    connect(&widgets, sender);
    widgets
}

/// Wire Play, the show link and the chapter rows to the pane.
fn connect(widgets: &DetailsPaneWidgets, sender: &ComponentSender<DetailsPane>) {
    let input = sender.clone();
    widgets
        .play
        .connect_clicked(move |_| input.input(DetailsPaneInput::Pressed(RowAction::Play)));
    let input = sender.clone();
    widgets
        .show
        .connect_clicked(move |_| input.input(DetailsPaneInput::ShowClicked));
    let input = sender.clone();
    widgets.chapters.connect_row_activated(move |_, row| {
        if let Ok(index) = usize::try_from(row.index()) {
            input.input(DetailsPaneInput::ChapterChosen(index));
        }
    });
}
