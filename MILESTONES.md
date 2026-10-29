# Beacon milestones

The checklist for building Beacon from `PLAN.md`. Tick an item only when it is merged and verified. Every milestone also has to pass the **exit gate**:

- [ ] HIG gate clean (`docs/hig-checklist.md`, HIG pages quoted, screenshots at 1440×900 and 360×640 in light and dark)
- [ ] Accessibility and translation checks pass (gettext, accessible labels and tooltips, keyboard walk-through, Orca, large text)
- [ ] Unit tests for all new logic, plus an integration test of each new feature's main scenario
- [ ] CI green; `docs/decisions.md` and `docs/architecture.md` up to date
- [ ] Human review of the running app

---

## M0 Setup

**Done when:** the app starts from `flatpak run` and CI is green.

### Deliverables

- [x] Toolchain checked: Rust 1.99 stable, `flatpak`, and the GNOME 50 Platform are installed
- [x] Toolchain completed (user-level, no sudo): `org.gnome.Sdk//50`, `org.freedesktop.Sdk.Extension.rust-stable//25.08`, flatpak-builder as `org.flatpak.Builder`, Meson through `uvx`, `cargo-deny` through `cargo install`
- [x] GNOME 50 runtime confirmed: GTK 4.22, libadwaita 1.9 (`AdwSidebar`, `AdwToggleGroup`, `AdwShortcutsDialog` all present), GStreamer 1.26 with GstPlay, SQLite 3.50
- [x] Cargo workspace: `beacon-core` (no GTK) and `beacon-gtk` (binary `beacon`); relm4 0.11 with `libadwaita` and `gnome_50`; `Cargo.lock` kept
- [x] Skeleton window: `AdwNavigationSplitView` with separate sidebar and content header bars, an `AdwBreakpoint` at 760sp, an `AdwStatusPage`, and a main menu with Keyboard Shortcuts and About
- [x] App ID `com.theoforger.Beacon` used in the desktop file, metainfo, GSettings schema and path, gresource prefix, icons, manifest and `GApplication`
- [x] Data files: desktop file, metainfo (passes `appstreamcli validate`), GSettings schema (window state), GResource, `style.css`, `shortcuts-dialog.ui`, placeholder app icon and symbolic icon
- [x] Meson: builds through Cargo, installs data, runs the validation tests and `cargo test`
- [x] Translations: `po/` with `POTFILES.in` and `LINGUAS`; every string wrapped; `beacon-pot` extracts strings from the Rust, UI, desktop, schema and metainfo files
- [x] Accessibility: the icon-only menu button has a tooltip and an accessible label
- [x] Flatpak manifest `build-aux/com.theoforger.Beacon.json` (GNOME 50, rust-stable, offline Cargo, minimal permissions)
- [x] `build-aux/cargo-sources.json` generated with `./build-aux/update-cargo-sources.sh` (local generator, no downloaded code)
- [x] Flatpak builds offline and runs: `flatpak run com.theoforger.Beacon` shows the split-view window with the status page
- [x] `cargo deny check` passes locally
- [x] Flathub manifest lint: clean except `appid-url-not-reachable` (https://theoforger.com timed out with 522; must be reachable before submitting)
- [x] CI workflow: rustfmt, clippy `-D warnings`, tests, desktop/metainfo/schema validation, cargo-deny, and a Flatpak build
- [ ] CI green on the first push (needs a GitHub remote; the homepage URL `github.com/theoforger/beacon` is a placeholder until then)
- [x] `LICENSE` (GPL-3.0-or-later) and SPDX headers in every source file
- [x] `CLAUDE.md` with commands, architecture, rules, requirements and the HIG gate
- [x] `docs/decisions.md`, `docs/architecture.md` and `docs/hig-checklist.md`
- [ ] Mockup exported into `docs/design/`
- [x] `/hig-review` skill written (`.claude/skills/hig-review/`) and `build-aux/screenshots.sh` for the four screenshots
- [x] Headless screenshots verified at 1440×900 and 360×640, light and dark
- [x] HIG gate for the M0 window: pass (see `docs/hig-reviews/m0.md`)
- [x] First commit (`8ea4a3e`)

## Watch: GNOME 51

- [ ] relm4 ships a `gnome_51` feature (checked weekly by `.github/workflows/relm4-gnome-51.yml`)
- [ ] Migrate: runtime and Sdk to 51, rust-stable to `//26.08`, relm4 feature, CI images, docs

## M1 Data and feeds

**Done when:** a feed URL becomes stored episodes, and a refresh adds only new ones.

### Deliverables

- [ ] Domain types in `beacon-core/src/model`: `Show`, `Episode`, `Season`, `QueueItem`, plus typed errors
- [ ] SQLite store (`rusqlite`): schema, migrations in `store/migrations/`, FTS5 table for search, in-memory mode for tests
- [ ] Played state, position, favourite, download and "new" flags in the schema from the start
- [ ] `FeedService`: fetch with `reqwest` (rustls, tokio), parse with `feed-rs`; confirm whether `feed-rs` exposes iTunes season, episode and chapters, and add a `roxmltree` pass if it does not
- [ ] Refresh: insert only new episodes (stable GUID matching, with a fallback for feeds without GUIDs), update show metadata
- [ ] Cover cache on disk, keyed by URL
- [ ] Fixture feeds in `tests/fixtures/`: with seasons, without seasons, with chapters, malformed
- [ ] Decide which real podcasts provide the fixtures (open question 3)
- [ ] Local HTTP test server serving the fixtures
- [ ] Flatpak: add `--share=network` and record it in `docs/decisions.md`
- [ ] `docs/decisions.md` entries for every new crate

### Tests

- [ ] Unit: feed parsing, season grouping, GUID matching
- [ ] Integration: subscribe from an RSS URL, episodes stored; refresh adds only new ones; migrations run on an empty and an existing database

## M2 Window shell

**Done when:** it matches the mockup at 1440px and 360px wide, in light and dark.

### Deliverables

- [ ] Three columns: `AdwNavigationSplitView` (sidebar and content) inside `AdwOverlaySplitView` (details pane)
- [ ] Sidebar header: Refresh, the app title and Main Menu; no group title
- [ ] `AdwSidebar` items Shows, New, Queue, Favorites and Downloaded, with Add at the bottom, plus count badges
- [ ] Content header: Back only with history, the view title, Search, and the details-pane toggle (darker background while on); no Forward
- [ ] Details pane header "Episode Details"; the close button moves between the pane header and the content header
- [ ] Narrow layout: sidebar as a page, content pushed on top, details pane as a full page with its own Back, starting closed, toggle hidden
- [ ] Navigation history in `App`
- [ ] Empty-state status pages for every view; the Shows placeholder says how to add a podcast and offers an Add Podcast button (deferred from the M0 HIG review)
- [ ] Main menu shell (Import and Export Subscriptions, Preferences, Keyboard Shortcuts, About), with items disabled until their features land
- [ ] Persistent bottom player bar placeholder across the full width
- [ ] Opens on Shows; follows the system dark mode and accent colour

### Tests

- [ ] Unit: navigation history and the `App` component's `update` (Back visibility, view switching)

## M3 Library views

**Done when:** real data can be browsed end to end.

### Deliverables

- [ ] Shows: a grid of covers with unplayed badges, and a final Add Podcast tile that opens Add
- [ ] New: episodes grouped by recency (Today, Yesterday, weekday, Last week, Earlier in the month), with an All / Unplayed `AdwToggleGroup` that is remembered in GSettings
- [ ] Podcast page: a hero (cover, category, title, author, description), Subscribe (accent) / Unsubscribe (neutral), and a More Options menu (Mark All as Played, Download All Episodes, Copy Feed Address)
- [ ] Podcast page: All / Unplayed remembered per show, a sort menu with radio items (Newest First, Oldest First), and collapsible seasons when the feed has season tags
- [ ] Episode row widget: cover, a two-line title, a subtitle (show, season and episode, date, length), the blue unplayed dot, the grey played check, a progress bar, and a flat play button with hover feedback
- [ ] Row menu: Show Details, queue, download, favourite and played toggles
- [ ] Episode lists use `GtkListView` over `gio::ListStore`
- [ ] Unplayed dot cleared on viewing the details, and not restored by Mark as Unplayed
- [ ] Marking played or unplayed never touches the queue, including Mark All as Played

### Tests

- [ ] Unit: recency grouping, sort, played and "new" state rules
- [ ] Integration: subscribe, see episodes, open a show page, use the All and Unplayed toggle

## M4 Playback

**Done when:** you can play, pause, close, reopen and resume at the same spot, and media keys work.

### Deliverables

- [ ] `PlayerService` wrapping `gstreamer-play`, publishing state; position saved periodically and on close
- [ ] Player bar: Favourite, Back 10 Seconds, a neutral round Play/Pause, Forward 30 Seconds, Next Episode, a seek `GtkScale` with elapsed and remaining time, speed, sleep timer and volume
- [ ] Settle the sleep timer lengths and volume behaviour (open question 2)
- [ ] The now-playing area opens the details pane
- [ ] Details pane: cover, show link, title, subtitle, actions (Play/Pause/Resume in accent; queue, favourite, download and played as icon-only state toggles), description, and chapters (current chapter grey and bold, click to seek)
- [ ] MPRIS: compare candidate crates (last release, downloads, open issues), choose one, record it in decisions, and wire up media keys
- [ ] Flatpak: add `--socket=pulseaudio` and the MPRIS D-Bus name (`--own-name=org.mpris.MediaPlayer2.com.theoforger.Beacon`)

### Tests

- [ ] Unit: position and resume rules, skip, speed, chapter lookup
- [ ] Integration (short local audio file, silent sink): play, pause, close, reopen and resume at the same position

## M5 Queue and lists

**Done when:** the queue behaves like the mockup's.

### Deliverables

- [ ] Queue view: Now Playing; Next in Queue with count and total time; Next from the source list
- [ ] Auto-continue: the manual queue first, then the rest of the source list
- [ ] Drag handles, plus Move Up and Move Down in the row menu; the playing episode shows no In Queue marker
- [ ] Favorites and Downloaded views with empty status pages
- [ ] `DownloadService`: manual downloads only, with resume; Download / Delete Download
- [ ] Preferences (`AdwPreferencesDialog`) with a Downloads group: Delete Old Downloads (switch, off by default), Delete After (spin row, days), and Only Delete Played Episodes (switch, on by default); the dependent rows are disabled while the master switch is off
- [ ] Cleanup at startup and daily; never deletes queued episodes; shows a toast when it removes something

### Tests

- [ ] Unit: queue order, auto-continue, auto-delete rules (with an injected clock)
- [ ] Integration: queue order and auto-continue; marking played leaves the queue alone; download with resume; auto-delete removes an old download

## M6 Search and Add

**Done when:** you can find a show and subscribe.

### Deliverables

- [ ] Check Apple's terms, attribution rules and rate limits; confirm `feedUrl` in a live response (open question 1)
- [ ] `GtkSearchBar` under the header, toggled from the header and by Ctrl+F; hidden in the Add view
- [ ] On a podcast page, a This Show / Everywhere toggle; This Show filters the page in place
- [ ] Results for matching shows and episodes (FTS5); with no results, a suggested Search Online button that opens Add with the query (not shown in the This Show scope)
- [ ] Add view: one rounded search field that also accepts an RSS URL (RSS Feed card with Add Feed); a "Search for Podcasts" status page when empty; results from the iTunes Search API; a show preview with Subscribe
- [ ] iTunes client: search on Enter or after a pause, cached results, `country` from the locale and as a setting, about 20 calls per minute
- [ ] Artwork-terms decision applied to the result covers
- [ ] Categories and charts stay out until there is a verified source
- [ ] Scheduled, non-blocking CI job that checks the live iTunes API assumptions

### Tests

- [ ] Unit: query building, URL detection, the rate limiter
- [ ] Integration (canned iTunes responses): search in This Show and Everywhere scope, the no-results page, Search Online, subscribe from iTunes results

## M7 OPML

**Done when:** a file from another app imports, and an export re-imports with no loss.

### Deliverables

- [ ] `OpmlService` using the `opml` crate (confirm that it parses and writes)
- [ ] Import from the main menu and by dropping a file on Add; `GtkFileDialog` (portal)
- [ ] Flatten nested outlines, accept `xmlUrl` in any letter case, take the name from `title` or `text`, skip outlines with no URL
- [ ] Normalise URLs and remove duplicates within the file and against existing subscriptions
- [ ] Subscribe in the background about four at a time, with a progress dialog, a summary toast and failures listed with Retry
- [ ] Bad XML shows an error status page
- [ ] Export: a save dialog defaulting to `podcasts.opml`; OPML 2.0 with a head and `outline type="rss"` entries; a warning about private feed tokens
- [ ] Fixture OPML files from at least three other podcast apps

### Tests

- [ ] Unit: OPML mapping, normalisation, de-duplication
- [ ] Integration: import a fixture file, export, re-import with no loss; bad XML handled

## M8 Polish

**Done when:** the app passes the review checklist in the Flathub guidelines.

### Deliverables

- [ ] Full accessibility and translation audit (Orca, keyboard, large text at 360px)
- [ ] Shortcuts: Ctrl+F, F9 (sidebar), F10 (main menu), all listed in `shortcuts-dialog.ui`
- [ ] Final app icon and symbolic icon (replacing the placeholders)
- [ ] Metainfo: screenshots, branding colours, release notes, real URLs
- [ ] Review sandbox permissions; portals only for file access
- [ ] Walk through the Flathub submission checklist and record the result

### Tests

- [ ] All scenario tests from M1 to M7 green in CI
