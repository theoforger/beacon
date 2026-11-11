// SPDX-License-Identifier: GPL-3.0-or-later

//! A show found in Apple's directory, as a row of the Add page's results:
//! the cover, the title, and the author and genre, as GNOME Podcasts'
//! `discovery_found_podcast.ui` lays them out, in an `AdwActionRow` that
//! opens the show's preview. A show already subscribed to says so, and
//! opens its own page instead.

use std::path::PathBuf;

use adw::prelude::*;
use beacon_core::model::ShowId;
use beacon_core::services::itunes::DirectoryShow;
use gettextrs::gettext;

use crate::ui::covers::Cover;

/// GNOME Podcasts' 64px.
const COVER_SIZE: i32 = 64;

/// A directory result, with what the library knows of it.
#[derive(Debug, Clone, PartialEq)]
pub struct DirectoryItem {
    pub show: DirectoryShow,
    /// The show, when its feed is already subscribed to.
    pub subscribed: Option<ShowId>,
    /// Apple's thumbnail, once fetched into the scratch cache.
    pub art: Option<PathBuf>,
}

/// The author and genre, as the row's subtitle shows them.
pub fn subtitle(show: &DirectoryShow) -> String {
    [show.author.as_deref(), show.genre.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(&gettext(" • "))
}

/// The row for `item`, and its cover, which the thumbnail fills once it
/// arrives.
pub fn row(item: &DirectoryItem) -> (adw::ActionRow, Cover) {
    let cover = Cover::new(COVER_SIZE);
    cover.set(item.art.as_deref());
    cover.widget().set_valign(gtk::Align::Center);
    cover.widget().set_margin_top(6);
    cover.widget().set_margin_bottom(6);
    let row = adw::ActionRow::builder()
        .title(&item.show.title)
        .subtitle(subtitle(&item.show))
        // Feed text, not markup.
        .use_markup(false)
        .title_lines(2)
        .subtitle_lines(1)
        .activatable(true)
        .build();
    row.add_prefix(cover.widget());
    if item.subscribed.is_some() {
        let label = gtk::Label::builder()
            .label(gettext("Subscribed"))
            .css_classes(["dim-label"])
            .build();
        row.add_suffix(&label);
    }
    row.add_suffix(
        &gtk::Image::builder()
            .icon_name("go-next-symbolic")
            .accessible_role(gtk::AccessibleRole::Presentation)
            .build(),
    );
    (row, cover)
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::*;

    #[test]
    fn the_subtitle_has_what_apple_gives() {
        let mut show = DirectoryShow {
            id: 1,
            title: "The Long Signal".into(),
            author: Some("Night Desk".into()),
            feed_url: Url::parse("https://example.com/feed").unwrap(),
            artwork: None,
            genre: Some("Science".into()),
            episode_count: None,
            latest: None,
            store_url: None,
        };
        assert_eq!(subtitle(&show), "Night Desk • Science");
        show.author = None;
        assert_eq!(subtitle(&show), "Science");
        show.genre = None;
        assert_eq!(subtitle(&show), "");
    }
}
