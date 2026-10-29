# Beacon milestones

The checklist for building Beacon. Design details are in `docs/`. Tick an item only when it is merged and verified.

A milestone is finished when its deliverables are done and its own **exit gate** is ticked:

- **HIG:** gate clean (`docs/hig-checklist.md`, HIG pages quoted); full screenshot set at 1440×900 and 360×640 in light and dark; `docs/hig-reviews/pending-screenshots.md` empty
- **Accessibility and translation:** gettext, accessible labels and tooltips, keyboard walk-through, Orca, large text. For M0 to M3 the walk-throughs are deferred to the accessibility checkpoint after M3: log them in `docs/hig-reviews/pending-accessibility.md`
- **Tests:** unit tests for all new logic, plus an integration test of each new feature's main scenario
- **CI and docs:** CI green; `docs/decisions.md` and `docs/architecture.md` up to date
- **Human review** of the running app

---

## M0 Setup ✓

**Done when:** the app starts from `flatpak run` and CI is green.

**Finished 2026-10-28.**

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
- [x] CI green on the first push (github.com/theoforger/Beacon, run 40807922368 on `7a252f7`: all four jobs passed)
- [x] `LICENSE` (GPL-3.0-or-later) and SPDX headers in every source file
- [x] `CLAUDE.md` with commands, architecture, rules, requirements and the HIG gate
- [x] `docs/decisions.md`, `docs/architecture.md` and `docs/hig-checklist.md`
- [x] Mockup exported into `docs/design/` (bundle, decoded source, and renders at 1440×900 and 360×640)
- [x] `/hig-review` skill written (`.claude/skills/hig-review/`) and `build-aux/screenshots.sh` for the four screenshots
- [x] Headless screenshots verified at 1440×900 and 360×640, light and dark
- [x] HIG gate for the M0 window: pass (see `docs/hig-reviews/m0.md`)
- [x] Pending screenshot checks for M0 cleared (main menu, About and Shortcuts dialogs; results in `docs/hig-reviews/m0.md`)
- [x] First commit (`8ea4a3e`)

### Exit gate

- [x] HIG: gate clean, full screenshot set taken, pending list empty (`docs/hig-reviews/m0.md`)
- [x] Accessibility and translation: gettext, labels and tooltips done; Ctrl+Q, Ctrl+W, F10 and menu arrow keys verified headless; Orca, large text and Ctrl+? deferred to the checkpoint after M3 (logged in `pending-accessibility.md`)
- [x] Tests: no logic in M0; `cargo test` and the Meson validation tests run in CI
- [x] CI and docs: CI green; docs up to date
- [x] Human review of the running app (2026-10-28)

## M1 Data and feeds ✓

**Done when:** a feed URL becomes stored episodes, and a refresh adds only new ones.

**Finished 2026-10-28.**

### Deliverables

- [x] Domain types in `beacon-core/src/model`: `Show`, `Episode`, `Season`, `QueueItem`, plus typed errors
- [x] SQLite store (`rusqlite`): schema, migrations in `store/migrations/`, FTS5 table for search, in-memory mode for tests
- [x] Played state, position, favourite, download and "new" flags in the schema from the start ("new" is the `seen` flag behind the blue dot)
- [x] Feed service: fetch with `reqwest` (rustls, tokio), parse with `rss`. `feed-rs` was tried and dropped: it loses item-level iTunes tags and rejects `MM:SS` durations (see `docs/decisions.md`)
- [x] Refresh: insert only new episodes (stable GUID matching, with a fallback for feeds without GUIDs), update show metadata
- [x] Cover cache on disk, keyed by URL
- [x] Fixture feeds in `tests/fixtures/`: with seasons, without seasons, with chapters, malformed, plus no GUIDs and a web page (hand-written, fictional shows)
- [x] Real-podcast fixtures: trimmed excerpts of Hello From The Magic Tavern, Hey Riddle Riddle, Destination Linux and The Adventure Zone (chosen 2026-10-28), plus Destination Linux chapter JSON for M4. Season names and Podlove chapters are only in the hand-written fixtures
- [x] Local HTTP test server serving the fixtures (`httpmock`)
- [x] Flatpak: add `--share=network` and record it in `docs/decisions.md`
- [x] `docs/decisions.md` entries for every new crate

### Tests

- [x] Unit: feed parsing, durations and dates, GUID matching, store sync, FTS, chapters (season grouping for display moves to M3, where it is used)
- [x] Integration: subscribe from an RSS URL, episodes stored; refresh adds only new ones; migrations run on an empty and an existing database; bad feeds, duplicates and a missing cover

### Exit gate

- [x] HIG: no UI changes in M1; pending list still empty
- [x] Accessibility and translation: no user-visible strings or UI in M1 (`beacon-core` has none by design)
- [x] Tests: 15 unit tests, 9 integration tests (`tests/library.rs`), 5 real-feed tests (`tests/real_feeds.rs`), plus an opt-in check of full downloaded feeds
- [x] CI and docs: CI green (run 40813797153 on `dd97ff9`); docs up to date
- [x] Human review: not applicable (no UI in M1); the user approved the seen-on-subscribe behaviour

## M2 Window shell

**Done when:** it matches the mockup at 1440px and 360px wide, in light and dark.

### Deliverables

- [x] Three columns: `AdwNavigationSplitView` (sidebar and content) inside `AdwOverlaySplitView` (details pane)
- [x] Sidebar header: Refresh, the app title and Main Menu; no group title
- [x] `AdwSidebar` items Shows, New, Queue, Favorites and Downloaded, with Add at the bottom (a second one-item `AdwSidebar` in the bottom bar), plus count badges (shown once M3 feeds them)
- [x] Content header: Back only with history, the view title, Search, and the details-pane toggle (darker background while on); no Forward
- [x] Details pane header "Episode Details"; the close button moves between the pane header and the content header
- [x] Narrow layout: sidebar as a page, content pushed on top, details pane as a full page with its own Back, starting closed, toggle hidden
- [x] Navigation history in `App`
- [x] Empty-state status pages for every view; the Shows placeholder says how to add a podcast and offers an Add Podcast button (deferred from the M0 HIG review)
- [x] Main menu shell (Import and Export Subscriptions, Preferences, Keyboard Shortcuts, About), with items disabled until their features land
- [x] Persistent bottom player bar placeholder across the full width
- [x] Opens on Shows; follows the system dark mode and accent colour

### Tests

- [x] Unit: navigation history (`navigation.rs`, 5 tests). `App::update` only forwards to `History` and widgets, so its rules are tested there

### Exit gate

- [x] HIG: gate clean, full screenshot set taken, pending list empty (`docs/hig-reviews/m2.md`)
- [x] Translation and labels checked (all strings in gettext, new files in `POTFILES.in`, labels and tooltips on icon-only buttons); accessibility walk-throughs logged in `pending-accessibility.md`
- [x] Tests: unit tests for the navigation rules; no feature scenario yet (the views get data in M3)
- [ ] CI and docs: CI green; docs up to date
- [ ] Human review of the running app

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
- [ ] Carried from M2: sidebar count badges fed from the library (Shows with new episodes, new episodes, queue length), and the details pane opening as a narrow full page with its own Back, checked in screenshots

### Tests

- [ ] Unit: recency grouping, sort, played and "new" state rules
- [ ] Integration: subscribe, see episodes, open a show page, use the All and Unplayed toggle

### Exit gate

- [ ] HIG: gate clean, full screenshot set taken, pending list empty
- [ ] Translation and labels checked; accessibility walk-throughs logged in `pending-accessibility.md` (deferred to the checkpoint after M3)
- [ ] Tests: unit tests for new logic, integration test for each feature's main scenario
- [ ] CI and docs: CI green; docs up to date
- [ ] Human review of the running app

## Accessibility checkpoint (after M3)

**Done when:** every row of `docs/hig-reviews/pending-accessibility.md` is checked, fixed where needed and cleared. M4 starts after this.

- [ ] Orca walk-through of every view and dialog built in M0 to M3
- [ ] Keyboard-only walk-through: every control reachable, focus order sensible, shortcuts work
- [ ] States exposed to assistive technology (selected sidebar item, toggle groups, played and unplayed)
- [ ] Large text at 360px wide: no clipping or overlap
- [ ] Results recorded in `docs/hig-reviews/a11y-checkpoint.md`; fixes have their own commits
- [ ] `pending-accessibility.md` empty

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

### Exit gate

- [ ] HIG: gate clean, full screenshot set taken, pending list empty
- [ ] Accessibility and translation checks pass
- [ ] Tests: unit tests for new logic, integration test for each feature's main scenario
- [ ] CI and docs: CI green; docs up to date
- [ ] Human review of the running app

## M5 Queue and lists

**Done when:** the queue behaves like the mockup's.

**Spec:** `docs/specs/auto-delete.md`

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

### Exit gate

- [ ] HIG: gate clean, full screenshot set taken, pending list empty
- [ ] Accessibility and translation checks pass
- [ ] Tests: unit tests for new logic, integration test for each feature's main scenario
- [ ] CI and docs: CI green; docs up to date
- [ ] Human review of the running app

## M6 Search and Add

**Done when:** you can find a show and subscribe.

**Spec:** `docs/specs/add-and-itunes.md`

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

### Exit gate

- [ ] HIG: gate clean, full screenshot set taken, pending list empty
- [ ] Accessibility and translation checks pass
- [ ] Tests: unit tests for new logic, integration test for each feature's main scenario
- [ ] CI and docs: CI green; docs up to date
- [ ] Human review of the running app

## M7 OPML

**Done when:** a file from another app imports, and an export re-imports with no loss.

**Spec:** `docs/specs/opml.md`

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

### Exit gate

- [ ] HIG: gate clean, full screenshot set taken, pending list empty
- [ ] Accessibility and translation checks pass
- [ ] Tests: unit tests for new logic, integration test for each feature's main scenario
- [ ] CI and docs: CI green; docs up to date
- [ ] Human review of the running app

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

### Exit gate

- [ ] HIG: gate clean, full screenshot set taken, pending list empty
- [ ] Accessibility and translation checks pass
- [ ] Tests: unit tests for new logic, integration test for each feature's main scenario
- [ ] CI and docs: CI green; docs up to date
- [ ] Human review of the running app

---

## Watch: GNOME 51

- [ ] relm4 ships a `gnome_51` feature (checked weekly by `.github/workflows/relm4-gnome-51.yml`)
- [ ] Migrate: runtime and Sdk to 51, rust-stable to `//26.08`, relm4 feature, CI images, docs
