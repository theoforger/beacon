// SPDX-License-Identifier: GPL-3.0-or-later

//! Where the content pane is. Navigation is a breadcrumb trail, not a
//! history: a top-level section from the sidebar, then any deeper pages
//! opened from it. Back only leaves a deeper page for its parent; changing
//! section starts a new trail. No GTK here, so the rules are unit-tested
//! without a display.

use beacon_core::model::ShowId;

/// A top-level section: the sidebar items and Add.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Location {
    Shows,
    New,
    Queue,
    Favorites,
    Downloaded,
    Add,
}

impl Location {
    /// The sidebar's main items, in order. Add sits apart, at the bottom.
    pub const SIDEBAR: [Location; 5] = [
        Location::Shows,
        Location::New,
        Location::Queue,
        Location::Favorites,
        Location::Downloaded,
    ];

    /// Stable name, used for stack pages.
    pub fn id(self) -> &'static str {
        match self {
            Location::Shows => "shows",
            Location::New => "new",
            Location::Queue => "queue",
            Location::Favorites => "favorites",
            Location::Downloaded => "downloaded",
            Location::Add => "add",
        }
    }

    /// Whether the header offers Search here. The Add view has its own field.
    pub fn has_search(self) -> bool {
        self != Location::Add
    }
}

/// A page below a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subpage {
    /// A podcast's page, opened from Shows (M3).
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "opened from the Shows grid in M3")
    )]
    Show(ShowId),
}

#[derive(Debug, Clone)]
pub struct Trail {
    section: Location,
    pages: Vec<Subpage>,
}

impl Trail {
    pub fn new(section: Location) -> Self {
        Self {
            section,
            pages: Vec::new(),
        }
    }

    pub fn section(&self) -> Location {
        self.section
    }

    /// The deepest open page, if any.
    #[cfg_attr(not(test), expect(dead_code, reason = "used from M3"))]
    pub fn page(&self) -> Option<Subpage> {
        self.pages.last().copied()
    }

    /// Back is offered only on a deeper page.
    pub fn can_go_back(&self) -> bool {
        !self.pages.is_empty()
    }

    /// Switch to a section, leaving any deeper pages. Returns true if
    /// anything changed.
    pub fn go(&mut self, section: Location) -> bool {
        let changed = self.section != section || !self.pages.is_empty();
        self.section = section;
        self.pages.clear();
        changed
    }

    /// Open a page one level deeper.
    #[cfg_attr(not(test), expect(dead_code, reason = "used from M3"))]
    pub fn open(&mut self, page: Subpage) {
        self.pages.push(page);
    }

    /// Leave the deepest page for its parent. Returns false at the top level.
    pub fn back(&mut self) -> bool {
        self.pages.pop().is_some()
    }
}

impl Default for Trail {
    /// The app opens on Shows.
    fn default() -> Self {
        Self::new(Location::Shows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHOW: Subpage = Subpage::Show(ShowId(1));

    #[test]
    fn opens_on_shows_at_the_top_level() {
        let trail = Trail::default();
        assert_eq!(trail.section(), Location::Shows);
        assert_eq!(trail.page(), None);
        assert!(!trail.can_go_back());
    }

    #[test]
    fn switching_sections_never_offers_back() {
        let mut trail = Trail::default();
        assert!(trail.go(Location::Queue));
        assert!(trail.go(Location::Add));
        assert!(!trail.can_go_back());
        assert!(!trail.back());
        assert_eq!(trail.section(), Location::Add);
    }

    #[test]
    fn back_leaves_a_deeper_page_for_its_parent() {
        let mut trail = Trail::default();
        trail.open(SHOW);
        assert!(trail.can_go_back());
        assert_eq!(trail.page(), Some(SHOW));
        assert!(trail.back());
        assert_eq!(trail.page(), None);
        assert_eq!(trail.section(), Location::Shows);
        assert!(!trail.can_go_back());
    }

    #[test]
    fn switching_sections_leaves_deeper_pages() {
        let mut trail = Trail::default();
        trail.open(SHOW);
        assert!(trail.go(Location::New));
        assert!(!trail.can_go_back());
        // Choosing the current section from a deeper page returns to its top.
        trail.go(Location::Shows);
        trail.open(SHOW);
        assert!(trail.go(Location::Shows));
        assert_eq!(trail.page(), None);
    }

    #[test]
    fn choosing_the_current_section_at_its_top_changes_nothing() {
        let mut trail = Trail::default();
        assert!(!trail.go(Location::Shows));
    }

    #[test]
    fn only_add_hides_search() {
        assert!(!Location::Add.has_search());
        assert!(Location::SIDEBAR.iter().all(|l| l.has_search()));
    }
}
