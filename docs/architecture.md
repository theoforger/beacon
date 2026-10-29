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
    specs/                  feature specs (OPML, Add view and iTunes, auto-delete)
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

- `beacon-core`: empty at M0.
- `beacon-gtk`: the app (binary `beacon`). Depends on `beacon-core`, never the reverse.

### M0

- `main.rs`: logging, gettext, GResource registration, `AdwApplication`, `app.quit` (Ctrl+Q), `window.close` (Ctrl+W).
- `app.rs`: the `App` component. An `AdwApplicationWindow` holding an `AdwNavigationSplitView` (sidebar and content, each an `AdwToolbarView` with its own `AdwHeaderBar`). An `AdwBreakpoint` at 760sp collapses the split view. The window size is saved in GSettings.
- `ui/dialogs/about.rs`: `AdwAboutDialog`.
- `data/shortcuts-dialog.ui`: loaded automatically by `AdwApplication`, which also provides `app.shortcuts` (Ctrl+?).
- `data/style.css`: loaded automatically from the resource base path `/com/theoforger/Beacon/`.
