// SPDX-License-Identifier: GPL-3.0-or-later

//! M7's scenarios in real widgets: searching the library from the root
//! page, Search Online, This Show and Everywhere on a podcast page, the
//! Add page's states with canned directory answers, and the new headers in
//! the narrowest window.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use beacon_core::ItunesError;
use beacon_core::model::ShowId;
use beacon_core::services::itunes::DirectoryShow;
use chrono::Utc;
use gtk::prelude::*;
use url::Url;

use super::{NARROWEST, episode, first, minimum_width, run_until, window};
use crate::navigation::Location;
use crate::ui::add_page::{ADD_TAG, AddPageInput};
use crate::ui::content::{Content, ContentInput, ContentOutput, SearchScope};
use crate::ui::directory_row::DirectoryItem;
use crate::ui::models::{ShowCard, ShowPageData};
use crate::ui::preview_page::PreviewPage;
use crate::ui::search_data::SearchResults;
use crate::ui::show_page::{ShowPage, ShowPageInput, ShowPageOutput};

fn wait(ms: u64) {
    let until = std::time::Instant::now() + Duration::from_millis(ms);
    run_until(|| std::time::Instant::now() > until);
}

fn card() -> ShowCard {
    ShowCard {
        id: ShowId(1),
        title: "The Long Signal".into(),
        author: Some("Night Desk".into()),
        cover: None,
        new_count: 0,
    }
}

fn page_data() -> ShowPageData {
    ShowPageData {
        show: beacon_core::model::Show {
            id: ShowId(1),
            feed_url: "https://example.com/feed.xml".parse().expect("a URL"),
            info: beacon_core::model::ShowInfo {
                title: "The Long Signal".into(),
                ..Default::default()
            },
            cover_path: None,
            subscribed_at: Utc::now(),
            last_refreshed_at: None,
        },
        prefs: Default::default(),
        seasons: None,
        episodes: (1..=3).map(|id| episode(id, None)).collect(),
        all_played: false,
        to_download: Vec::new(),
        query: None,
    }
}

/// Each section's search asks for its own list only, shows the results in
/// place of the section under the title Search Results, and with nothing
/// found offers Search Online in Shows alone, which opens Add searching
/// Apple for it.
pub(super) fn the_root_page_searches_its_section() {
    let content = Content::default();
    let outputs: Rc<RefCell<Vec<ContentOutput>>> = Rc::default();
    let seen = outputs.clone();
    content.connect_output(move |output| seen.borrow_mut().push(output));
    let window = window(&content);
    let (results, add) = content.search_pages();

    // New searches New, and offers nothing online.
    content.send(ContentInput::ShowSection(Location::New));
    content.toggle_search();
    let (entry, _, _) = content.search_state();
    assert_eq!(
        entry.placeholder_text().as_deref(),
        Some("Search New Episodes")
    );
    entry.set_text("signal");
    let asked = |section: Location| {
        outputs.borrow().iter().any(|o| {
            matches!(o, ContentOutput::Search(SearchScope::Section(s), Some(q)) if *s == section && q == "signal")
        })
    };
    run_until(|| asked(Location::New));
    assert!(asked(Location::New));
    let (_, title, _) = content.search_state();
    assert_eq!(title, "Search Results");
    let mut found = SearchResults::new("signal", Location::New);
    found.episodes = vec![episode(1, None)];
    content.send(ContentInput::SetSearch(Box::new(found)));
    assert!(!results.showing_empty());
    content.send(ContentInput::SetSearch(Box::new(SearchResults::new(
        "signal",
        Location::New,
    ))));
    assert!(results.showing_empty());
    assert!(
        !results.online_button().is_visible(),
        "only Shows looks online"
    );

    // Shows searches its shows; with none found, Search Online.
    content.send(ContentInput::ShowSection(Location::Shows));
    content.toggle_search();
    let (entry, _, _) = content.search_state();
    assert_eq!(entry.placeholder_text().as_deref(), Some("Search Shows"));
    entry.set_text("signal");
    run_until(|| asked(Location::Shows));
    assert!(asked(Location::Shows));
    let mut found = SearchResults::new("signal", Location::Shows);
    found.shows = vec![card()];
    content.send(ContentInput::SetSearch(Box::new(found)));
    assert!(!results.showing_empty());
    content.send(ContentInput::SetSearch(Box::new(SearchResults::new(
        "signal",
        Location::Shows,
    ))));
    assert!(results.showing_empty());
    let online = results.online_button();
    assert!(online.is_visible() && online.has_css_class("suggested-action"));
    online.emit_clicked();
    wait(400);
    let (_, _, page) = content.search_state();
    assert_eq!(page.as_deref(), Some(ADD_TAG));
    assert!(
        outputs
            .borrow()
            .iter()
            .any(|o| matches!(o, ContentOutput::SearchDirectory(q) if q == "signal"))
    );
    let (showing, _) = add.showing();
    assert_eq!(showing, "searching");

    // Choosing a section brings it back, without a search.
    content.send(ContentInput::ShowSection(Location::New));
    let (_, title, _) = content.search_state();
    assert_eq!(title, "New");
    window.destroy();
}

/// On a podcast page the search narrows the page's own episodes, and
/// closing it ends the search.
pub(super) fn the_podcast_page_searches_its_episodes() {
    let page = ShowPage::new(&gtk::ToggleButton::new());
    let queries: Rc<RefCell<Vec<Option<String>>>> = Rc::default();
    let seen = queries.clone();
    page.connect_output(move |output| {
        if let ShowPageOutput::Search(_, query) = output {
            seen.borrow_mut().push(query);
        }
    });
    page.send(ShowPageInput::Set(Box::new(page_data())));
    let nav = adw::NavigationView::new();
    nav.push(&page);
    let window = window(&nav);
    page.search().type_text("episode");
    run_until(|| !queries.borrow().is_empty());
    assert_eq!(queries.borrow().last(), Some(&Some("episode".into())));
    // No scope to choose: the bar holds the field alone.
    assert!(first::<adw::ToggleGroup>(page.upcast_ref()).is_none());
    page.toggle_search();
    assert_eq!(queries.borrow().last(), Some(&None));
    window.destroy();
}

fn directory_item(title: &str) -> DirectoryItem {
    DirectoryItem {
        show: DirectoryShow {
            id: 1,
            title: title.into(),
            author: Some("Night Desk".into()),
            feed_url: Url::parse("https://example.com/feed.xml").expect("a URL"),
            artwork: None,
            genre: Some("Science".into()),
            episode_count: Some(12),
            latest: None,
            store_url: None,
        },
        subscribed: None,
        art: None,
    }
}

/// The Add page searches once for the same words, shows Apple's results,
/// says when there are none or when Apple refused, and ignores an answer
/// to words no longer in the field.
pub(super) fn the_add_page_shows_the_directorys_answers() {
    let content = Content::default();
    let window = window(&content);
    let outputs: Rc<RefCell<Vec<ContentOutput>>> = Rc::default();
    let seen = outputs.clone();
    content.connect_output(move |output| seen.borrow_mut().push(output));
    content.send(ContentInput::OpenAdd);
    let (_, add) = content.search_pages();
    add.send(AddPageInput::Search("long signal".into()));
    add.send(AddPageInput::Search("Long  Signal".into()));
    let searches = outputs
        .borrow()
        .iter()
        .filter(|o| matches!(o, ContentOutput::SearchDirectory(_)))
        .count();
    assert_eq!(
        searches,
        1,
        "the same words search once: {:?}",
        outputs.borrow()
    );
    assert_eq!(add.showing().0, "searching");

    add.send(AddPageInput::Results("other words".into(), Ok(Vec::new())));
    assert_eq!(add.showing().0, "searching", "a stale answer is ignored");
    add.send(AddPageInput::Results(
        "long signal".into(),
        Ok(vec![directory_item("The Long Signal")]),
    ));
    assert_eq!(
        add.showing(),
        ("results".into(), vec!["The Long Signal".into()])
    );

    add.send(AddPageInput::Search("nothing here".into()));
    add.send(AddPageInput::Results("nothing here".into(), Ok(Vec::new())));
    assert_eq!(add.showing().0, "none");

    add.send(AddPageInput::Search("too many".into()));
    add.send(AddPageInput::Results(
        "too many".into(),
        Err(ItunesError::RateLimited(Duration::from_secs(30))),
    ));
    assert_eq!(add.showing().0, "error");
    // After an error the same words may be tried again.
    add.send(AddPageInput::Search("too many".into()));
    assert_eq!(add.showing().0, "searching");

    // A feed address is not searched for.
    add.send(AddPageInput::Search("https://example.com/feed.xml".into()));
    wait(100);
    assert_eq!(add.showing().0, "feed");
    window.destroy();
}

/// The podcast page's header, with Search added, and the preview's fit
/// the narrowest window with three window buttons.
pub(super) fn narrow_search_headers_fit() {
    let settings = gtk::Settings::default().expect("GTK started");
    let layout = settings.gtk_decoration_layout();
    settings.set_gtk_decoration_layout(Some("menu:minimize,maximize,close"));
    let details = gtk::ToggleButton::new();
    // The narrow layout hides the details toggle.
    details.set_visible(false);
    let page = ShowPage::new(&details);
    page.send(ShowPageInput::Set(Box::new(page_data())));
    crate::ui::selection_bar::set_narrow(true);
    let preview = PreviewPage::default();
    let preview_details = gtk::ToggleButton::new();
    preview_details.set_visible(false);
    preview.pack_end_buttons(&preview_details);
    let mut needed = Vec::new();
    for (name, page) in [
        (
            "podcast page",
            page.upcast_ref::<adw::NavigationPage>().clone(),
        ),
        (
            "preview",
            preview.upcast_ref::<adw::NavigationPage>().clone(),
        ),
    ] {
        let nav = adw::NavigationView::new();
        nav.add(&adw::NavigationPage::new(&gtk::Label::new(None), "Root"));
        nav.push(&page);
        let window = gtk::Window::builder()
            .default_width(NARROWEST)
            .default_height(640)
            .child(&nav)
            .build();
        window.present();
        wait(300);
        let header = first::<adw::HeaderBar>(page.upcast_ref()).expect("a header");
        needed.push((name, minimum_width(&header)));
        if name == "podcast page" {
            page.downcast_ref::<ShowPage>()
                .expect("the podcast page")
                .toggle_search();
            wait(200);
            let bar = first::<gtk::SearchBar>(page.upcast_ref()).expect("a search bar");
            needed.push(("podcast search bar", minimum_width(&bar)));
        }
        window.destroy();
    }
    crate::ui::selection_bar::set_narrow(false);
    settings.set_gtk_decoration_layout(layout.as_deref());
    for (name, needed) in needed {
        assert!(needed <= NARROWEST, "the {name} needs {needed}px");
    }
}
