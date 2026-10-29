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
      src/main.rs, app.rs   window, navigation history, breakpoints
      src/ui/               sidebar, content pages, details pane, player bar
      src/ui/widgets/       episode row, cover, category tile
      src/ui/dialogs/       preferences, about, shortcuts, opml import
      src/player/           GstPlay wrapper, MPRIS
  tests/fixtures/           sample feeds (seasons, chapters), OPML files
```

## Components and services

`App` owns the window and navigation history. `Sidebar`, `ContentPages`, `DetailsPane` and `PlayerBar` are relm4 components. Services run as relm4 workers or commands: `Store` (SQLite), `FeedService`, `PlayerService` (wraps GstPlay and publishes state), `DownloadService` and `OpmlService`. Views never talk to the network or database directly; they send messages and render state. Use `FactoryVecDeque` only for short lists such as the queue and chapters.

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
| Episode lists | `GtkListView` over a `gio::ListStore` | Hundreds of rows; the HIG limits boxed lists to small ones |
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

- `navigation.rs`: `Location` (the sections: Shows, New, Queue, Favorites, Downloaded), `Subpage` (deeper pages: Add, and podcast pages from M3), `View` (a section or a subpage) and `Trail`, a breadcrumb: a section plus the deeper pages opened from it. Back returns to the parent; switching sections clears the deeper pages; Add always opens under Shows. No GTK; unit-tested.
- `app.rs`: an outer `AdwToolbarView` holds the player bar as its bottom bar, so it spans the full width. Its content is an `AdwOverlaySplitView` (details pane at the end) around an `AdwNavigationSplitView` (sidebar and content). One `AdwBreakpoint` at 760sp collapses both, makes the details pane full width with its own Back, hides the details toggle, and switches the sidebars to page mode. `App` keeps the trail and keeps the sidebar selection, content page title and Back in step with it.
- `ui/sidebar.rs`: header with Add Podcast and the main menu (Refresh Feeds, Import, Export; Preferences, Keyboard Shortcuts, About), and the library `AdwSidebar` with count badges.
- `ui/content.rs`: an `AdwNavigationView`. The root page holds the section: header (Refresh Feeds in Shows and New, Search toggle, details toggle), a `GtkSearchBar` (toggle, Ctrl+F, type to search) and a `GtkStack` with one empty state per section. Deeper pages (Add now, podcast pages in M3) are pushed on top with their own header, which gets Back automatically; each has a details toggle that follows the root page's. The view's `popped` signal tells `App` when the user went back.
- `ui/details_pane.rs`, `ui/player_bar.rs`: placeholders for M4.
- Actions: `win.about`, `win.search` (Ctrl+F), `win.back` (Alt+Left, Back key). `win.refresh`, `win.import`, `win.export` and `win.preferences` are referenced but not registered yet, which leaves their buttons and menu items insensitive until they land.

