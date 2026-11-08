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
    the_narrow_layout_fits_the_narrowest_window();
    narrow_headers_fit_three_window_buttons();
    the_player_bar_lists_chapters();
    the_details_pane_offers_the_row_menus_actions();
    favorite_rows_show_a_star();
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
    bar.emit(PlayerBarInput::Chapters(EpisodeId(8), chapters.clone()));
    settle();
    assert!(!button.is_sensitive());

    // The stored chapters come before the player knows the length.
    bar.emit(PlayerBarInput::Chapters(EpisodeId(7), chapters));
    settle();
    bar.emit(PlayerBarInput::Duration(Duration::from_secs(3600)));
    bar.emit(PlayerBarInput::Position(Duration::from_secs(15 * 60)));
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
    let content = Content::builder().launch(()).detach();
    // In a collapsed split view, as in the app, so Back to the sidebar shows.
    let split = adw::NavigationSplitView::builder()
        .sidebar(&adw::NavigationPage::new(&gtk::Label::new(None), "Beacon"))
        .content(&adw::NavigationPage::new(content.widget(), "New"))
        .collapsed(true)
        .show_content(true)
        .build();
    let window = window(&split);
    content.emit(ContentInput::ShowSection(Location::New));
    let until = std::time::Instant::now() + Duration::from_millis(100);
    run_until(|| std::time::Instant::now() > until);
    // What the narrow breakpoint hides.
    content.widgets().details_toggle().set_visible(false);
    content.widgets().refresh_holder().set_visible(false);
    // The page's breakpoint bin reports less and clips what it holds, so
    // measure the page inside it.
    let page = first::<adw::ToolbarView>(content.widget().upcast_ref())
        .expect("New's page has a toolbar view");
    let needed = minimum_width(&page);
    window.destroy();
    settings.set_gtk_decoration_layout(layout.as_deref());
    assert!(needed <= NARROWEST, "New's narrow page needs {needed}px");
}

/// The details pane's More Options holds the episode's actions as a row's
/// menu words them, and choosing one asks the app for it.
fn the_details_pane_offers_the_row_menus_actions() {
    use crate::ui::details_pane::{DetailsPane, DetailsPaneInput, DetailsPaneOutput};
    use crate::ui::episode_actions::items;
    use crate::ui::models::RowAction;

    let asked = Rc::new(std::cell::RefCell::new(Vec::new()));
    let record = asked.clone();
    let pane = DetailsPane::builder()
        .launch(())
        .connect_receiver(move |_, output| {
            if let DetailsPaneOutput::Action(item, action) = output {
                record.borrow_mut().push((item.episode.id, action));
            }
        });
    let window = window(pane.widget());
    let more = pane.widgets().more().clone();
    let settle = || {
        let until = std::time::Instant::now() + Duration::from_millis(100);
        run_until(|| std::time::Instant::now() > until);
    };
    settle();
    assert!(!more.is_mapped(), "no episode, no actions");

    let item = partly_played();
    let id = item.episode.id;
    pane.emit(DetailsPaneInput::Show(Box::new(item)));
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

    pane.emit(DetailsPaneInput::Clear);
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
        hero.update(&ShowPageData {
            show: beacon_core::model::Show {
                id: ShowId(1),
                feed_url: "https://example.com/feed.xml".parse().expect("a URL"),
                info,
                cover_path: None,
                subscribed_at: Utc::now(),
                last_refreshed_at: None,
            },
            prefs: Default::default(),
            seasons: None,
            episodes: Vec::new(),
            all_played: false,
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
        };
        let page = ShowPage::builder()
            .launch(gtk::ToggleButton::new())
            .detach();
        page.emit(ShowPageInput::Set(Box::new(data)));
        let nav = adw::NavigationView::new();
        nav.push(page.widget());
        probe(
            &format!("{name} podcast page"),
            nav.upcast_ref(),
            360..=1200,
        );
        let pane = DetailsPane::builder().launch(()).detach();
        pane.emit(DetailsPaneInput::Show(Box::new(longest)));
        probe(
            &format!("{name} details pane"),
            pane.widget().upcast_ref(),
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
