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
use gtk::{self, gdk, gio, glib, prelude::*};

use crate::ui::dialogs::description::text_height;
use crate::ui::episode_list::{Entry, EpisodeList};
use crate::ui::models::{EpisodeItem, SeasonItems, ShowPageData};
use crate::ui::play_button;
use crate::ui::player_bar::PlayerBar;
use crate::ui::selection::Selection;
use crate::ui::show_page::{ShowPage, ShowPageInput};
use crate::ui::widgets::episode_row::{EpisodeRow, RowSetup};

mod search;

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
                enclosure: Some(beacon_core::model::Enclosure {
                    url: url::Url::parse("https://example.com/1.mp3").unwrap(),
                    mime_type: None,
                    length: None,
                }),
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
        selectable: true,
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

/// The first `T` in `widget`, depth first.
fn first<T: IsA<gtk::Widget>>(widget: &gtk::Widget) -> Option<T> {
    if let Some(found) = widget.downcast_ref::<T>() {
        return Some(found.clone());
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(found) = first::<T>(&widget) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
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
    search::narrow_search_headers_fit();
    search::the_root_page_searches_its_section();
    search::the_podcast_page_searches_its_episodes();
    search::the_add_page_shows_the_directorys_answers();
    search::the_podcast_title_waits_for_the_hero_to_scroll_away();
    the_narrow_layout_fits_the_narrowest_window();
    narrow_headers_fit_three_window_buttons();
    clear_sits_where_refresh_does();
    the_player_bar_lists_chapters();
    the_slim_bar_shows_loading();
    the_details_pane_offers_the_row_menus_actions();
    favorite_rows_show_a_star();
    queued_rows_can_be_reordered();
    downloads_show_on_rows();
    progress_needs_a_length();
    preferences_follow_settings();
    crate::ui::dialogs::sync_page::the_sync_page_shows_each_state();
    the_podcast_header_fits_its_description();
    selection_shortcuts_work_in_selection_mode();
    rows_word_their_titles_and_subtitles();
    narrow_rows_leave_out_play_and_shrink_the_cover();
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
    let page = ShowPage::new(&gtk::ToggleButton::new());
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
        to_download: Vec::new(),
        query: None,
    };
    page.send(ShowPageInput::Set(Box::new(data.clone())));
    let nav = adw::NavigationView::new();
    nav.push(&page);
    let window = window(&nav);
    let rows = || count::<EpisodeRow>(page.upcast_ref());
    run_until(|| rows() == 6);
    assert_eq!(rows(), 6);

    let expanders = || {
        let mut found = Vec::new();
        let mut stack = vec![page.clone().upcast::<gtk::Widget>()];
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
    page.send(ShowPageInput::Set(Box::new(data)));
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

    let bar = PlayerBar::default();
    let asked = Rc::new(std::cell::RefCell::new(Vec::new()));
    let record = asked.clone();
    bar.connect_output(move |output| {
        if let PlayerBarOutput::Seek(to) = output {
            record.borrow_mut().push(to);
        }
    });
    let window = window(&bar);
    bar.send(PlayerBarInput::Episode(Some(Box::new(NowPlayingCard {
        id: EpisodeId(7),
        title: "Letters home".into(),
        show_title: "Paper Trails".into(),
        cover: None,
        // As in a feed without lengths: the player finds it once playing.
        duration: None,
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
    let (button, list, seek) = {
        let widgets = bar.widgets();
        let (button, list, seek) = widgets.chapters();
        (button.clone(), list, seek.clone())
    };
    assert!(!button.is_sensitive(), "no chapters yet");

    // Another episode's chapters are not this one's.
    bar.send(PlayerBarInput::Chapters(EpisodeId(8), chapters.clone()));
    settle();
    assert!(!button.is_sensitive());

    // The stored chapters come before the player knows the length.
    bar.send(PlayerBarInput::Chapters(EpisodeId(7), chapters));
    settle();
    bar.send(PlayerBarInput::Duration(Duration::from_secs(3600)));
    bar.send(PlayerBarInput::Position(Duration::from_secs(15 * 60)));
    settle();
    assert!(button.is_sensitive());
    // A tick at 10 and at 40 minutes of the hour, not both at the end.
    let ticks = ticks(&seek);
    assert_eq!(ticks.len(), 2, "{ticks:?}");
    for (tick, minutes) in ticks.iter().zip([10.0, 40.0]) {
        assert!(
            (tick - minutes / 60.0).abs() < 0.05,
            "the tick for {minutes} minutes is at {tick} of the bar: {ticks:?}"
        );
    }
    let rows: Vec<_> = (0..).map_while(|i| list.row_at_index(i)).collect();
    assert_eq!(rows.len(), 3);
    assert!(rows[1].has_css_class("current-chapter"));
    assert!(!rows[0].has_css_class("current-chapter"));

    rows[2].activate();
    settle();
    assert_eq!(*asked.borrow(), [Duration::from_secs(40 * 60)]);

    // How much the player holds fills the trough that far.
    bar.send(PlayerBarInput::Buffered(0.5));
    settle();
    let filled = fill(&seek);
    assert!(
        (filled - 0.5).abs() < 0.05,
        "half held fills {filled} of the trough"
    );
    window.destroy();
}

/// While its episode loads, the narrow layout's slim bar shows the
/// spinner over its Play, as the wide bar does (found off by the user).
fn the_slim_bar_shows_loading() {
    use crate::player::PlaybackState;
    use crate::ui::player_bar::{NowPlayingCard, PlayerBarInput};

    let bar = PlayerBar::default();
    bar.set_sheet(true);
    let window = window(bar.widgets().mini());
    bar.send(PlayerBarInput::Episode(Some(Box::new(NowPlayingCard {
        id: EpisodeId(7),
        title: "Letters home".into(),
        show_title: "Paper Trails".into(),
        cover: None,
        duration: None,
    }))));
    bar.send(PlayerBarInput::State(PlaybackState::Buffering));
    let until = std::time::Instant::now() + Duration::from_millis(300);
    run_until(|| std::time::Instant::now() > until);
    let play = bar.widgets().mini_play().clone();
    assert!(play.is_mapped(), "the slim bar shows Play");
    let spinner = play
        .parent()
        .and_then(|overlay| overlay.last_child())
        .filter(|child| child != &play)
        .expect("a spinner lies over the slim bar's Play");
    assert!(spinner.is_mapped(), "the spinner shows");
    assert!(
        spinner.width() >= 16 && spinner.height() >= 16,
        "the spinner is {}x{}",
        spinner.width(),
        spinner.height()
    );
    window.destroy();
}

/// Clear, in Queue and Downloaded, sits where Refresh Feeds sits in Shows
/// and New, at the very start of the header (found off by the user).
fn clear_sits_where_refresh_does() {
    use crate::navigation::Location;
    use crate::ui::content::{Content, ContentInput};

    let content = Content::default();
    let window = window(&content);
    let (refresh, clear) = content.start_buttons();
    let settle = || {
        let until = std::time::Instant::now() + Duration::from_millis(100);
        run_until(|| std::time::Instant::now() > until);
    };
    let x = |button: &gtk::Button| {
        button
            .compute_point(&content, &gtk::graphene::Point::new(0.0, 0.0))
            .expect("the button is in the pane")
            .x()
    };
    content.send(ContentInput::ShowSection(Location::Shows));
    settle();
    let refresh_x = x(&refresh);
    for section in [Location::Queue, Location::Downloaded] {
        content.send(ContentInput::ShowSection(section));
        settle();
        assert!(clear.is_mapped(), "{section:?} shows Clear");
        assert_eq!(x(&clear), refresh_x, "Clear in {section:?}");
    }
    window.destroy();
}

/// With minimise, maximise and close beside the title (GTK's default, and
/// some distributions'), New's header still fits the narrowest window once
/// the narrow layout has left out what it leaves out.
fn narrow_headers_fit_three_window_buttons() {
    use crate::navigation::Location;
    use crate::ui::content::{Content, ContentInput};

    let settings = gtk::Settings::default().expect("GTK started");
    let layout = settings.gtk_decoration_layout();
    settings.set_gtk_decoration_layout(Some("menu:minimize,maximize,close"));
    let content = Content::default();
    // In a collapsed split view, as in the app, so Back to the sidebar shows.
    let split = adw::NavigationSplitView::builder()
        .sidebar(&adw::NavigationPage::new(&gtk::Label::new(None), "Beacon"))
        .content(&adw::NavigationPage::new(&content, "New"))
        .collapsed(true)
        .show_content(true)
        .build();
    // The narrowest window, so the page's breakpoints fold what they fold
    // (selection mode's actions into one menu).
    let window = gtk::Window::builder()
        .default_width(NARROWEST)
        .default_height(640)
        .child(&split)
        .build();
    window.present();
    // What the narrow breakpoint hides.
    content.widgets().details_toggle().set_visible(false);
    content.widgets().refresh_holder().set_visible(false);
    crate::ui::selection_bar::set_narrow(true);
    // The page's breakpoint bin reports less and clips what it holds, so
    // measure the page inside it.
    let page =
        first::<adw::ToolbarView>(content.upcast_ref()).expect("the root page has a toolbar view");
    // New has the most buttons; Queue and Downloaded have Clear.
    let mut needed = Vec::new();
    for section in [Location::New, Location::Queue, Location::Downloaded] {
        content.send(ContentInput::ShowSection(section));
        let until = std::time::Instant::now() + Duration::from_millis(100);
        run_until(|| std::time::Instant::now() > until);
        needed.push((format!("{section:?}"), minimum_width(&page)));
        // Select is left out, and Ctrl+A still selects.
        let select = content.select_button();
        assert!(!select.is_visible(), "{section:?} leaves Select out");
        assert!(crate::ui::selection_bar::takes_shortcut(
            &select,
            &content.selection(),
            0
        ));
        // Selection mode's header too, with its longest heading ("No
        // Episodes Selected"), which overflowed 360px (found in the M5
        // review).
        content.selection().set_active(true);
        let until = std::time::Instant::now() + Duration::from_millis(100);
        run_until(|| std::time::Instant::now() > until);
        needed.push((format!("{section:?} selecting"), minimum_width(&page)));
        content.selection().set_active(false);
    }
    window.destroy();
    crate::ui::selection_bar::set_narrow(false);
    settings.set_gtk_decoration_layout(layout.as_deref());
    for (section, needed) in needed {
        assert!(
            needed <= NARROWEST,
            "{section}'s narrow page needs {needed}px"
        );
    }
}

/// The details pane's More Options holds the episode's actions as a row's
/// menu words them, and choosing one asks the app for it.
fn the_details_pane_offers_the_row_menus_actions() {
    use crate::ui::details_pane::{DetailsPane, DetailsPaneInput, DetailsPaneOutput};
    use crate::ui::episode_actions::items;
    use crate::ui::models::RowAction;

    let asked = Rc::new(std::cell::RefCell::new(Vec::new()));
    let record = asked.clone();
    let pane = DetailsPane::default();
    pane.connect_output(move |output| {
        if let DetailsPaneOutput::Action(item, action) = output {
            record.borrow_mut().push((item.episode.id, action));
        }
    });
    let window = window(&pane);
    let more = pane.widgets().more().clone();
    let settle = || {
        let until = std::time::Instant::now() + Duration::from_millis(100);
        run_until(|| std::time::Instant::now() > until);
    };
    settle();
    assert!(!more.is_mapped(), "no episode, no actions");

    let item = partly_played();
    let id = item.episode.id;
    pane.send(DetailsPaneInput::Show(Box::new(item)));
    settle();
    assert!(more.is_mapped());
    let labels: Vec<_> = items(&more.menu_model().expect("a menu"))
        .into_iter()
        .map(|(label, _)| label.replace('_', ""))
        .collect();
    assert_eq!(
        labels,
        [
            "Add to Queue",
            "Download",
            "Add to Favorites",
            "Mark as Played",
            "Reset Progress"
        ]
    );
    more.activate_action("episode.reset", None)
        .expect("More Options has the episode's actions");
    settle();
    assert_eq!(*asked.borrow(), [(id, RowAction::ResetProgress)]);

    pane.send(DetailsPaneInput::Clear);
    settle();
    assert!(!more.is_mapped());
    window.destroy();
}

/// In the narrow layout a row leaves out its Play, which the details page
/// has, and draws a smaller cover, giving the title the room; wide again,
/// both come back. Selection mode leaves out Play and its space too.
fn narrow_rows_leave_out_play_and_shrink_the_cover() {
    use crate::ui::widgets::episode_row;

    let row = EpisodeRow::new(&setup());
    row.bind(&partly_played());
    let window = window(&row);
    let settle = || {
        let until = std::time::Instant::now() + Duration::from_millis(100);
        run_until(|| std::time::Instant::now() > until);
    };
    let find = |action: &str| {
        let mut stack = vec![row.clone().upcast::<gtk::Widget>()];
        while let Some(widget) = stack.pop() {
            if widget
                .downcast_ref::<gtk::Button>()
                .is_some_and(|b| b.action_name().as_deref() == Some(action))
            {
                return widget;
            }
            let mut child = widget.first_child();
            while let Some(c) = child {
                child = c.next_sibling();
                stack.push(c);
            }
        }
        panic!("no {action} button");
    };
    let cover = first::<gtk::Overlay>(row.upcast_ref())
        .filter(|o| o.has_css_class("cover"))
        .expect("a row that shows its show has a cover");
    settle();
    let wide = minimum_width(&row);
    assert!(find("row.play").is_drawable());
    assert_eq!(cover.height(), 64);

    episode_row::set_narrow(true);
    settle();
    let narrow = minimum_width(&row);
    assert!(!find("row.play").is_drawable(), "no Play when narrow");
    assert_eq!(cover.height(), 48);
    // Play, its spacing and 16px of cover.
    assert!(wide - narrow >= 60, "narrow saves {}px", wide - narrow);

    episode_row::set_narrow(false);
    settle();
    assert!(find("row.play").is_drawable());
    assert_eq!(cover.height(), 64);
    assert_eq!(minimum_width(&row), wide);
    window.destroy();
}

/// Titles take two lines in any window unless the progress bar needs the
/// room; subtitles leave out the show, which the spoken name keeps where
/// the cover shows it.
fn rows_word_their_titles_and_subtitles() {
    let labels = |row: &EpisodeRow| {
        let mut found = Vec::new();
        let mut stack = vec![row.clone().upcast::<gtk::Widget>()];
        while let Some(widget) = stack.pop() {
            if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                found.push(label.clone());
            }
            let mut child = widget.first_child();
            while let Some(c) = child {
                child = c.next_sibling();
                stack.push(c);
            }
        }
        found
    };
    let title_lines = |row: &EpisodeRow| {
        labels(row)
            .iter()
            .find(|l| l.has_css_class("heading"))
            .map(gtk::Label::lines)
    };
    let row = EpisodeRow::new(&setup());
    let mut item = partly_played();
    row.bind(&item);
    assert_eq!(
        title_lines(&row),
        Some(1),
        "the progress bar needs the room"
    );
    item.episode.state.position = Duration::ZERO;
    row.bind(&item);
    assert_eq!(title_lines(&row), Some(2), "two lines in a wide window too");

    assert!(
        labels(&row)
            .iter()
            .all(|l| !l.label().contains("A show with")),
        "no show in the subtitle"
    );
    assert!(row.accessible_name().contains("A show with a long name"));
    let page_row = EpisodeRow::new(&RowSetup {
        show_identity: false,
        ..setup()
    });
    page_row.bind(&item);
    assert!(!page_row.accessible_name().contains("A show with"));
}

/// Ctrl+A applies where a page offers selection and in selection mode,
/// where the header holding Select is hidden; the other shortcuts only in
/// selection mode.
fn selection_shortcuts_work_in_selection_mode() {
    use crate::ui::selection_bar::{select_button, takes_shortcut};

    let selection = Selection::default();
    let select = select_button(&selection);
    let header = adw::HeaderBar::new();
    header.pack_end(&select);
    let window = window(&header);
    assert!(takes_shortcut(&select, &selection, 0));
    assert!(!takes_shortcut(&select, &selection, 1));
    assert!(!takes_shortcut(&select, &selection, 2));

    selection.set_active(true);
    header.set_visible(false);
    for action in 0..3 {
        assert!(takes_shortcut(&select, &selection, action), "{action}");
    }

    // A page that offers no selection.
    selection.set_active(false);
    header.set_visible(true);
    select.set_visible(false);
    assert!(!takes_shortcut(&select, &selection, 0));
    window.destroy();
}

/// Beside the cover (256px) the text keeps to its height, a long
/// description losing lines and Read More showing; a short one shows whole;
/// stacked under the cover, four lines.
fn the_podcast_header_fits_its_description() {
    use crate::ui::show_hero::ShowHero;

    let long = "Destination Linux is a weekly show where Ryan, Jill, and guests share \
        their passion for Linux and Open Source. It is a show for all experience \
        levels, whether you are a beginner to Linux and Open Source or a long time \
        user, this is the podcast for you. It covers a wide range of topics from the \
        latest news, discussions on Linux and Open Source, Linux distributions, \
        Linux applications, security, privacy, AI, gaming on Linux, and unique \
        in-depth interviews with Linux developers and leaders and much more! \
        Website https://example.com Become a Patron https://example.com/patron";
    let fit = |description: &str, width: i32| {
        let hero = ShowHero::new();
        let mut info = beacon_core::model::ShowInfo {
            title: "A podcast".into(),
            ..Default::default()
        };
        info.description = Some(description.into());
        hero.update(&crate::ui::show_hero::HeroData {
            title: info.title,
            description: info.description,
            ..Default::default()
        });
        // A window of the width: the header takes all it is given.
        let window = gtk::Window::builder()
            .default_width(width)
            .default_height(600)
            .child(hero.widget())
            .build();
        window.present();
        let until = std::time::Instant::now() + Duration::from_millis(300);
        run_until(|| std::time::Instant::now() > until);
        let fitted = hero.fitted();
        window.destroy();
        fitted
    };

    let (lines, read_more, height) = fit(long, 760);
    assert!(read_more, "a long description is cut, with Read More");
    assert!(
        height <= 256,
        "beside the cover the text is {height}px tall"
    );
    assert!((1..=6).contains(&lines));

    let (_, read_more, _) = fit("A short description.", 760);
    assert!(!read_more, "a short description shows whole");

    let (lines, read_more, _) = fit(long, 400);
    assert_eq!(lines, 4, "stacked, four lines");
    assert!(read_more);
}

/// A favourite's row shows a star after its title and says "Favorite";
/// rebound to another episode, the star goes.
fn favorite_rows_show_a_star() {
    let row = EpisodeRow::new(&setup());
    let window = window(&row);
    let stars = |row: &EpisodeRow| {
        let mut found = 0;
        let mut stack = vec![row.clone().upcast::<gtk::Widget>()];
        while let Some(widget) = stack.pop() {
            if widget.downcast_ref::<gtk::Image>().is_some_and(|i| {
                i.is_visible() && i.icon_name().as_deref() == Some("starred-symbolic")
            }) && widget.is_drawable()
            {
                found += 1;
            }
            let mut child = widget.first_child();
            while let Some(c) = child {
                child = c.next_sibling();
                stack.push(c);
            }
        }
        found
    };
    let settle = || {
        let until = std::time::Instant::now() + Duration::from_millis(100);
        run_until(|| std::time::Instant::now() > until);
    };
    let mut item = partly_played();
    row.bind(&item);
    settle();
    assert_eq!(stars(&row), 0);
    assert!(!row.accessible_name().contains("Favorite"));

    item.episode.state.favorite = true;
    row.bind(&item);
    settle();
    assert_eq!(stars(&row), 1);
    assert!(
        row.accessible_name().contains("Favorite"),
        "{}",
        row.accessible_name()
    );

    item.episode.state.favorite = false;
    row.bind(&item);
    settle();
    assert_eq!(stars(&row), 0);
    window.destroy();
}

/// In the Queue view, only the queued rows have a drag handle and Move Up
/// and Move Down, unavailable at the queue's ends. Dragged, a row stays
/// inside its card, the others make way, and letting go saves the order.
fn queued_rows_can_be_reordered() {
    use crate::ui::models::RowAction;
    use crate::ui::queue_data::QueueData;
    use crate::ui::queue_view::{QueueView, QueueViewInput, QueueViewOutput};
    use crate::ui::widgets::queue_card::QueueCard;

    let asked = Rc::new(std::cell::RefCell::new(Vec::new()));
    let record = asked.clone();
    let view = QueueView::new(Selection::default());
    view.connect_output(move |QueueViewOutput::Action(item, action)| {
        record.borrow_mut().push((item.episode.id.0, action));
    });
    let window = window(&view);
    view.send(QueueViewInput::Set(Box::new(QueueData {
        now_playing: Some(episode(1, None)),
        queued: vec![episode(2, None), episode(3, None), episode(4, None)],
        from_source: vec![episode(5, None)],
        source_name: Some("New".into()),
        next: None,
    })));
    let settle = || {
        let until = std::time::Instant::now() + Duration::from_millis(300);
        run_until(|| std::time::Instant::now() > until);
    };
    settle();

    let mut rows = Vec::new();
    let mut stack = vec![view.clone().upcast::<gtk::Widget>()];
    while let Some(widget) = stack.pop() {
        if let Some(row) = widget.downcast_ref::<EpisodeRow>() {
            if row.is_mapped() {
                rows.push(row.clone());
            }
            continue;
        }
        let mut child = widget.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            stack.push(c);
        }
    }
    let row = |id: i64| {
        rows.iter()
            .find(|row| row.accessible_name().starts_with(&format!("Episode {id},")))
            .unwrap_or_else(|| panic!("episode {id} has a row"))
    };
    for (id, queued) in [(1, false), (2, true), (3, true), (4, true), (5, false)] {
        assert_eq!(
            row(id).handle().is_some_and(|h| h.is_drawable()),
            queued,
            "episode {id}'s handle"
        );
    }

    // The first can't go up nor the last down; the rest ask.
    for (id, action) in [
        (2, "row.move-up"),
        (2, "row.move-down"),
        (4, "row.move-up"),
        (4, "row.move-down"),
    ] {
        let _ = row(id).activate_action(action, None);
    }
    let _ = row(1).activate_action("row.move-down", None);
    assert_eq!(
        *asked.borrow(),
        [(2, RowAction::MoveDown), (4, RowAction::MoveUp)]
    );
    asked.borrow_mut().clear();

    let card = first::<QueueCard>(view.upcast_ref()).expect("the queue's card");
    let ids = |card: &QueueCard| -> Vec<i64> {
        card.rows()
            .iter()
            .filter_map(|r| r.item().map(|i| i.episode.id.0))
            .collect()
    };
    // The handle's column grabs the row; the rest of the row does not.
    let handle = card.rows()[1]
        .handle()
        .and_then(|h| h.compute_bounds(&card))
        .expect("a queued row's handle");
    let middle = |b: &gtk::graphene::Rect| {
        (
            f64::from(b.x() + b.width() / 2.0),
            f64::from(b.y() + b.height() / 2.0),
        )
    };
    let (hx, hy) = middle(&handle);
    assert_eq!(card.handle_at(hx, hy), Some(1));
    assert_eq!(
        card.handle_at(hx, hy + 20.0),
        Some(1),
        "anywhere in its column"
    );
    assert_eq!(card.handle_at(hx + 200.0, hy), None, "the row's text");
    let height = f64::from(card.rows()[0].height());
    // Up from the first, it stays in the card; down past the end, too.
    assert_eq!(card.drag_by(0, -50.0), 0.0);
    let bottom = card.drag_by(0, 1000.0);
    assert!(
        (bottom - 2.0 * height).abs() < 2.0,
        "{bottom} for rows of {height}"
    );
    // Over the last, the two below have made way.
    settle();
    let second = card.rows()[1].compute_bounds(&card).unwrap().y();
    assert!(
        f64::from(second).abs() < 2.0,
        "the second row slid up to the top, at {second}"
    );
    card.let_go();
    assert_eq!(ids(&card), [3, 4, 2], "in the new order at once");
    assert_eq!(*asked.borrow(), [(4, RowAction::MoveHere(EpisodeId(2)))]);
    window.destroy();
}

/// The visible progress bars in `widget`, with their fractions.
fn bars(widget: &gtk::Widget) -> Vec<f64> {
    let mut found = Vec::new();
    let mut stack = vec![widget.clone()];
    while let Some(widget) = stack.pop() {
        if let Some(bar) = widget.downcast_ref::<gtk::ProgressBar>()
            && bar.is_drawable()
        {
            found.push(bar.fraction());
        }
        let mut child = widget.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            stack.push(c);
        }
    }
    found
}

/// A row shows its episode's download under way in place of its playback
/// progress, and the playback progress again once it ends.
fn downloads_show_on_rows() {
    use std::collections::HashMap;

    use crate::ui::downloads;

    let row = EpisodeRow::new(&setup());
    let window = window(&row);
    let settle = || {
        let until = std::time::Instant::now() + Duration::from_millis(100);
        run_until(|| std::time::Instant::now() > until);
    };
    let item = partly_played();
    row.bind(&item);
    settle();
    let played = bars(row.upcast_ref());
    assert_eq!(played.len(), 1, "the playback progress");

    // Labels showing in the row, for the download's text.
    let labels = |row: &EpisodeRow| {
        let mut found = Vec::new();
        let mut stack = vec![row.clone().upcast::<gtk::Widget>()];
        while let Some(widget) = stack.pop() {
            if let Some(label) = widget.downcast_ref::<gtk::Label>()
                && label.is_drawable()
            {
                found.push(label.label().to_string());
            }
            let mut child = widget.first_child();
            while let Some(c) = child {
                child = c.next_sibling();
                stack.push(c);
            }
        }
        found
    };
    downloads::set(HashMap::from([(
        item.episode.id,
        downloads::Shown {
            received: 12_000_000,
            total: Some(30_000_000),
        },
    )]));
    settle();
    assert_eq!(bars(row.upcast_ref()), [0.4], "the download, alone");
    assert!(
        labels(&row).contains(&"12.0\u{a0}MB of 30.0\u{a0}MB downloaded".to_owned()),
        "{:?}",
        labels(&row)
    );
    assert!(
        crate::ui::episode_actions::items(
            crate::ui::episode_actions::menu(&item, "row").upcast_ref()
        )
        .iter()
        .any(|(label, _)| label == "Cancel _Download")
    );

    downloads::set(HashMap::new());
    settle();
    assert_eq!(bars(row.upcast_ref()), played);
    assert!(!labels(&row).iter().any(|l| l.contains("downloaded")));
    window.destroy();
}

/// A partly played episode of unknown length has no progress bar, which
/// could only stay empty (found in the M5 review).
fn progress_needs_a_length() {
    let row = EpisodeRow::new(&setup());
    let window = window(&row);
    let mut item = partly_played();
    item.episode.info.duration = None;
    row.bind(&item);
    let until = std::time::Instant::now() + Duration::from_millis(100);
    run_until(|| std::time::Instant::now() > until);
    assert!(
        bars(row.upcast_ref()).is_empty(),
        "{:?}",
        bars(row.upcast_ref())
    );
    window.destroy();
}

/// Preferences' rows show and change their settings, and the rows under
/// Delete Old Downloads are unavailable while it is off. Settings from
/// the schema in `data/`, compiled for the test, in memory.
fn preferences_follow_settings() {
    use adw::prelude::{AdwDialogExt, ComboRowExt, PreferencesRowExt};

    let schemas = std::env::temp_dir().join(format!("beacon-schemas-{}", std::process::id()));
    std::fs::create_dir_all(&schemas).unwrap();
    let compiled = std::process::Command::new("glib-compile-schemas")
        .arg(format!("--targetdir={}", schemas.display()))
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data"))
        .status()
        .is_ok_and(|status| status.success());
    if !compiled {
        return;
    }
    let source = gio::SettingsSchemaSource::from_directory(&schemas, None, false).unwrap();
    let schema = source.lookup(crate::config::APP_ID, false).unwrap();
    let backend = gio::memory_settings_backend_new();
    let settings = gio::Settings::new_full(&schema, Some(&backend), None);

    let sync = crate::ui::dialogs::sync_page::SyncPage::default();
    let dialog = crate::ui::dialogs::preferences::dialog(&settings, &sync);
    let host = window(&gtk::Box::new(gtk::Orientation::Vertical, 0));
    dialog.present(Some(&host));
    let until = std::time::Instant::now() + Duration::from_millis(200);
    run_until(|| std::time::Instant::now() > until);
    let switches: Vec<adw::SwitchRow> = {
        let mut found = Vec::new();
        let mut stack = vec![dialog.clone().upcast::<gtk::Widget>()];
        while let Some(widget) = stack.pop() {
            // The Sync page's switch is tested on its own.
            if widget.is::<crate::ui::dialogs::sync_page::SyncPage>() {
                continue;
            }
            if let Some(row) = widget.downcast_ref::<adw::SwitchRow>() {
                found.push(row.clone());
            }
            let mut child = widget.first_child();
            while let Some(c) = child {
                child = c.next_sibling();
                stack.push(c);
            }
        }
        found.sort_by_key(|row| row.title().len());
        found
    };
    let [enabled, only_played] = &switches[..] else {
        panic!("two switches, found {}", switches.len());
    };
    let after = first::<adw::SpinRow>(dialog.upcast_ref()).expect("Delete After");

    assert!(!enabled.is_active(), "off by default");
    assert!(only_played.is_active(), "on by default");
    assert_eq!(after.value(), 7.0);
    assert!(!after.is_sensitive() && !only_played.is_sensitive());

    settings.set_boolean("delete-old-downloads", true).unwrap();
    assert!(enabled.is_active());
    assert!(after.is_sensitive() && only_played.is_sensitive());
    after.set_value(30.0);
    assert_eq!(settings.uint("delete-after-days"), 30);
    only_played.set_active(false);
    assert!(!settings.boolean("delete-only-played"));
    let policy = crate::app::downloads::cleanup_policy(&settings);
    assert!(policy.enabled && policy.after_days == 30 && !policy.only_played);

    // The directory's country: Automatic first, then by name; choosing one
    // keeps its code.
    let country = first::<adw::ComboRow>(dialog.upcast_ref()).expect("Country or Region");
    assert_eq!(country.selected(), 0, "automatic by default");
    let countries = crate::ui::dialogs::preferences::countries();
    assert!(countries[0].0.is_empty() && countries[0].1.starts_with("Automatic ("));
    let germany = countries
        .iter()
        .position(|(code, _)| code == "DE")
        .expect("Germany has a directory");
    country.set_selected(germany as u32);
    assert_eq!(settings.string("directory-country"), "DE");
    country.set_selected(0);
    assert_eq!(settings.string("directory-country"), "");
    let _ = std::fs::remove_dir_all(&schemas);
    dialog.force_close();
    host.destroy();
}

/// Where the seek bar's chapter ticks are, as fractions of its width.
fn ticks(seek: &gtk::Scale) -> Vec<f64> {
    fn marks(widget: &gtk::Widget, found: &mut Vec<gtk::Widget>) {
        let mut child = widget.first_child();
        while let Some(c) = child {
            if c.css_name() == "mark" {
                found.push(c.clone());
            }
            marks(&c, found);
            child = c.next_sibling();
        }
    }
    let mut found = Vec::new();
    marks(seek.upcast_ref(), &mut found);
    let width = f64::from(seek.width());
    let mut ticks: Vec<f64> = found
        .iter()
        .filter_map(|mark| mark.compute_bounds(seek))
        .map(|b| f64::from(b.x() + b.width() / 2.0) / width)
        .collect();
    ticks.sort_by(f64::total_cmp);
    ticks
}

/// How far the seek bar's fill reaches, as a share of its trough.
fn fill(seek: &gtk::Scale) -> f64 {
    fn find(widget: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
        let mut child = widget.first_child();
        while let Some(c) = child {
            if c.css_name() == name {
                return Some(c);
            }
            if let Some(found) = find(&c, name) {
                return Some(found);
            }
            child = c.next_sibling();
        }
        None
    }
    let trough = find(seek.upcast_ref(), "trough").expect("a trough");
    let Some(fill) = find(&trough, "fill") else {
        return 0.0;
    };
    f64::from(fill.width()) / f64::from(trough.width())
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
    let bar = PlayerBar::default();
    bar.set_sheet(true);
    for (part, needed) in [
        ("slim bar", minimum_width(bar.widgets().mini())),
        ("player sheet", minimum_width(&bar)),
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

/// Not a check: how long a frame's work takes while the width changes, for
/// a real show's podcast page and for the details pane showing its longest
/// episode description. Feeds and covers come from `BEACON_PROBE_FEEDS`
/// (`NAME.xml` and `NAME.xml.cover`). Run alone, on a private display:
/// `BEACON_PROBE_FEEDS=dir xvfb-run cargo test -p beacon-gtk resize_cost -- --ignored --nocapture`.
#[test]
#[ignore = "a timing probe, not a check"]
fn resize_cost() {
    use crate::ui::details_pane::{DetailsPane, DetailsPaneInput};

    let Some(dir) = std::env::var_os("BEACON_PROBE_FEEDS") else {
        return;
    };
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
    let dir = std::path::PathBuf::from(dir);
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .expect("the probe folder")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".xml"))
        .collect();
    names.sort();
    for name in names {
        let feed = beacon_core::services::feeds::parse_feed(
            &std::fs::read(dir.join(&name)).expect("the feed"),
        )
        .expect("a feed");
        let items: Vec<EpisodeItem> = feed
            .episodes
            .into_iter()
            .enumerate()
            .map(|(i, parsed)| EpisodeItem {
                episode: Episode {
                    id: EpisodeId(i as i64 + 1),
                    show_id: ShowId(1),
                    info: parsed.info,
                    state: EpisodeState::default(),
                    added_at: Utc::now(),
                },
                show_title: feed.show.title.clone(),
                cover: None,
                in_queue: false,
            })
            .collect();
        let longest = items
            .iter()
            .max_by_key(|i| i.episode.info.description.as_ref().map_or(0, String::len))
            .cloned()
            .expect("episodes");
        let cover = dir.join(format!("{name}.cover"));
        let data = ShowPageData {
            show: beacon_core::model::Show {
                id: ShowId(1),
                feed_url: "https://example.com/feed.xml".parse().expect("a URL"),
                info: feed.show.clone(),
                cover_path: cover.exists().then_some(cover),
                subscribed_at: Utc::now(),
                last_refreshed_at: None,
            },
            prefs: Default::default(),
            seasons: None,
            episodes: items,
            all_played: false,
            to_download: Vec::new(),
            query: None,
        };
        let page = ShowPage::new(&gtk::ToggleButton::new());
        page.send(ShowPageInput::Set(Box::new(data)));
        let nav = adw::NavigationView::new();
        nav.push(&page);
        probe(
            &format!("{name} podcast page"),
            nav.upcast_ref(),
            360..=1200,
        );
        let pane = DetailsPane::default();
        pane.send(DetailsPaneInput::Show(Box::new(longest)));
        probe(
            &format!("{name} details pane"),
            pane.upcast_ref(),
            280..=600,
        );
    }
}

/// Lay `widget` out at every width in `widths`, narrowing then widening,
/// and print how long each frame's work took.
fn probe(name: &str, widget: &gtk::Widget, widths: std::ops::RangeInclusive<i32>) {
    let holder = gtk::Box::builder().halign(gtk::Align::Start).build();
    holder.append(widget);
    widget.set_size_request(*widths.end(), 700);
    let window = gtk::Window::builder()
        .default_width(1300)
        .default_height(760)
        .child(&holder)
        .build();
    window.present();
    let settle = |ms| {
        let until = std::time::Instant::now() + Duration::from_millis(ms);
        run_until(|| std::time::Instant::now() > until);
    };
    settle(800);
    let clock = window
        .frame_clock()
        .expect("a mapped window has a frame clock");
    let started = Rc::new(std::cell::Cell::new(None::<std::time::Instant>));
    let work = Rc::new(std::cell::RefCell::new(Vec::new()));
    let mark = started.clone();
    let before_id = clock.connect_before_paint(move |_| mark.set(Some(std::time::Instant::now())));
    let (mark, record) = (started.clone(), work.clone());
    let after_id = clock.connect_after_paint(move |_| {
        if let Some(start) = mark.take() {
            record
                .borrow_mut()
                .push(start.elapsed().as_secs_f64() * 1000.0);
        }
    });
    let (low, high) = (*widths.start(), *widths.end());
    for width in (low..=high).rev().step_by(8).chain((low..=high).step_by(8)) {
        let before = work.borrow().len();
        widget.set_size_request(width, 700);
        let until = std::time::Instant::now() + Duration::from_millis(300);
        run_until(|| work.borrow().len() > before || std::time::Instant::now() > until);
        if let Some(ms) = work.borrow().get(before).filter(|ms| **ms > 20.0) {
            eprintln!("SLOW {name} at {width}px: {ms:.1}ms");
        }
    }
    clock.disconnect(before_id);
    clock.disconnect(after_id);
    let mut frames = work.borrow().clone();
    frames.sort_by(f64::total_cmp);
    let mean = frames.iter().sum::<f64>() / frames.len().max(1) as f64;
    eprintln!(
        "PROBE {name}: {} frames, mean {mean:.1}ms, median {:.1}ms, p90 {:.1}ms, max {:.1}ms",
        frames.len(),
        frames[frames.len() / 2],
        frames[frames.len() * 9 / 10],
        frames[frames.len() - 1]
    );
    window.destroy();
    settle(200);
}
