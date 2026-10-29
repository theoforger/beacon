# Beacon: Build Plan

Oct 29, 2026 · @Theo

Build the mockup as a Rust app on relm4 0.11 and libadwaita, packaged as a Flatpak. Set up the project first, then add features in small milestones, each ending with a HIG review and a headless screenshot.

## Requirements

This section is the contract for Claude Code. It records everything agreed in the design conversation.

### Build requirements

1. Start the plan with project setup.
2. Use Rust and the relm4 framework.
3. Prefer well-maintained, off-the-shelf Rust libraries over writing things from scratch.
4. After every UI change, check it against the GNOME Human Interface Guidelines (HIG).
5. Use libadwaita components wherever possible instead of building widgets.
6. Support OPML import and export.

### Decisions

- **Machine:** your Linux machine running CachyOS with GNOME 51. Build in a Flatpak against the GNOME 50 runtime, so the build does not depend on the host.
- **Target:** GNOME 50 for now.
- **Directory search:** iTunes Search API.
- **Sync:** none for now.
- **Downloads:** manual only, never automatic. An auto-delete feature removes old downloaded episodes, configurable in Settings.
- **Translation and accessibility:** both from day one.
- **Window sizes:** small desktop windows only, down to 360px wide. No Phosh-specific work.
- **Versions:** use the latest release of every crate and do not pin to a specific version. Only the GNOME 50 runtime is fixed.
- **Licence:** GPL-3.0-or-later. Put the licence text in `LICENSE` and an SPDX header in every source file.
- **Distribution:** Flathub is not the goal right now, but build to its requirements from the start so submitting later is a checklist, not a rewrite. That means a validated metainfo file and desktop file (checked in CI), the narrowest sandbox permissions that work, portals for file access, and cargo sources generated for an offline Flatpak build.
- **Name and app ID:** Beacon, with the app ID `com.theoforger.Beacon`. Use the ID everywhere it appears: the `.desktop` and metainfo file names, the GSettings schema ID and path (`/com/theoforger/Beacon/`), the gresource prefix, the icon names, the Flatpak manifest and `GApplication`. Flathub accepts a reverse-DNS ID only for a domain the developer controls, so keep `theoforger.com` under your control.

### Product requirements from the mockup

Link to the mockup: https://claude.ai/code/artifact/b9bbe38d-528e-453e-a46a-0b2393da4cc5

Read the design's files from that link. Match how it looks.

**Layout and navigation**

- Three columns: a left sidebar, a centre view, and an expandable right details pane. A persistent playback bar sits at the bottom across the full width.
- The sidebar runs from the top edge of the window and has its own header: Refresh on the left, the app title in the centre, Main Menu on the right. It has no group title.
- Sidebar items: Shows, New, Queue, Favorites, Downloaded, with Add at the bottom. Shows carries a count of podcasts that have new episodes. New shows the number of new episodes. Queue shows its length.
- The centre header has Back on the left (only when there is history to go back to), the view title in the centre, then Search and the details-pane toggle on the right. There is no Forward button. Search is hidden in the Add view.
- The details pane also runs from the top edge. Its header says Episode Details. The window close button sits in the pane header while the pane is open, and in the centre header when it is closed.
- On narrow windows the sidebar becomes a full page. Choosing an item pushes the content on top, and Back returns to the sidebar. The details pane becomes a full page with its own Back button, starts closed, and its header toggle is hidden.
- The app opens on Shows. Dark mode and the accent colour follow the system.

**Views**

- **Shows:** a grid of cover art with a badge for unplayed episodes. The last tile is Add Podcast, which opens the Add view.
- **New:** episodes from all subscribed shows grouped by recency (Today, Yesterday, weekday, Last week, Earlier in the month). An All / Unplayed toggle group sits above, and the choice is remembered.
- **Podcast page:** a hero with cover, category, title, author and description. Subscribe is the accent button, and becomes a neutral Unsubscribe once subscribed. A More Options menu holds Mark All as Played, Download All Episodes and Copy Feed Address. Below sit an All / Unplayed toggle group (remembered per show) and a sort menu with radio items, Newest First and Oldest First. Episodes group into collapsible seasons, with season names, when the feed carries season tags.
- **Queue:** three groups. Now Playing. Next in Queue, with count and total time, for episodes added by hand. Next from the list the playing episode came from. The behaviour copies a music app: manual queue first, then the rest of the list. Rows have a drag handle at the start, and Move Up and Move Down in the row menu.
- **Favorites** and **Downloaded:** episode lists with status pages when empty.
- **Add:** one search field that also accepts a pasted RSS feed URL (an RSS Feed card with Add Feed). Reference only: the real Add view is modeled on what the iTunes Search API returns (see Add view and iTunes), so the category tiles, Popular This Week and the card layout in the mockup are placeholders.
- **Search:** a header toggle (and Ctrl+F) opens a search bar under the header. On a podcast page a This Show / Everywhere toggle appears, and This Show filters that page in place. Otherwise results show matching shows and episodes. With no results, a suggested Search Online button opens the Add view with the query filled in. The button does not appear in This Show scope.
- **Empty states** are status pages with an icon, a title and one line of description.

**Episode rows**

- Cover, title (two lines at most), and a subtitle of show, season and episode, date and length. The subtitle does not show time left.
- Unplayed episodes have a blue dot. Viewing the details removes it, and marking an episode unplayed does not bring it back. Played episodes have a grey check after the title. Partly played episodes have a progress bar.
- The playing episode shows no In Queue marker.
- The row play button has no fill, blends into the row, and gives hover feedback. It shows pause while playing.
- The row menu holds Show Details, Move Up and Move Down (queue only), Add to or Remove From Queue, Download or Delete Download, Add to or Remove From Favorites, and Mark as Played or Unplayed.
- Marking an episode as played or unplayed never touches the queue. That includes Mark All as Played on a podcast page.

**Details pane**

- Cover, show link, title, subtitle, then actions: Play, Pause or Resume (accent coloured), queue, favourite, download and mark played. The four toggles show their state through the icon only, never through a colour or background change.
- Description, then chapters. The current chapter has a gray background and bold text, and choosing a chapter seeks to it.

**Player bar**

- Favourite (left of Back 10 Seconds), Back 10 Seconds, a neutral round Play and Pause, Forward 30 Seconds, Next Episode, a seek bar with elapsed and remaining time, playback speed, sleep timer and volume. The now-playing area opens the details pane.

**Look**

- Selectors use the toggle-group style: a rounded track with a lighter pill on the selected option. Search fields are fully rounded.
- Header toggles (Search, details pane) show a darker background while on.
- Pill-shaped buttons only for the suggested action, used sparingly.

## Project setup

Do this before any feature work. Step 0 ends with an empty window that builds and runs from a Flatpak, and the rules Claude Code must follow written down.

1. **Install the toolchain on your CachyOS machine.** `rustup` (stable), `flatpak` and `flatpak-builder` from the Arch repositories, the GNOME 50 Platform and Sdk (`org.gnome.Platform//50`, `org.gnome.Sdk//50`), and the Rust Sdk extension (`org.freedesktop.Sdk.Extension.rust-stable`). Your host runs GNOME 51, but build and test in the Flatpak so the result matches the GNOME 50 target.
2. **Confirm what GNOME 50 ships.** Check the libadwaita version inside the runtime before relying on the newest widgets (`AdwSidebar`, `AdwToggleGroup`, the shortcuts dialog).
3. **Generate the skeleton.** Relm4 maintains a project template with Meson, a Flatpak manifest, GSettings and a desktop file wired up. Check its README for the current command and let Claude Code run it. Not verified in this session.
4. **Use the latest dependencies.** `relm4` (latest release) with its `libadwaita` feature and `gnome_50`. Add every crate without a version pin, and take relm4, gtk4-rs, libadwaita-rs and gstreamer-rs together, because they must match each other (when checked, relm4 0.11.0 sat on gtk4 0.11 and libadwaita 0.9). The one ceiling is the GNOME 50 runtime: a crate release that needs a newer C library than the runtime ships will not build, so use the newest release that does. Commit Cargo.lock and update on a schedule.
5. **Lay out the repo** as described in the next section.
6. **Set up translations and accessibility now.** Add gettext to the build (a `po/` folder, `POTFILES.in`), wrap every user-visible string from the first line of UI code, and set accessible labels on every icon-only button.
7. **Put the mockup in the repo.** Export it into `docs/design/` so Claude Code can open the reference at any time. I can export it for you.
8. **Add CI early.** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo deny`, and a Flatpak build.
9. **Write `CLAUDE.md` before the first feature.** It holds the build and run commands, the architecture summary, the Requirements section above, the stack rules (prefer maintained crates, prefer libadwaita widgets, justify every exception in `docs/decisions.md`) and the HIG gate.
10. **Finish step 0 with a green check.** The app starts from `flatpak run`, shows an empty split-view window with a status page, and CI passes.

## Project structure

My recommendation is a Cargo workspace with two crates. The core crate has no GTK dependency, which keeps the boundary between interface and logic honest and lets most tests run without a display.

```text
beacon/
  Cargo.toml                workspace
  meson.build               top-level build, i18n, installs data files
  CLAUDE.md
  build-aux/                Flatpak manifest, generated cargo sources
  data/                     .desktop, metainfo, icons, gschema, gresource, style.css
  po/                       POTFILES.in, LINGUAS, .pot, translations
  docs/
    design/                 exported mockup
    hig-checklist.md
    decisions.md            every crate and custom-widget choice
    architecture.md
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

Rules:

1. **Dependencies point one way.** `beacon-gtk` depends on `beacon-core`, never the reverse. Core never imports GTK, GLib or relm4.
2. **No input or output in the UI layer.** Components send messages and render state. Network, database and files belong in services.
3. **One component per file,** kept under about 400 lines. Split before it grows.
4. **Typed messages** between components and services. No stringly-typed events.
5. **All CSS lives in `data/style.css`.** Use libadwaita's named colours and style classes, never hard-coded hex values, so dark mode and accent colours work.
6. **All user-visible strings go through gettext,** including tooltips, accessible labels and status pages.
7. **Every new crate or custom widget** gets a line in `docs/decisions.md` with the reason.
8. **Tests:** services and store tested in core with fixture feeds and a local HTTP server. Features covered by integration tests (see Test strategy), UI by the HIG gate screenshots.
9. **Errors** are typed in core and shown to people as toasts or status pages, never as panics.

## Stack

One maintained crate per job. The status column says what I confirmed in this session and what Claude Code must check at setup.

| Job | Choice | Status |
| --- | --- | --- |
| UI framework | [relm4](https://docs.rs/crate/relm4/latest) latest release (0.11.0 on gtk4 0.11 and libadwaita 0.9 when checked) | Confirmed on docs.rs |
| Libadwaita version flags | Relm4's `gnome_43` to `gnome_50` features, set to gnome\_50 for the target runtime | Confirmed that the features exist |
| Playback | `gstreamer-play` from gstreamer-rs (high-level player: seek, rate, position and state signals) | Crate exists in gstreamer-rs; use the latest release |
| Feed parsing | `feed-rs` latest release | Not confirmed that it exposes itunes season, episode and chapters. Test with real feeds, and add a small `roxmltree` pass for those tags if not |
| OPML | `opml` crate, latest release | Parse and write both listed on docs.rs; confirm at setup |
| HTTP | `reqwest` with rustls on tokio; relm4 commands run on tokio | Standard choice, not checked |
| Storage | SQLite through `rusqlite`, with FTS5 for search | My recommendation |
| Directory search | iTunes Search API (no key). Results include each show's feed URL, so subscribing is still plain RSS | Decided. Check Apple's terms and rate limits |
| Media keys and MPRIS | An MPRIS server crate | Not chosen. Claude Code should compare candidates on last release, downloads and open issues |
| Settings | GSettings through `gio` | Standard |
| Reference code | [GNOME Podcasts](https://github.com/alatiera/gnome-podcasts): Rust, GTK4, libadwaita | Read it for feed refresh and download edge cases; check its licence before reusing any code |

Before adding any crate, Claude Code records its last release date, download count, open-issue health and licence in `docs/decisions.md`.

## Architecture

Map every mockup element to a stock libadwaita widget first. A custom widget needs an entry in `docs/decisions.md`.

| Mockup element | Widget | Notes |
| --- | --- | --- |
| Window and three columns | `AdwApplicationWindow`, `AdwNavigationSplitView` for sidebar and content, `AdwOverlaySplitView` for the details pane | An `AdwBreakpoint` at 760sp collapses to pages |
| Header bars | `AdwHeaderBar` in an `AdwToolbarView` | Sidebar header and content header are separate bars |
| Sidebar | `AdwSidebar` (libadwaita 1.9) | Not confirmed that libadwaita-rs 0.9 binds it. Fallback: `GtkListBox` with `.navigation-sidebar` |
| All, Unplayed and the search scope | `AdwToggleGroup` (libadwaita 1.7) | Same check for the binding |
| Empty states | `AdwStatusPage` | Suggested pill button for Search Online |
| Search | `GtkSearchBar` with `GtkSearchEntry`, toggled from the header | Ctrl+F |
| Episode lists | `GtkListView` over a `gio::ListStore` | Hundreds of rows; the HIG limits boxed lists to small ones |
| Row menu and sort menu | `GMenu` in a `GtkPopoverMenu` or `GtkMenuButton` | Radio items for sort |
| Cover art | `GtkPicture` with rounded CSS | Disk cache for downloaded covers |
| Category tiles | `GtkFlowBox` of styled buttons | The one place a custom style is expected |
| Player bar | A bottom bar in the content `AdwToolbarView` | Seek is a `GtkScale` |
| Main menu items | `AdwPreferencesDialog`, `AdwShortcutsDialog`, `AdwAboutDialog` | Check which exist in the chosen runtime |
| Feedback | `AdwToastOverlay` | Import results, errors |

**Components.** `App` owns the window and navigation history. `Sidebar`, `ContentPages`, `DetailsPane` and `PlayerBar` are relm4 components. Services run as relm4 workers or commands: `Store` (SQLite), `FeedService`, `PlayerService` (wraps GstPlay and publishes state), `DownloadService` and `OpmlService`. Views never talk to the network or database directly; they send messages and render state. Use `FactoryVecDeque` only for short lists such as the queue and chapters.

## Milestones

Every milestone ends with the HIG gate passing and its unit and integration tests in place, in addition to its own done-when.

| Step | Scope | Done when |
| --- | --- | --- |
| M0 Setup | Skeleton, Flatpak, CI, `CLAUDE.md`, decisions log | The app starts from `flatpak run` and CI is green |
| M1 Data and feeds | SQLite schema, fetch and parse a feed, refresh, cover cache, tests on sample feeds with and without seasons | A feed URL becomes stored episodes, and a refresh adds only new ones |
| M2 Window shell | Split views, sidebar, both header bars, 760sp breakpoint, status pages, main menu shell | Matches the mockup at 1440px and 360px wide, light and dark |
| M3 Library views | Shows grid with the Add tile, New with the toggle group, show page with seasons, sort menu, row menu | Real data browsable end to end |
| M4 Playback | GstPlay, player bar, seek, speed, skip, saved position, details pane with chapters, MPRIS | Play, pause, close, reopen and resume at the same spot; media keys work |
| M5 Queue and lists | Queue with auto-continue, favorites, downloads with resume, auto-delete of old downloads in Preferences, played state | Queue behaves like the mockup's |
| M6 Search and Add | Global search bar with scope toggle, Add view built on what the iTunes API returns, RSS URL entry | Find a show and subscribe |
| M7 OPML | Import and export | A file from another app imports; export re-imports with no loss |
| M8 Polish | Final accessibility and translation audit, shortcuts (Ctrl+F, F9, F10), metainfo and icon, Flathub checklist | Passes the review checklist in the Flathub guidelines |

## HIG gate

The rule for Claude Code: after every UI change, run the gate before reporting the change as done.

1. **Checklist file.** `docs/hig-checklist.md` holds the rules we found while auditing the mockup: one suggested button per view; an icon or a label on a button, not both, outside header bars; header capitalization; at most two controls per row and non-focusable rows; Move Up and Move Down beside drag handles; menus of 3 to 12 items without mixed item types; circular buttons sparingly; empty states as status pages; search in a bar under the header with Ctrl+F; adaptive layout down to 360px.
2. **A `/hig-review` command.** I can write it as a Claude Code skill. It lists the changed UI files, fetches the relevant pages from developer.gnome.org/hig, compares the change with the checklist and the fetched text, and reports pass or fail with a quote for each finding.
3. **Read the guideline, not memory.** During the mockup audits I claimed once from memory that the HIG discourages icons in menu items. The menus page said nothing of the kind, and I withdrew it. The command must quote the page it checked.
4. **Screenshots.** Run the app headless at 1440 by 900 and 360 by 640, light and dark, and look at the images. Compare against the mockup.
5. **Fix, then rerun** until the report is clean.

The gate is a judgment review, not a linter. Keep a human look at the end of each milestone.

**Accessibility and translation gate.** Every UI change also passes these checks. All user-visible strings go through gettext. Every icon-only button has an accessible label and a tooltip. State changes (pressed, selected, current) are exposed to assistive technology. A keyboard-only walk-through reaches every new control. Layouts survive longer translated text, so no fixed-width labels. Check with Orca and with large text at 360px wide.

## Test strategy

Two rules drive everything else:

1. **Unit tests ship with the code.** Any change that adds or alters logic includes unit tests in the same change. No logic lands untested.
2. **Integration tests ship with the feature.** As soon as a feature ships, it gets an integration test of its main user scenario. A feature without one is not finished.

**Layers**

| Layer | What it covers | How |
| --- | --- | --- |
| Unit | Parsing, season grouping, queue and auto-continue logic, played state, auto-delete rules, OPML mapping, relm4 `update` functions | `cargo test` in `beacon-core`, plus component `update` tests that send messages and assert state. No display needed |
| Integration | Feed fetch and refresh, SQLite migrations and queries, downloads and resume, iTunes client, and the main scenario of every feature | A local HTTP server serving fixture feeds and canned iTunes responses, in-memory SQLite, an injected clock for auto-delete. Feature scenarios send messages to the services and relm4 components and assert the resulting state and database rows. No display needed |

**No UI automation.** The project does not run end-to-end tests through the real window. What the screen shows is checked by the HIG gate (screenshots, keyboard walk-through, Orca) after every UI change. To keep that gap small, keep logic out of widgets: components only translate messages into core calls and state into widget properties.

**Scenarios to cover, by milestone**

- M1 to M3: subscribe from an RSS URL, see episodes, refresh, open a show page, use the All and Unplayed toggle.
- M4: play, pause, close the app, reopen and resume at the same position.
- M5: queue order and auto-continue, mark played leaves the queue alone, download, auto-delete removes an old download.
- M6: search with This Show and Everywhere scope, no-results page, Search Online, subscribe from iTunes results.
- M7: import an OPML file, export, re-import with no loss.

**Rules for all tests**

- CI never calls the real iTunes API or real podcast servers. Use recorded fixtures.
- One scheduled job may call the live iTunes API to check the assumptions the plan flags, such as the `feedUrl` field. It must not block merges.
- Playback tests use a short local audio file and a silent output, never a real sound device.
- Unit and integration tests run on every push and pull request.
- A bug fix starts with a failing test that reproduces it.

## OPML

**Import.** Offer it from the main menu (Import Subscriptions) and by dropping a file onto the Add view.

1. Choose the file with `GtkFileDialog`, which uses the portal inside Flatpak.
2. Parse with the `opml` crate. Walk nested outlines and flatten them. Read `xmlUrl` (tolerate attribute-case differences), and use `title` or `text` as the name. Skip outlines with no feed URL.
3. Normalise each URL (trim, lower-case the host) and remove duplicates within the file and against existing subscriptions.
4. Subscribe in the background, about four feeds at a time, each fetch validating that the URL is a real feed.
5. Show progress in a dialog, then a summary toast such as "Imported 41 of 44 podcasts". Failures stay listed with the reason and a Retry button.
6. Bad XML shows an error status page. It never crashes the app.

**Export.** Main menu, Export Subscriptions, then a save dialog defaulting to `podcasts.opml`. Write OPML 2.0 with a head (title, creation date) and one `outline type="rss"` per subscription with `text`, `title`, `xmlUrl` and `htmlUrl`. Warn that private feed URLs may contain secret tokens before the file is shared.

**Tests.** Keep fixture files exported from at least three other podcast apps. The round trip (export, re-import) must lose nothing.

## Add view and iTunes

The mockup's Add view is reference only. Build the real one around what the iTunes Search API returns, and drop mockup features it cannot support.

**What Apple documents** (the search API page, read in this session):

- Search needs a `term` and a `country`. Optional parameters include `media`, `entity`, `attribute`, `limit` (1 to 200, default 50), `lang` and `explicit`.
- Results include the show name, artist, artwork URLs and explicitness. Lookup by iTunes ID is also available.
- The limit is about 20 calls per minute, and Apple says that may change.
- Apple restricts how promotional content such as artwork may be used: to promote store content, near a store badge.

**What that page does not document:** genre browsing, top charts, and the `feedUrl` field that podcast results carry. Many apps rely on `feedUrl`, so Claude Code should confirm it in a live response before building on it. Subscribing then stays plain RSS.

**What this means for the design**

1. **Search first.** Add is a search field (which still accepts a pasted RSS URL) plus a results list. With no query it shows a status page, "Search for Podcasts", instead of Popular This Week.
2. **Categories and charts only if there is a source.** The tiles and Popular This Week stay out until a real source is found and tested. Apple's top-chart feeds are a candidate but unverified here.
3. **Result rows use the fields the API actually returns.** Open a result to a show preview with Subscribe.
4. **Respect the rate limit.** Search on Enter or after a pause, never per keystroke, and cache results.
5. **Check the artwork terms** before showing Apple-supplied covers in results, or show the feed's own artwork once the show is subscribed.
6. **`country` is required.** Derive it from the system locale and make it a setting.

## Auto-delete and settings

Auto-delete is the one feature not in the mockup, so it needs a design. Agreed design:

- Preferences gets a Downloads group made of stock libadwaita rows (`AdwSwitchRow`, `AdwSpinRow`).
- **Delete Old Downloads** is a switch and the master control. It is off by default, and the rows below are disabled while it is off.
- **Delete After** is a spin row, in days.
- **Only Delete Played Episodes** is a switch, on by default. On: the clock starts when the episode was played. Off: it starts when the download finished.
- Episodes in the queue are never deleted. That is fixed, not a setting.
- Cleanup runs at startup and once a day while the app is open, and shows a toast when it removes something.
- Nothing is ever downloaded automatically.

## Still open

1. **iTunes limits.** Check Apple's terms, attribution rules and rate limits for the Search API before building on it. Apple documents about 20 calls per minute and limits on promotional artwork; see Add view and iTunes.
2. **Mockup behaviours that were pretend.** Sleep timer lengths and volume. Seasons only exist when the feed has season tags.
3. **Test feeds.** Which real podcasts should be the fixtures, especially ones with seasons and chapters?
