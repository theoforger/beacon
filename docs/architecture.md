# Architecture

The target design, then what is built so far. Rules for working in this structure are in `CLAUDE.md`.

## Target layout

A Cargo workspace with two crates. The core crate has no GTK dependency, which keeps the boundary between interface and logic honest and lets most tests run without a display.

```text
beacon/
  Cargo.toml                workspace
  meson.build               top-level build, i18n, installs data files
  CLAUDE.md
  MILESTONES.md
  build-aux/                Flatpak manifest, generated cargo sources, scripts
  data/                     .desktop, metainfo, icons, gschema, gresource, style.css
  po/                       POTFILES.in, LINGUAS, .pot, translations
  docs/
    design/                 exported mockup
    specs/                  feature specs (OPML, Add page and iTunes, auto-delete)
    hig-checklist.md
    hig-reviews/            HIG gate reports
    decisions.md            every crate and custom-widget choice
    architecture.md
    testing.md
  crates/
    beacon-core/            no GTK
      src/model/            domain types: Show, Episode, Season, QueueItem
      src/store/            SQLite, migrations/, queries
      src/services/         feeds, itunes, downloads, cleanup, opml
    beacon-gtk/             the app
      src/main.rs, app.rs   window, sections, breakpoints
      src/ui/               sidebar, content pages, details pane, player bar
      src/ui/widgets/       episode row, cover, category tile
      src/ui/dialogs/       preferences, about, shortcuts, opml import
      src/player/           GstPlay wrapper, MPRIS
  tests/fixtures/           sample feeds (seasons, chapters), OPML files
```

## Components and services

`App` owns the window and switches sections; deeper pages live in the content pane's `AdwNavigationView`. `Sidebar`, `ContentPages`, `DetailsPane` and `PlayerBar` are relm4 components. Services run as relm4 workers or commands: `Store` (SQLite), `FeedService`, `PlayerService` (wraps GstPlay and publishes state), `DownloadService` and `OpmlService`. Views never talk to the network or database directly; they send messages and render state. Use `FactoryVecDeque` only for short lists such as the queue and chapters.

## Widget map

Map every mockup element to a stock libadwaita widget first. A custom widget needs an entry in `docs/decisions.md`. All widgets below exist in the GNOME 50 runtime (libadwaita 1.9) and are bound by libadwaita-rs 0.9.

| Mockup element | Widget | Notes |
| --- | --- | --- |
| Window and three columns | `AdwApplicationWindow`, `AdwNavigationSplitView` for sidebar and content, `AdwOverlaySplitView` for the details pane | An `AdwBreakpoint` at 760sp collapses to pages |
| Header bars | `AdwHeaderBar` in an `AdwToolbarView` | Sidebar header and content header are separate bars |
| Sidebar | `AdwSidebar` | |
| All, Unplayed and the search scope | `AdwToggleGroup` | |
| Empty states | `AdwStatusPage` | Suggested pill button for Search Online |
| Search | `GtkSearchBar` with `GtkSearchEntry`, toggled from the header | Ctrl+F |
| Episode lists | A one-column `GtkGridView` (read as a list) over a `gio::ListStore` | Hundreds of rows; the HIG limits boxed lists to small ones; a list view keeps 200 rows, too many to restyle when the narrow layout moves the page |
| Row menu and sort menu | `GMenu` in a `GtkPopoverMenu` or `GtkMenuButton` | Radio items for sort |
| Cover art | `GtkPicture` with rounded CSS | Disk cache for downloaded covers |
| Category tiles | `GtkFlowBox` of styled buttons | The one place a custom style is expected; only if a category source exists (see `docs/specs/add-and-itunes.md`) |
| Player bar | A bottom bar in the content `AdwToolbarView` | Seek is a `GtkScale` |
| Main menu items | `AdwPreferencesDialog`, `AdwShortcutsDialog`, `AdwAboutDialog` | |
| Feedback | `AdwToastOverlay` | Import results, errors |

## Built so far

### Crates

- `beacon-core`: model, store and feed services (M1). Not yet used by the app; M2 and M3 connect it.
- `beacon-gtk`: the app (binary `beacon`). Depends on `beacon-core`, never the reverse.

### M0

- `main.rs`: logging, gettext, GResource registration, `AdwApplication`, `app.quit` (Ctrl+Q), `window.close` (Ctrl+W).
- `app.rs`: the `App` component. An `AdwApplicationWindow` holding an `AdwNavigationSplitView` (sidebar and content, each an `AdwToolbarView` with its own `AdwHeaderBar`). An `AdwBreakpoint` at 760sp collapses the split view. The window size is saved in GSettings.
- `ui/dialogs/about.rs`: `AdwAboutDialog`.
- `data/shortcuts-dialog.ui`: loaded automatically by `AdwApplication`, which also provides `app.shortcuts` (Ctrl+?).
- `data/style.css`: loaded automatically from the resource base path `/com/theoforger/Beacon/`.

### M1

- `model/`: `Show` and `Episode` (database identity plus the listener's state) wrap `ShowInfo` and `EpisodeInfo` (what the feed says). Also `Season`, `Enclosure`, `Chapter`, `QueueItem`, `ParsedFeed`.
- `store/`: `Store` over SQLite. Migrations in `store/migrations/` (rusqlite_migration). Tables `shows`, `episodes` (unique on show and GUID, with the listener's state columns), `chapters`, `queue`, plus FTS5 tables `shows_fts` and `episodes_fts` kept in sync by triggers. `sync_episodes` inserts new episodes and updates metadata of known ones in one transaction, never touching state.
- `services/feeds/`: `FeedClient` fetches; `parse_feed` reads RSS 2.0 with the iTunes, Podcasting 2.0 and Podlove Simple Chapters namespaces (`rss` crate). `time.rs` parses durations and loose `<pubDate>` values.
- `services/covers.rs`: `CoverCache` keyed by the SHA-256 of the image URL; writes through a `.part` file.
- `services/library.rs`: `Library` ties them together. `subscribe` and `refresh` fetch without holding the store lock, then write. Time comes from an injected `Clock`.
- Errors: `FeedError`, `StoreError`, `CoverError`, `LibraryError` (`error.rs`).

### M2

- `navigation.rs`: `Location`, the five sections, and which of them offer Refresh. Deeper pages are not modelled here: the content pane's `AdwNavigationView` owns them.
- `app.rs`: an outer `AdwToolbarView` holds the player bar as its bottom bar, so it spans the full width. Its content is an `AdwOverlaySplitView` (details pane at the end) around an `AdwNavigationSplitView` (sidebar and content). One `AdwBreakpoint` at 760sp collapses both, makes the details pane full width with its own Back, hides the details toggle, and switches the sidebars to page mode. `App` passes section changes from the sidebar to the content pane and the window title.
- `ui/sidebar.rs`: header with Add Podcast and the main menu (Refresh Feeds, Import, Export; Preferences, Keyboard Shortcuts, About), and the library `AdwSidebar` with count badges.
- `ui/content.rs`: an `AdwNavigationView`. The root page holds the section: header (Refresh Feeds in Shows and New, Search toggle, details toggle), a `GtkSearchBar` (toggle, Ctrl+F, type to search while the root page shows) and a `GtkStack` with one empty state per section. Deeper pages (Add now, podcast pages in M3) are pushed over the current section with their own header, which gets Back automatically; each has a details toggle that follows the root page's. Choosing a section replaces any deeper pages without a slide.
- `ui/details_pane.rs`, `ui/player_bar.rs`: placeholders for M4 (now `ui/details_pane/` and `ui/player_bar/`).
- Actions: `win.about`, `win.search` (Ctrl+F), `win.back` (Alt+Left, Back key). `win.refresh`, `win.import`, `win.export` and `win.preferences` are referenced but not registered yet, which leaves their buttons and menu items insensitive until they land.

### M3

- Core: `browse.rs` (recency groups, season groups, sorting), the "new" rule on `EpisodeState`, `EpisodeFilter`, `SortOrder`, `ShowPrefs`, `LibraryCounts`; `store/state.rs` (played, seen, favourites, queue, page preferences, unsubscribe) and `store/views.rs` (counts, new counts, recent episodes); migration 002; `text.rs`; `Library::open` and `refresh_all`.
- `app/`: `mod.rs` holds the window, its actions (`win.refresh` added, Ctrl+R and F5) and an `AdwToastOverlay`; `library.rs` reloads every view from the store after each change, applies row actions, runs subscribe and refresh as relm4 commands, and confirms unsubscribing with an `AdwAlertDialog`; `window.rs` holds the narrow breakpoint and window size.
- `ui/models.rs`: view models (`EpisodeItem`, `ShowCard`, `ShowPageData`) and their loaders; views never read the store.
- `ui/content.rs` hosts `ShowsGrid` (a `GtkGridView`), `NewView` (a heading and a card of episodes per recency group) and the deeper pages `ShowPage` (the hero, then a card per season under a `GtkExpander` heading, or one card) and `AddPage`. `ui/episode_list.rs` is each page's one recycling list (a one-column `GtkGridView`), holding the header, headings and episodes so that only the rows on screen exist, and changing only the entries that changed (2026-11-08; until then a `GtkListBox` per group, filled at idle priority).
- `ui/page_filters.rs`: the filter button (sort, played, downloaded, Reset Sort and Filters, the accent dot), which owns its `filter.*` actions; used by podcast pages and New.
- `ui/widgets/episode_row.rs` (its menu built when opened), `ui/covers.rs` (square covers, fixed-size or filling a grid column, art cropped to the square), `ui/widgets/smooth_texture.rs` (draws covers with a trilinear filter), `ui/format.rs` (subtitles, lengths, dates, headers), `ui/errors.rs` (a translated message per error variant).

### M4

- Core: `playback.rs` (resume, skips, speeds, saving, the sleep timer and fade, the current chapter); `Store::set_position` and `Store::finish`; `PlaybackError`.
- `player/`: `Player`, a `GstPlay` wrapper reporting `PlayerEvent`s (state, position, duration, end, error) on the main loop; `player/tests.rs` plays a local file to a silent sink.
- `app/playback.rs`: starts the player with the remembered volume and speed, plays a row's episode (or pauses and resumes it), saves the position while playing, on pause and on close, and marks finished episodes played.
- After review: `text::markup` (description HTML to Pango markup); the podcast page header in the mockup's layout, with More Options in the page's header bar; episode lists paged, rebinding only changed rows (list views since 2026-11-08).
- `ui/player_bar/`: the player bar (now playing, Chapters, transport, seek, speed, sleep timer, volume): `mod.rs` renders state and sends typed outputs, `build.rs` makes its widgets, `chapters.rs` the Chapters popover and the seek bar's chapter ticks (Favourite until 2026-11-08); `app/controls.rs` acts on them (seeking, speed and volume remembered, the sleep timer's fade, Next Episode, opening details). `ui/chapter_list.rs` makes chapter rows for the popover and the details pane. Rows show Pause for the playing episode through `episode_row::show_playing`.
- `ui/details_pane/`: `mod.rs` renders the episode, the play state and the current chapter, and sends typed outputs (an action, open the show, play from a chapter); `build.rs` makes its widgets. Core: `services/feeds/chapters.rs` reads JSON chapters files; `Library::chapters` fetches them once and keeps them.
- Selection mode: `ui/selection.rs` (a page's selected episodes, shared by its rows and bar), `ui/selection_bar.rs` (Select button, selection header, action bar, Ctrl+A and Escape), `ui/widgets/episode_row/selecting.rs` (check box, hover, secondary click, long press). Rows get a `RowSetup` (actions, show identity, selection). Core: `EpisodeChange` and `Store::change_episodes`.
- MPRIS: `player/mpris.rs` (the `mpris-server` player on the main loop, reporting `MediaControl`s), `app/media.rs` (keeps it told what plays and acts on what it asks, as the player bar's controls do). Core: `playback::seek_by` and `seek_to`.
- Narrow player (2026-11-07): `app/window.rs` builds an `AdwBottomSheet` around a toolbar view and the main view, and the narrow breakpoint moves only the player bar from the toolbar into the sheet (an `AdwMultiLayoutView` until 2026-11-08, which moved the whole main view); `ui/player_bar/compact.rs` holds the slim bar and the sheet's header, and `PlayerBarWidgets::set_sheet` lays the player out as the sheet's column.
- Sweep (2026-11-04): `app/library.rs` keeps background loading (`Parts`, `load`, `job`, `reload`, `loaded`); what the views ask for (row actions, opening and leaving shows, subscribing, refreshing) moved to `app/actions.rs`. `Library::refresh_all` fetches up to four feeds at once.
