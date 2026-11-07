// SPDX-License-Identifier: GPL-3.0-or-later

//! Play buttons while their episode loads (asked by the user, 2026-11-07):
//! every play button for the episode is greyed out and cannot be pressed
//! until the player has the stream, and a spinner the size of the play icon
//! shows over it in full colour. Libadwaita greys a disabled button with a
//! filter over all of it, so the spinner lies on top of the button
//! (`with_spinner`) rather than inside it. The button's own icon or label
//! is hidden meanwhile but keeps its room, so nothing changes size.

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk::{self, prelude::*};

use crate::player::PlaybackState;

/// The play icon's size, which the spinner matches.
const ICON_SIZE: i32 = 16;

/// Whether the player is still getting the stream.
pub fn is_loading(state: Option<PlaybackState>) -> bool {
    state == Some(PlaybackState::Buffering)
}

/// Whether the episode plays, or is about to.
pub fn is_playing(state: Option<PlaybackState>) -> bool {
    matches!(
        state,
        Some(PlaybackState::Playing | PlaybackState::Buffering)
    )
}

/// `button` with room for the loading spinner over it: put this where the
/// button would go.
pub fn with_spinner(button: &gtk::Button) -> gtk::Overlay {
    let spinner = adw::Spinner::builder()
        .width_request(ICON_SIZE)
        .height_request(ICON_SIZE)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .can_target(false)
        .visible(false)
        .build();
    let overlay = gtk::Overlay::builder()
        .child(button)
        .valign(button.valign())
        .halign(button.halign())
        .build();
    overlay.add_overlay(&spinner);
    overlay
}

/// Set an icon-only play button for its episode's `state`: loading, Pause
/// while it plays, otherwise Play.
pub fn set_icon(button: &gtk::Button, state: Option<PlaybackState>) {
    if show_loading(button, is_loading(state)) {
        return;
    }
    let (icon, label) = if is_playing(state) {
        ("media-playback-pause-symbolic", gettext("Pause"))
    } else {
        ("media-playback-start-symbolic", gettext("Play"))
    };
    button.set_icon_name(icon);
    button.set_tooltip_text(Some(&label));
    button.update_property(&[gtk::accessible::Property::Label(&label)]);
}

/// A labelled play button that keeps one width whatever it says: its
/// labels (with their access keys) stacked, the widest setting the width.
/// `labels` are page names and texts.
pub fn stacked_labels(button: &gtk::Button, labels: &[(&str, String)]) {
    let stack = gtk::Stack::builder().hhomogeneous(true).build();
    for (name, text) in labels {
        let label = gtk::Label::with_mnemonic(text);
        label.set_mnemonic_widget(Some(button));
        stack.add_named(&label, Some(name));
    }
    button.set_child(Some(&stack));
}

/// Show page `name` of a button from `stacked_labels`, or loading.
pub fn set_label(button: &gtk::Button, loading: bool, name: &str) {
    if show_loading(button, loading) {
        return;
    }
    let Some(stack) = button.child().and_downcast::<gtk::Stack>() else {
        return;
    };
    stack.set_visible_child_name(name);
    if let Some(label) = stack.visible_child().and_downcast::<gtk::Label>() {
        button.update_property(&[gtk::accessible::Property::Label(&label.text())]);
    }
    button.set_tooltip_text(None);
}

/// Grey `button` out under its spinner while loading, or bring it back.
/// Returns whether it is loading.
fn show_loading(button: &gtk::Button, loading: bool) -> bool {
    button.set_sensitive(!loading);
    if loading {
        // Hidden, keeping its room (`style.css`).
        button.add_css_class("loading");
        let label = gettext("Loading");
        button.set_tooltip_text(Some(&label));
        button.update_property(&[gtk::accessible::Property::Label(&label)]);
    } else {
        button.remove_css_class("loading");
    }
    if let Some(spinner) = button
        .parent()
        .and_downcast::<gtk::Overlay>()
        .and_then(|overlay| overlay.last_child())
        .and_downcast::<adw::Spinner>()
    {
        spinner.set_visible(loading);
    }
    loading
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffering_is_loading_and_counts_as_playing() {
        assert!(is_loading(Some(PlaybackState::Buffering)));
        assert!(!is_loading(Some(PlaybackState::Playing)));
        assert!(!is_loading(None));
        assert!(is_playing(Some(PlaybackState::Buffering)));
        assert!(is_playing(Some(PlaybackState::Playing)));
        assert!(!is_playing(Some(PlaybackState::Paused)));
        assert!(!is_playing(None));
    }
}
