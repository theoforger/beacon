// SPDX-License-Identifier: GPL-3.0-or-later

//! Where the content pane is. Navigation is a breadcrumb trail, not a
//! history: a section from the sidebar, then any deeper pages opened from
//! it. Back only leaves a deeper page for its parent; changing section
//! starts a new trail. No GTK here, so the rules are unit-tested without a
//! display.

use beacon_core::model::ShowId;

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
    /// The sidebar items, in order.
    pub const ALL: [Location; 5] = [
        Location::Shows,
        Location::New,
        Location::Queue,
        Location::Favorites,
        Location::Downloaded,
    ];
}

/// A page below a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subpage {
    /// Search for a podcast or paste a feed address. Always under Shows.
    Add,
    /// A podcast's page, opened from Shows (M3).
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "opened from the Shows grid in M3")
    )]
    Show(ShowId),
}

/// What the content pane shows: a section's top level or a deeper page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Section(Location),
    Subpage(Subpage),
}

impl View {
    /// Whether the header offers Search here. The Add page has its own field.
    pub fn has_search(self) -> bool {
        self != View::Subpage(Subpage::Add)
    }
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

    /// The deepest open page, or the section's top level.
    pub fn view(&self) -> View {
        match self.pages.last() {
            Some(&page) => View::Subpage(page),
            None => View::Section(self.section),
        }
    }

    /// Back is offered only on a deeper page. The navigation view's header
    /// shows it, so only the tests ask.
    #[cfg(test)]
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

    /// Open a page one level deeper in the current section.
    pub fn open(&mut self, page: Subpage) {
        self.pages.push(page);
    }

    /// Open Add under Shows, from wherever the trail is. Opening it again
    /// while it is showing changes nothing.
    pub fn open_add(&mut self) -> bool {
        if self.view() == View::Subpage(Subpage::Add) {
            return false;
        }
        self.go(Location::Shows);
        self.open(Subpage::Add);
        true
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
        assert_eq!(trail.view(), View::Section(Location::Shows));
        assert!(!trail.can_go_back());
    }

    #[test]
    fn switching_sections_never_offers_back() {
        let mut trail = Trail::default();
        assert!(trail.go(Location::Queue));
        assert!(trail.go(Location::Favorites));
        assert!(!trail.can_go_back());
        assert!(!trail.back());
        assert_eq!(trail.view(), View::Section(Location::Favorites));
    }

    #[test]
    fn back_leaves_a_deeper_page_for_its_parent() {
        let mut trail = Trail::default();
        trail.open(SHOW);
        assert!(trail.can_go_back());
        assert_eq!(trail.view(), View::Subpage(SHOW));
        assert!(trail.back());
        assert_eq!(trail.view(), View::Section(Location::Shows));
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
        assert_eq!(trail.view(), View::Section(Location::Shows));
    }

    #[test]
    fn choosing_the_current_section_at_its_top_changes_nothing() {
        let mut trail = Trail::default();
        assert!(!trail.go(Location::Shows));
    }

    #[test]
    fn add_opens_under_shows_from_anywhere_and_back_returns_to_shows() {
        let mut trail = Trail::default();
        trail.go(Location::Queue);
        assert!(trail.open_add());
        assert_eq!(trail.section(), Location::Shows);
        assert_eq!(trail.view(), View::Subpage(Subpage::Add));
        assert!(!trail.open_add(), "already showing");
        assert!(trail.back());
        assert_eq!(trail.view(), View::Section(Location::Shows));
        assert!(!trail.can_go_back());
    }

    #[test]
    fn only_add_hides_search() {
        assert!(!View::Subpage(Subpage::Add).has_search());
        assert!(View::Subpage(SHOW).has_search());
        assert!(Location::ALL.iter().all(|&l| View::Section(l).has_search()));
    }
}
