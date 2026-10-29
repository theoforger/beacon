// SPDX-License-Identifier: GPL-3.0-or-later

//! The sections the sidebar switches between. Everything deeper (the Add
//! page, and podcast pages from M3) is a page pushed onto the content pane's
//! `AdwNavigationView`, which owns that stack, its Back and its gestures.

/// A section: one of the sidebar items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Location {
    Shows,
    New,
    Queue,
    Favorites,
    Downloaded,
}

impl Location {
    /// The sidebar items, in order. The app opens on the first.
    pub const ALL: [Location; 5] = [
        Location::Shows,
        Location::New,
        Location::Queue,
        Location::Favorites,
        Location::Downloaded,
    ];

    /// Whether the content header offers Refresh Feeds: the sections where
    /// new episodes appear.
    pub fn has_refresh(self) -> bool {
        matches!(self, Location::Shows | Location::New)
    }
}
