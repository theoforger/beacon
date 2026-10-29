// SPDX-License-Identifier: GPL-3.0-or-later

//! Where the content pane is, and how it got there. No GTK here, so the rules
//! are unit-tested without a display.

/// A place the content pane can show.
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

/// Browser-style history without Forward: going somewhere new pushes it,
/// Back pops. Going to the current location is not a step.
#[derive(Debug, Clone)]
pub struct History {
    stack: Vec<Location>,
}

impl History {
    pub fn new(start: Location) -> Self {
        Self { stack: vec![start] }
    }

    pub fn current(&self) -> Location {
        *self.stack.last().expect("history is never empty")
    }

    pub fn can_go_back(&self) -> bool {
        self.stack.len() > 1
    }

    /// Returns true if the location changed.
    pub fn go(&mut self, location: Location) -> bool {
        if self.current() == location {
            return false;
        }
        self.stack.push(location);
        true
    }

    /// Returns the location now shown, or `None` if there was nowhere to go.
    pub fn back(&mut self) -> Option<Location> {
        if !self.can_go_back() {
            return None;
        }
        self.stack.pop();
        Some(self.current())
    }
}

impl Default for History {
    /// The app opens on Shows.
    fn default() -> Self {
        Self::new(Location::Shows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_on_shows_with_nothing_to_go_back_to() {
        let history = History::default();
        assert_eq!(history.current(), Location::Shows);
        assert!(!history.can_go_back());
    }

    #[test]
    fn going_somewhere_pushes_and_back_pops() {
        let mut history = History::default();
        assert!(history.go(Location::Queue));
        assert!(history.go(Location::Add));
        assert!(history.can_go_back());
        assert_eq!(history.back(), Some(Location::Queue));
        assert_eq!(history.back(), Some(Location::Shows));
        assert_eq!(history.back(), None);
        assert_eq!(history.current(), Location::Shows);
    }

    #[test]
    fn going_to_the_current_location_is_not_a_step() {
        let mut history = History::default();
        assert!(!history.go(Location::Shows));
        assert!(!history.can_go_back());
        history.go(Location::New);
        assert!(!history.go(Location::New));
        assert_eq!(history.back(), Some(Location::Shows));
    }

    #[test]
    fn revisiting_a_location_is_a_new_step() {
        let mut history = History::default();
        history.go(Location::New);
        history.go(Location::Shows);
        assert_eq!(history.back(), Some(Location::New));
        assert_eq!(history.back(), Some(Location::Shows));
    }

    #[test]
    fn only_add_hides_search() {
        assert!(!Location::Add.has_search());
        assert!(Location::SIDEBAR.iter().all(|l| l.has_search()));
    }
}
