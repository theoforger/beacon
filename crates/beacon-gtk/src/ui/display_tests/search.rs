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
use crate::ui::search_data::SearchResults;
use crate::ui::show_page::{PageKind, ShowPage, ShowPageInput, ShowPageOutput};

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
    let page = ShowPage::new(&gtk::ToggleButton::new(), PageKind::Library);
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
    let searches = || {
        outputs
            .borrow()
            .iter()
            .filter(|o| matches!(o, ContentOutput::SearchDirectory(_)))
            .count()
    };
    // Typing goes nowhere (asked by the user 2026-11-12), however long it
    // pauses; Search sends it.
    add.type_text("long signal");
    wait(1200);
    assert_eq!(searches(), 0, "typing alone does not search");
    assert_eq!(add.showing().0, "start");
    assert!(add.press_search(), "Search is available for words");
    assert_eq!(searches(), 1);
    // The same words again, from Enter or Search Online, ask nothing new.
    add.send(AddPageInput::Activate);
    add.send(AddPageInput::Search("Long  Signal".into()));
    let searches = searches();
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

    // A feed address is not searched for, and Search is unavailable.
    add.type_text("https://example.com/feed.xml");
    wait(400);
    assert_eq!(add.showing().0, "feed");
    assert!(!add.press_search());
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
    let page = ShowPage::new(&details, PageKind::Library);
    page.send(ShowPageInput::Set(Box::new(page_data())));
    crate::ui::selection_bar::set_narrow(true);
    let preview_details = gtk::ToggleButton::new();
    preview_details.set_visible(false);
    let preview = ShowPage::new(&preview_details, PageKind::Preview);
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

/// A podcast page's header shows the show's name only once the hero's
/// large title has scrolled away, and typing opens its search while the
/// page shows.
pub(super) fn the_podcast_title_waits_for_the_hero_to_scroll_away() {
    let page = ShowPage::new(&gtk::ToggleButton::new(), PageKind::Library);
    let mut data = page_data();
    data.episodes = (1..=40).map(|id| episode(id, None)).collect();
    page.send(ShowPageInput::Set(Box::new(data)));
    let nav = adw::NavigationView::new();
    nav.add(&adw::NavigationPage::new(&gtk::Label::new(None), "Root"));
    nav.push(&page);
    let window = gtk::Window::builder()
        // Wide rows, as this window is not the narrow layout.
        .default_width(480)
        .default_height(640)
        .child(&nav)
        .build();
    window.present();
    wait(500);
    let header = first::<adw::HeaderBar>(page.upcast_ref()).expect("a header");
    let scrolled = first::<gtk::ScrolledWindow>(page.upcast_ref()).expect("the list");
    assert!(!header.shows_title(), "the hero's title is in view");
    let adjustment = scrolled.vadjustment();
    adjustment.set_value(adjustment.upper());
    wait(300);
    assert!(header.shows_title(), "scrolled past the hero");
    adjustment.set_value(0.0);
    wait(300);
    assert!(!header.shows_title(), "back at the top");
    // Straight from the bottom to near the top, as a fast scroll goes: the
    // hero is laid out again only after the jump (found by the user
    // 2026-11-12, the title stayed).
    for near_top in [0.0, 1.0, 20.0] {
        adjustment.set_value(adjustment.upper());
        wait(300);
        assert!(header.shows_title());
        adjustment.set_value(near_top);
        wait(300);
        assert!(!header.shows_title(), "jumped back to {near_top}");
    }

    let bar = first::<gtk::SearchBar>(page.upcast_ref()).expect("a search bar");
    assert_eq!(
        bar.key_capture_widget().as_ref(),
        Some(window.upcast_ref::<gtk::Widget>()),
        "typing opens the search"
    );
    nav.pop();
    wait(500);
    assert!(
        bar.key_capture_widget().is_none(),
        "not once the page has gone"
    );
    window.destroy();
}

/// Narrowing and widening the window leaves the details pane as it was
/// (found by the user 2026-11-12: widening always opened it, then, once
/// pinned, always closed it), with the app's own breakpoint setters.
pub(super) fn resizing_leaves_the_details_pane_as_it_was() {
    for open in [false, true] {
        let split = crate::app::details_split(&gtk::Label::new(None), &gtk::Label::new(None));
        let window = adw::Window::builder()
            .default_width(1000)
            .default_height(600)
            .content(&split)
            .build();
        adw::prelude::AdwWindowExt::add_breakpoint(&window, crate::app::details_breakpoint(&split));
        window.present();
        wait(200);
        split.set_show_sidebar(open);
        wait(100);
        window.set_default_size(400, 600);
        wait(300);
        assert!(
            split.is_collapsed() && !split.shows_sidebar(),
            "narrow hides it"
        );
        window.set_default_size(1000, 600);
        wait(300);
        assert!(!split.is_collapsed());
        assert_eq!(split.shows_sidebar(), open, "open before: {open}");
        // Opened while narrow, it stays open once wide.
        split.set_show_sidebar(false);
        window.set_default_size(400, 600);
        wait(300);
        split.set_show_sidebar(true);
        window.set_default_size(1000, 600);
        wait(300);
        assert!(split.shows_sidebar(), "opened while narrow");
        window.destroy();
    }
}

/// A show's preview is its podcast page with Subscribe: its feed's
/// episodes as rows that cannot act (no Play, no menu, no details), and
/// Subscribe only once the feed has loaded. Nothing it does reaches the
/// library but Subscribe.
pub(super) fn a_preview_is_the_podcast_page_with_subscribe() {
    use crate::ui::preview_data;
    use crate::ui::widgets::episode_row::EpisodeRow;

    let page = ShowPage::new(&gtk::ToggleButton::new(), PageKind::Preview);
    let outputs: Rc<RefCell<Vec<String>>> = Rc::default();
    let seen = outputs.clone();
    page.connect_output(move |output| seen.borrow_mut().push(format!("{output:?}")));
    let item = directory_item("Seasons");
    page.send(ShowPageInput::Preview(
        Box::new(preview_data::directory_page(&item)),
        None,
    ));
    let nav = adw::NavigationView::new();
    nav.push(&page);
    let window = window(&nav);
    wait(300);
    assert!(!page.can_subscribe(), "not before the feed loads");
    let loaded = preview_data::preview_page(&preview_data::tests::preview("seasons.xml"));
    page.send(ShowPageInput::PreviewLoaded(Ok(Box::new(loaded))));
    wait(300);
    assert!(page.can_subscribe());
    let rows = super::count::<EpisodeRow>(page.upcast_ref());
    assert!(rows > 0, "the feed's episodes show");
    // No row control, no filter, search or selection on a preview.
    assert!(first::<gtk::SearchBar>(page.upcast_ref()).is_none());
    let row = first::<EpisodeRow>(page.upcast_ref()).expect("a row");
    assert!(first::<gtk::MenuButton>(row.upcast_ref()).is_none());
    assert!(first::<gtk::Button>(row.upcast_ref()).is_none());
    page.press_subscribe();
    assert!(!page.can_subscribe(), "once, while it subscribes");
    assert_eq!(
        outputs.borrow().as_slice(),
        [
            r#"Subscribe(Url { scheme: "https", cannot_be_a_base: false, username: "", password: None, host: Some(Domain("example.com")), port: None, path: "/feed.xml", query: None, fragment: None })"#
        ]
    );
    page.send(ShowPageInput::SubscribeDone);
    assert!(page.can_subscribe());
    window.destroy();
}

/// Subscribing from a preview puts the show's page in its place, so Back
/// returns to the results rather than to the preview.
pub(super) fn subscribing_swaps_the_preview_for_the_shows_page() {
    let content = Content::default();
    let window = window(&content);
    content.send(ContentInput::OpenAdd);
    content.send(ContentInput::OpenPreview(Box::new(directory_item(
        "Seasons",
    ))));
    wait(600);
    let (_, _, page) = content.search_state();
    assert_eq!(page.as_deref(), Some(crate::ui::show_page::PREVIEW_TAG));
    content.send(ContentInput::OpenShow(Box::new(page_data())));
    let (_, _, page) = content.search_state();
    assert_eq!(page.as_deref(), Some(crate::ui::show_page::SHOW_TAG));
    content.widgets().go_back();
    wait(600);
    let (_, _, page) = content.search_state();
    assert_eq!(page.as_deref(), Some(ADD_TAG));
    window.destroy();
}
