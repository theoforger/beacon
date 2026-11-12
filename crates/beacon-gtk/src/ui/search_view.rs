// SPDX-License-Identifier: GPL-3.0-or-later

//! Search results, in place of the section (or of a podcast page's
//! episodes, in Everywhere scope), as the mockup lays them out: the
//! matching shows as tiles under Shows, then the matching episodes in one
//! card under a heading that counts them. With nothing found, a status page
//! whose suggested Search Online opens the Add page with the query.

use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use beacon_core::model::ShowId;
use gettextrs::{gettext, ngettext};
use gtk::glib;

use crate::navigation::Location;
use crate::ui::covers::Cover;
use crate::ui::episode_list::{Entry, EpisodeList};
use crate::ui::messages::Output;
use crate::ui::models::{EpisodeItem, RowAction, ShowCard};
use crate::ui::search_data::SearchResults;
use crate::ui::selection::Selection;
use crate::ui::shows_grid::show_tile_with;
use crate::ui::widgets::episode_row::RowSetup;

#[derive(Debug)]
pub enum SearchViewInput {
    Set(Box<SearchResults>),
}

#[derive(Debug)]
pub enum SearchViewOutput {
    /// A row's action, with the section searched, whose list it is from.
    Action(Box<EpisodeItem>, RowAction, Location),
    OpenShow(ShowId),
    /// Search Online: look for the query in Apple's directory.
    SearchOnline(String),
}

mod imp {
    use std::cell::{OnceCell, RefCell};

    use super::*;

    #[derive(Default)]
    pub struct SearchView {
        pub parts: OnceCell<super::Parts>,
        pub results: RefCell<SearchResults>,
        pub output: Output<SearchViewOutput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SearchView {
        const NAME: &'static str = "BeaconSearchView";
        type Type = super::SearchView;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for SearchView {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }
    impl WidgetImpl for SearchView {}
    impl BinImpl for SearchView {}
}

glib::wrapper! {
    pub struct SearchView(ObjectSubclass<imp::SearchView>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

pub struct Parts {
    list: EpisodeList,
    shows: gtk::FlowBox,
    shows_heading: gtk::Label,
    stack: gtk::Stack,
    empty: adw::StatusPage,
    online: gtk::Button,
}

impl Default for SearchView {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl SearchView {
    fn build(&self) {
        let output = self.imp().output.clone();
        // Rows name their show, as in the sections' own lists.
        let rows = RowSetup {
            on_action: Rc::new({
                let output = output.clone();
                let this = self.downgrade();
                move |item, action| {
                    let Some(view) = this.upgrade() else {
                        return;
                    };
                    let section = view.imp().results.borrow().section;
                    output.emit(SearchViewOutput::Action(
                        Box::new(item.clone()),
                        action,
                        section,
                    ));
                }
            }),
            show_identity: true,
            selection: Selection::default(),
            selectable: false,
        };

        // The shows, as on the Shows grid, 2 to 7 to a line.
        let shows = gtk::FlowBox::builder()
            .homogeneous(true)
            .min_children_per_line(1)
            .max_children_per_line(7)
            .selection_mode(gtk::SelectionMode::None)
            .activate_on_single_click(true)
            .css_classes(["search-shows"])
            .build();
        shows.update_property(&[gtk::accessible::Property::Label(&gettext("Shows"))]);
        let this = self.downgrade();
        shows.connect_child_activated(move |_, child| {
            let Some(view) = this.upgrade() else {
                return;
            };
            let id = view
                .imp()
                .results
                .borrow()
                .shows
                .get(child.index() as usize)
                .map(|card| card.id);
            if let Some(id) = id {
                view.imp().output.emit(SearchViewOutput::OpenShow(id));
            }
        });
        let shows_heading = gtk::Label::builder()
            .xalign(0.0)
            .css_classes(["heading"])
            .accessible_role(gtk::AccessibleRole::Heading)
            .margin_top(32)
            .margin_bottom(6)
            .build();
        let header = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        header.append(&shows_heading);
        header.append(&shows);

        // Translators: how a screen reader names the list of search results.
        let list = EpisodeList::new(
            &rows,
            &gettext("Search Results"),
            Some(header.upcast()),
            None,
        );

        let online = gtk::Button::builder()
            .label(gettext("_Search Online"))
            .use_underline(true)
            .halign(gtk::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .build();
        let this = self.downgrade();
        online.connect_clicked(move |_| {
            if let Some(view) = this.upgrade() {
                let query = view.imp().results.borrow().query.clone();
                view.imp()
                    .output
                    .emit(SearchViewOutput::SearchOnline(query));
            }
        });
        let empty = adw::StatusPage::builder()
            .icon_name("system-search-symbolic")
            .title(gettext("No Results Found"))
            .child(&online)
            .build();

        let stack = gtk::Stack::builder().vexpand(true).build();
        stack.add_named(list.widget(), Some("results"));
        stack.add_named(&empty, Some("empty"));
        stack.set_visible_child_name("empty");
        self.set_child(Some(&stack));
        let _ = self.imp().parts.set(Parts {
            list,
            shows,
            shows_heading,
            stack,
            empty,
            online,
        });
    }

    pub fn connect_output(&self, receive: impl Fn(SearchViewOutput) + 'static) {
        self.imp().output.connect(receive);
    }

    pub fn send(&self, input: SearchViewInput) {
        let SearchViewInput::Set(results) = input;
        let imp = self.imp();
        let parts = imp.parts.get().expect("built when constructed");
        if imp.results.borrow().shows != results.shows {
            set_tiles(&parts.shows, &results.shows);
        }
        let count = results.shows.len();
        parts.shows_heading.set_label(
            &ngettext("{n} Show", "{n} Shows", count as u32).replace("{n}", &count.to_string()),
        );
        // Only Shows looks further, in Apple's directory: the other
        // sections hold what is already subscribed.
        let shows = results.section == Location::Shows;
        parts.online.set_visible(shows);
        parts.empty.set_description(Some(&if shows {
            gettext("Try a different search term, or look for new podcasts online")
        } else {
            gettext("Try a different search term")
        }));
        parts.stack.set_visible_child_name(if results.is_empty() {
            "empty"
        } else {
            "results"
        });
        parts.list.set(entries(&results));
        imp.results.replace(*results);
    }

    /// Whether the status page shows, for tests.
    #[cfg(test)]
    pub fn showing_empty(&self) -> bool {
        let parts = self.imp().parts.get().expect("built when constructed");
        parts.stack.visible_child_name().as_deref() == Some("empty")
    }

    /// Search Online, for tests.
    #[cfg(test)]
    pub fn online_button(&self) -> gtk::Button {
        let parts = self.imp().parts.get().expect("built when constructed");
        parts.online.clone()
    }
}

fn set_tiles(flow: &gtk::FlowBox, cards: &[ShowCard]) {
    flow.remove_all();
    for card in cards {
        // The Shows grid's narrowest cover: two still fit at 360px.
        flow.append(&show_tile_with(card, Cover::new(150)));
    }
}

/// The shows, when there are any, then the episodes in one card under a
/// heading that counts them.
fn entries(results: &SearchResults) -> Vec<Entry> {
    let mut entries = Vec::new();
    if !results.shows.is_empty() {
        entries.push(Entry::Header);
    }
    let count = results.episodes.len();
    if count > 0 {
        entries.push(Entry::Heading {
            title: ngettext("{n} Episode", "{n} Episodes", count as u32)
                .replace("{n}", &count.to_string()),
            fold: None,
            top: if results.shows.is_empty() { 32 } else { 24 },
        });
        entries.extend(Entry::card(&results.episodes));
    }
    entries.push(Entry::Space(32));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::episode_actions::item;

    #[test]
    fn shows_or_the_counted_episodes() {
        let card = ShowCard {
            id: ShowId(1),
            title: "The Long Signal".into(),
            author: None,
            cover: None,
            new_count: 0,
        };
        let mut shows = SearchResults::new("signal", Location::Shows);
        shows.shows = vec![card];
        assert_eq!(entries(&shows), [Entry::Header, Entry::Space(32)]);

        let mut episodes = SearchResults::new("signal", Location::New);
        episodes.episodes = vec![item()];
        let found = entries(&episodes);
        assert!(matches!(&found[0], Entry::Heading { title, .. } if title == "1 Episode"));
        assert!(matches!(
            found[1],
            Entry::Episode {
                first: true,
                last: true,
                ..
            }
        ));
    }
}
