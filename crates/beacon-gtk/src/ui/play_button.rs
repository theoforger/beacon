// SPDX-License-Identifier: GPL-3.0-or-later

//! Play buttons while their episode loads: a spinner in place of the icon
//! or label until the player has the stream (asked by the user,
//! 2026-11-07). The episode's row and the details pane's Play are greyed
//! out meanwhile; the player bar's and the slim bar's round buttons stay as
//! they are, the spinner in the middle, and still pause.

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk::{self, prelude::*};

use crate::player::PlaybackState;

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

/// Set an icon-only play button for its episode's `state`: a spinner while
/// it loads (the button greyed out if `grey_out`), Pause while it plays,
/// otherwise Play.
pub fn set_icon(button: &gtk::Button, state: Option<PlaybackState>, grey_out: bool) {
    if is_loading(state) {
        show_loading(button, grey_out);
        return;
    }
    let (icon, label) = if is_playing(state) {
        ("media-playback-pause-symbolic", gettext("Pause"))
    } else {
        ("media-playback-start-symbolic", gettext("Play"))
    };
    button.set_sensitive(true);
    button.set_icon_name(icon);
    button.set_tooltip_text(Some(&label));
    button.update_property(&[gtk::accessible::Property::Label(&label)]);
}

/// Set a labelled play button: a spinner while its episode loads, else
/// `label` (with its access key).
pub fn set_label(button: &gtk::Button, loading: bool, label: &str) {
    if loading {
        show_loading(button, true);
        return;
    }
    button.set_sensitive(true);
    button.set_label(label);
    button.set_tooltip_text(None);
    button.reset_property(gtk::AccessibleProperty::Label);
}

fn show_loading(button: &gtk::Button, grey_out: bool) {
    button.set_sensitive(!grey_out);
    if button
        .child()
        .is_some_and(|child| child.is::<adw::Spinner>())
    {
        return;
    }
    let label = gettext("Loading");
    button.set_child(Some(&adw::Spinner::new()));
    button.set_tooltip_text(Some(&label));
    button.update_property(&[gtk::accessible::Property::Label(&label)]);
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
