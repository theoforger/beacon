// SPDX-License-Identifier: GPL-3.0-or-later

//! Checks that need widgets laid out with libadwaita's stylesheet and
//! Beacon's: the narrowest window's parts fit it, and long episode lists
//! make rows only for what is on screen. They need a display; without one
//! (CI has none) there is nothing to measure and the test passes. One test,
//! since GTK starts on one thread only.

use std::rc::Rc;
use std::time::Duration;

use beacon_core::model::{Episode, EpisodeId, EpisodeInfo, EpisodeState, Season, ShowId};
use chrono::Utc;
use relm4::gtk::{self, gdk, glib, prelude::*};
use relm4::{Component, ComponentController, adw};

use crate::ui::dialogs::description::text_height;
use crate::ui::episode_list::{Entry, EpisodeList};
use crate::ui::models::{EpisodeItem, SeasonItems, ShowPageData};
use crate::ui::play_button;
use crate::ui::player_bar::PlayerBar;
use crate::ui::selection::Selection;
use crate::ui::show_page::{ShowPage, ShowPageInput};
use crate::ui::widgets::episode_row::{EpisodeRow, RowSetup};

/// The narrowest window Beacon supports.
const NARROWEST: i32 = 360;
/// What New's list takes from the window's width around a row: its 12px
/// margins on each side.
const AROUND_ROW: i32 = 12 * 2;

fn partly_played() -> EpisodeItem {
    EpisodeItem {
        episode: Episode {
            id: EpisodeId(1),
            show_id: ShowId(1),
            info: EpisodeInfo {
                title: "A long episode title that goes on and on".into(),
                duration: Some(Duration::from_secs(3600)),
                ..Default::default()
            },
            state: EpisodeState {
                position: Duration::from_secs(600),
                ..Default::default()
            },
            added_at: Utc::now(),
        },
        show_title: "A show with a long name".into(),
        cover: None,
        in_queue: false,
    }
}

fn minimum_width(widget: &impl IsA<gtk::Widget>) -> i32 {
    widget.measure(gtk::Orientation::Horizontal, -1).0
}

fn setup() -> RowSetup {
    RowSetup {
        on_action: Rc::new(|_, _| {}),
        show_identity: true,
        selection: Selection::default(),
    }
}

/// Episode `id` of show 1, a few days older than episode `id - 1`.
fn episode(id: i64, season: Option<u32>) -> EpisodeItem {
    let mut item = partly_played();
    item.episode.id = EpisodeId(id);
    item.episode.info.title = format!("Episode {id}");
    item.episode.info.season = season.map(|number| Season { number, name: None });
    item.episode.state.position = Duration::ZERO;
    item
}

/// Run the main loop until `done`, or for a few seconds at most.
fn run_until(mut done: impl FnMut() -> bool) {
    let context = glib::MainContext::default();
    let start = std::time::Instant::now();
    while !done() && start.elapsed() < Duration::from_secs(5) {
        context.iteration(false);
    }
}

fn count<T: IsA<gtk::Widget>>(widget: &gtk::Widget) -> usize {
    let mut found = usize::from(widget.is::<T>());
    let mut child = widget.first_child();
    while let Some(widget) = child {
        found += count::<T>(&widget);
        child = widget.next_sibling();
    }
    found
}

fn window(child: &impl IsA<gtk::Widget>) -> gtk::Window {
    let window = gtk::Window::builder()
        .default_width(800)
        .default_height(600)
        .child(child)
        .build();
    window.present();
    window
}

#[test]
fn widgets_lay_out_as_designed() {
    if gtk::init().is_err() {
        return;
    }
    adw::init().expect("libadwaita starts once GTK has");
    let css = gtk::CssProvider::new();
    css.load_from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/style.css"));
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("GTK started, so there is a display"),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    the_narrow_layout_fits_the_narrowest_window();
    the_player_bar_lists_chapters();
    long_lists_make_rows_only_for_the_screen();
    seasons_fold_away_their_episodes();
}

/// A list of thousands of episodes makes rows for about a screenful, and
/// a change to one episode leaves the other rows alone.
fn long_lists_make_rows_only_for_the_screen() {
    let header = gtk::Label::new(Some("A podcast"));
    let list = EpisodeList::new(&setup(), "Episodes", Some(header.clone().upcast()), None);
    let items: Vec<_> = (1..=2000).map(|id| episode(id, None)).collect();
    let mut entries = vec![Entry::Header];
    entries.extend(Entry::card(&items));
    list.set(entries);
    let window = window(list.widget());
    let made = || count::<EpisodeRow>(list.widget().upcast_ref());
    run_until(|| made() > 0);
    let rows = made();
    assert!(
        rows > 0 && rows < crate::ui::episode_list::KEPT_ROWS,
        "{rows} rows made for 2000 episodes"
    );

    // The header is one widget, handed from row to row as the list scrolls
    // away from it and back.
    let adjustment = list.widget().vadjustment();
    for to in [f64::MAX, 0.0] {
        adjustment.set_value(to.min(adjustment.upper()));
        let settled = std::time::Instant::now() + Duration::from_millis(300);
        run_until(|| std::time::Instant::now() > settled);
    }
    assert!(header.is_mapped(), "the header shows again at the top");
    // Each row is as tall as what it holds: the header a line, an episode
    // its 64px and margins.
    let mut first_row = None;
    let mut stack = vec![list.widget().clone().upcast::<gtk::Widget>()];
    while let Some(widget) = stack.pop() {
        if widget.is::<EpisodeRow>() && widget.is_mapped() {
            first_row = Some(widget);
            break;
        }
        let mut child = widget.last_child();
        while let Some(next) = child {
            child = next.prev_sibling();
            stack.push(next);
        }
    }
    let row = first_row.expect("an episode row shows");
    assert!(
        header.height() < 40 && row.height() >= 76,
        "header {}px, row {}px",
        header.height(),
        row.height()
    );
    window.destroy();
}

/// Folding a season takes its episodes out of the list; unfolding brings
/// them back.
fn seasons_fold_away_their_episodes() {
    let page = ShowPage::builder()
        .launch(gtk::ToggleButton::new())
        .detach();
    let season = |number: u32, ids: std::ops::RangeInclusive<i64>| SeasonItems {
        season: Some(Season { number, name: None }),
        episodes: ids.map(|id| episode(id, Some(number))).collect(),
    };
    let mut data = ShowPageData {
        show: beacon_core::model::Show {
            id: ShowId(1),
            feed_url: "https://example.com/feed.xml".parse().expect("a URL"),
            info: Default::default(),
            cover_path: None,
            subscribed_at: Utc::now(),
            last_refreshed_at: None,
        },
        prefs: Default::default(),
        seasons: Some(vec![season(2, 4..=6), season(1, 1..=3)]),
        episodes: Vec::new(),
        all_played: false,
    };
    page.emit(ShowPageInput::Set(Box::new(data.clone())));
    let nav = adw::NavigationView::new();
    nav.push(page.widget());
    let window = window(&nav);
    let rows = || count::<EpisodeRow>(page.widget().upcast_ref());
    run_until(|| rows() == 6);
    assert_eq!(rows(), 6);

    let expanders = || {
        let mut found = Vec::new();
        let mut stack = vec![page.widget().clone().upcast::<gtk::Widget>()];
        while let Some(widget) = stack.pop() {
            if let Ok(expander) = widget.clone().downcast::<gtk::Expander>() {
                found.push(expander);
            }
            let mut child = widget.first_child();
            while let Some(next) = child {
                child = next.next_sibling();
                stack.push(next);
            }
        }
        found
    };
    let season_two = expanders()
        .into_iter()
        .find(|e| {
            e.label_widget()
                .and_downcast::<gtk::Label>()
                .is_some_and(|l| l.label().contains('2'))
        })
        .expect("season 2 has a heading");
    season_two.set_expanded(false);
    run_until(|| rows() == 3);
    assert_eq!(rows(), 3, "season 2's episodes are folded away");

    // A refresh keeps the fold.
    data.all_played = true;
    page.emit(ShowPageInput::Set(Box::new(data)));
    let settled = std::time::Instant::now() + Duration::from_millis(300);
    run_until(|| std::time::Instant::now() > settled);
    assert_eq!(rows(), 3);
    window.destroy();
}

/// The player bar's Chapters button is available once the playing
/// episode's chapters arrive; its popover lists them, marks the one
/// playing, and choosing one seeks to it.
fn the_player_bar_lists_chapters() {
    use beacon_core::model::Chapter;

    use crate::ui::player_bar::{NowPlayingCard, PlayerBarInput, PlayerBarOutput};

    let bar = PlayerBar::builder().launch(());
    let asked = Rc::new(std::cell::RefCell::new(Vec::new()));
    let record = asked.clone();
    let bar = bar.connect_receiver(move |_, output| {
        if let PlayerBarOutput::Seek(to) = output {
            record.borrow_mut().push(to);
        }
    });
    let window = window(bar.widget());
    bar.emit(PlayerBarInput::Episode(Some(Box::new(NowPlayingCard {
        id: EpisodeId(7),
        title: "Letters home".into(),
        show_title: "Paper Trails".into(),
        cover: None,
        duration: Some(Duration::from_secs(3600)),
    }))));
    let chapter = |minutes: u64, title: &str| Chapter {
        start: Duration::from_secs(minutes * 60),
        title: title.into(),
        url: None,
        image_url: None,
    };
    let chapters = vec![
        chapter(0, "Cold open"),
        chapter(10, "Interview"),
        chapter(40, "Letters"),
    ];
    let settle = || {
        let until = std::time::Instant::now() + Duration::from_millis(100);
        run_until(|| std::time::Instant::now() > until);
    };
    settle();
    let (button, list) = {
        let widgets = bar.widgets();
        let (button, list) = widgets.chapters();
        (button.clone(), list)
    };
    assert!(!button.is_sensitive(), "no chapters yet");

    // Another episode's chapters are not this one's.
    bar.emit(PlayerBarInput::Chapters(EpisodeId(8), chapters.clone()));
    settle();
    assert!(!button.is_sensitive());

    bar.emit(PlayerBarInput::Chapters(EpisodeId(7), chapters));
    bar.emit(PlayerBarInput::Position(Duration::from_secs(15 * 60)));
    settle();
    assert!(button.is_sensitive());
    let rows: Vec<_> = (0..).map_while(|i| list.row_at_index(i)).collect();
    assert_eq!(rows.len(), 3);
    assert!(rows[1].has_css_class("current-chapter"));
    assert!(!rows[0].has_css_class("current-chapter"));

    rows[2].activate();
    settle();
    assert_eq!(*asked.borrow(), [Duration::from_secs(40 * 60)]);
    window.destroy();
}

/// The narrowest window, 360px wide: the parts that span it must fit.
fn the_narrow_layout_fits_the_narrowest_window() {
    // A partly played row: Adwaita's progress bar alone wanted 150px.
    let row = EpisodeRow::new(&setup());
    row.bind(&partly_played());
    let needed = minimum_width(&row) + AROUND_ROW;
    assert!(needed <= NARROWEST, "a partly played row needs {needed}px");

    // The player spans the window: in the narrow layout, the slim bar and
    // the sheet with the whole player, speed, sleep timer and volume kept.
    let bar = PlayerBar::builder().launch(()).detach();
    bar.widgets().set_sheet(true);
    for (part, needed) in [
        ("slim bar", minimum_width(bar.widgets().mini())),
        ("player sheet", minimum_width(bar.widget())),
    ] {
        assert!(needed <= NARROWEST, "the narrow {part} needs {needed}px");
    }

    // The details pane's Play keeps one width whatever it says and while
    // it loads, and the spinner over it is the play icon's size.
    let play = gtk::Button::new();
    play_button::stacked_labels(
        &play,
        &[
            ("play", "_Play".into()),
            ("pause", "_Pause".into()),
            ("resume", "_Resume".into()),
        ],
    );
    let overlay = play_button::with_spinner(&play);
    let mut widths = Vec::new();
    for (page, loading) in [("play", false), ("resume", false), ("pause", true)] {
        play_button::set_label(&play, loading, page);
        widths.push(minimum_width(&overlay));
    }
    assert!(
        widths.windows(2).all(|w| w[0] == w[1]),
        "Play's widths: {widths:?}"
    );
    let spinner = overlay
        .last_child()
        .expect("the spinner lies over the button");
    assert!(spinner.is_visible() && !play.is_sensitive());
    assert_eq!(spinner.measure(gtk::Orientation::Horizontal, -1).1, 16);

    // Read More's dialog is as tall as its text at the width it gets: more
    // lines in a narrow window, never the single line it measured before.
    let text = gtk::Label::builder()
        .label("A show description long enough to wrap onto several lines. ".repeat(8))
        .wrap(true)
        .margin_start(24)
        .margin_end(24)
        .build();
    let line = gtk::Label::new(Some("One line"))
        .measure(gtk::Orientation::Vertical, -1)
        .1;
    let narrow = text_height(&text, NARROWEST);
    let wide = text_height(&text, 560);
    assert!(
        narrow > wide && wide > 3 * line,
        "{narrow}px narrow, {wide}px wide"
    );
}
