# Beacon

A podcast app for GNOME. Rust, gtk4-rs and libadwaita-rs (GTK 4.24, libadwaita 1.10), GNOME 51 runtime, packaged as a Flatpak. App ID `com.theoforger.Beacon`. Licence GPL-3.0-or-later.

- `MILESTONES.md`: milestone checklist; tick items as they land.
- `docs/architecture.md`: target layout, views and the widget map, plus what is built. `docs/testing.md`: test strategy. `docs/specs/`: feature specs (OPML, Add page and iTunes, auto-delete, sync).
- `docs/decisions.md`: every crate and custom widget, with the reason, plus planned crates. `docs/hig-checklist.md`: the HIG gate. `docs/design/`: the mockup.

## Commands

```sh
# Fast loop (no Meson). Compile the schema once, then run.
mkdir -p target/schemas && glib-compile-schemas --strict --targetdir=target/schemas data/
GSETTINGS_SCHEMA_DIR=target/schemas cargo run

# Checks (same as CI)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check

# Meson: build, validate data files, regenerate the translation template
meson setup _build && meson compile -C _build && meson test -C _build
meson compile -C _build beacon-pot

# Flatpak (the real target). Regenerate cargo sources after any Cargo.lock change.
# flatpak-builder runs from the org.flatpak.Builder Flatpak (no host package).
./build-aux/update-cargo-sources.sh
flatpak run org.flatpak.Builder --user --install --force-clean build-flatpak build-aux/com.theoforger.Beacon.json
flatpak run com.theoforger.Beacon
flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest build-aux/com.theoforger.Beacon.json
```

Headless screenshots for the HIG gate: `build-aux/screenshots.sh` writes 1440×900 and 360×640, light and dark, to `target/screenshots/`; limit it with `SIZES=360x640 SCHEMES=dark build-aux/screenshots.sh`. It runs on a private session bus (`dbus-run-session`); without one, a Beacon you have open gets activated instead and the capture is black. To screenshot a state reached by clicks or keys, use `build-aux/headless/shot.sh SIZE SCHEME OUT key:F10 click:X,Y type:TEXT wait:2 …` (it prints the frame's brightness; 0 means black); `press:X,Y drag:X,Y release` drags, and the schemes `light-hc` and `dark-hc` add high contrast. It uses a scratch library, never yours; set `BEACON_STATE_DIR` to keep one across shots and `BEACON_SCALE=2` for a HiDPI screen (click coordinates are then in device pixels), and serve fixtures with `python3 -m http.server` to subscribe to them. `RUST_LOG=beacon=debug` logs every message `App` handles, often a cheaper check than a screenshot. Run the HIG gate with `/hig-review`. The screenshot scripts need Xvfb, ImageMagick, python-xlib and Mesa; without Mesa the app aborts and the capture is black.

## Architecture

Cargo workspace. `crates/beacon-core` holds the model, SQLite store and services (feeds, iTunes, downloads, cleanup, OPML) and never imports GTK or GLib. `crates/beacon-gtk` is the app: `App` owns the window and switches sections; deeper pages are pushed onto the content pane's `AdwNavigationView`, which owns that stack; the window (`app/object.rs`, an `AdwApplicationWindow` subclass) holds `App` and handles its `AppMsg`s and background results one at a time from one channel (`app/sender.rs`); background work runs on a tokio runtime the app owns. `Sidebar`, `Content`, `DetailsPane`, `PlayerBar` and the section views are GObject subclasses: each takes a typed input enum through `send`, queued so it handles one at a time (`ui/messages.rs`), and reports a typed output enum to the callback its owner connects. Views send typed messages and render state; they never touch the network, database or files. Cargo builds the binary and embeds the GResource (`build.rs`); Meson installs data files and runs gettext.

## Rules

1. Dependencies point one way: `beacon-gtk` → `beacon-core`.
2. No I/O in the UI layer.
3. One view per file, under about 400 lines.
4. Typed messages only; no stringly-typed events.
5. All CSS in `data/style.css`, libadwaita named colours and style classes only.
6. Every user-visible string through gettext (`gettextrs::gettext`), including tooltips and accessible labels. Add new files with strings to `po/POTFILES.in`.
7. Every icon-only button gets `set_tooltip_text` and an accessible `Label` property.
8. Prefer maintained crates and stock libadwaita widgets. Every new crate (with last release, downloads, repository health, licence) and every custom widget gets an entry in `docs/decisions.md`. Add crates with `cargo add` (latest release, no exact pin); keep gtk4-rs, libadwaita-rs and gstreamer-rs in step; the GNOME 51 runtime is the ceiling, and each binding's version feature matches it (gtk `v4_24`, adw `v1_10`, gstreamer `v1_28`).
9. SPDX header `GPL-3.0-or-later` in every source file.
10. Errors are typed in core and shown as toasts or status pages, never panics. The UI maps each error variant to its own gettext message; an error's `Display` text is for logs only, since core has no translations.
11. Unit tests ship with every logic change; every feature ships with an integration test of its main scenario. A bug fix starts with a failing test. CI never calls live iTunes or podcast servers.
12. Sandbox permissions stay as narrow as works; file access goes through portals (`GtkFileDialog`). Record each new permission in `docs/decisions.md`.
13. For UI elements GNOME Podcasts also has, use the same components it uses (decided 2026-10-30). Its source, https://gitlab.gnome.org/World/podcasts (GPL-3.0-or-later, like Beacon), is the reference: read its `.ui` files and widgets before building a similar element, and record where Beacon follows or departs from it in `docs/decisions.md`.

## HIG gate

After every UI change, before reporting it done:

1. Go through `docs/hig-checklist.md`.
2. Fetch the relevant pages from https://developer.gnome.org/hig/ and quote them for each finding. Do not cite the HIG from memory.
3. Screenshots, sparingly. Take only the views the change affects (often one size and one scheme), once the change is finished, not while iterating. Every affected view you skip goes in `docs/hig-reviews/pending-screenshots.md`: what to check, which views, and how to reach the state. At the end of each milestone take the full set (1440×900 and 360×640, light and dark), work through every pending row, record the results in `docs/hig-reviews/mN.md` and clear the rows. A milestone cannot close with pending rows in its Screenshots table.
4. Run the translation and accessibility checks that need no person: gettext, `POTFILES.in`, accessible labels and tooltips, states exposed in the code (checked in tests where they can be), layouts that survive longer translated text. The checks that need a person (Orca, keyboard walk-throughs, large text, media keys) are deferred until the UI is final (decided 2026-11-08): log what each change needs checked in the Manual accessibility checks table of `docs/hig-reviews/pending-screenshots.md`. They are worked through in M9.
5. Fix and rerun until clean. A human looks at the end of each milestone.

## Requirements

This section is the contract for Claude Code. It records everything agreed in the design conversation.

### Build requirements

1. Project setup comes before any feature work (done in M0).
2. Use Rust with gtk4-rs and libadwaita-rs directly, views as GObject subclasses (changed 2026-11-09 from the relm4 framework).
3. Prefer well-maintained, off-the-shelf Rust libraries over writing things from scratch.
4. After every UI change, check it against the GNOME Human Interface Guidelines (HIG).
5. Use libadwaita components wherever possible instead of building widgets.
6. Support OPML import and export.

### Decisions

- **Build environment:** build in a Flatpak against the GNOME 51 runtime, so the build does not depend on the host.
- **Target:** GNOME 51 (moved from GNOME 50 on 2026-11-09).
- **Directory search:** iTunes Search API.
- **Sync:** nextcloud-gpodder (subscriptions, play positions, played state), behind a backend trait so gpodder.net-style servers can follow; see `docs/specs/sync.md` (decided 2026-11-08, replacing "none for now").
- **Downloads:** manual only, never automatic. An auto-delete feature removes old downloaded episodes, configurable in Settings.
- **Translation and accessibility:** both from day one.
- **Window sizes:** small desktop windows only, down to 360px wide. No Phosh-specific work.
- **Versions:** use the latest release of every crate and do not pin to a specific version. Only the GNOME 51 runtime is fixed.
- **Licence:** GPL-3.0-or-later. Put the licence text in `LICENSE` and an SPDX header in every source file.
- **Distribution:** Flathub is not the goal right now, but build to its requirements from the start so submitting later is a checklist, not a rewrite. That means a validated metainfo file and desktop file (checked in CI), the narrowest sandbox permissions that work, portals for file access, and cargo sources generated for an offline Flatpak build.
- **Name and app ID:** Beacon, with the app ID `com.theoforger.Beacon`. Use the ID everywhere it appears: the `.desktop` and metainfo file names, the GSettings schema ID and path (`/com/theoforger/Beacon/`), the gresource prefix, the icon names, the Flatpak manifest and `GApplication`. Flathub accepts a reverse-DNS ID only for a domain the developer controls, so keep `theoforger.com` under your control.

### Product requirements from the mockup

Link to the mockup: https://claude.ai/code/artifact/b9bbe38d-528e-453e-a46a-0b2393da4cc5

Read the design's files from that link. Match how it looks.

**Layout and navigation**

- Three columns: a left sidebar, a centre view, and an expandable right details pane. A persistent playback bar sits at the bottom across the full width.
- The sidebar runs from the top edge of the window and has its own header: Add Podcast (+) on the left, the app title in the centre, Main Menu on the right. It has no group title. Refresh Feeds is the first item of the main menu, and also a button at the start of the content header in Shows and New, except in the narrow layout, where the header would not fit 360px beside minimise, maximise and close (changed 2026-11-08). (Changed on 2026-10-29: Add moved from the bottom of the sidebar to the header, following the HIG's header-bar guidance, and Refresh moved from the header into the menu.)
- Sidebar items: Shows, New, Queue, Favorites, Downloaded. Shows carries a count of podcasts that have new episodes. New shows the number of new episodes. Queue shows its length.
- The centre header has Back on the left (or Refresh Feeds, in Shows and New), the view title in the centre, then Search and the details-pane toggle on the right (F9 also toggles the pane, added 2026-11-08). There is no Forward button. Back appears only on a deeper page and returns to the page under it, never for switching sections (decided 2026-10-29). Deeper pages (Add, podcast pages) slide in from the right over whatever is showing, and go back with Back, a two-finger touchpad swipe, the mouse's back button, Alt+Left or the Back key. Search is hidden on the Add page.
- The details pane also runs from the top edge. Its header says Episode Details. The window close button sits in the pane header while the pane is open, and in the centre header when it is closed.
- On narrow windows the sidebar becomes a full page. Choosing an item pushes the content on top, and Back returns to the sidebar. The details pane becomes a full page with its own Back button, starts closed, and its header toggle is hidden.
- The app opens on Shows. Dark mode and the accent colour follow the system.

**Views**

- **Shows:** a grid of cover art with a badge for unplayed episodes. The last tile is Add Podcast, which opens the Add page.
- **New:** episodes from all subscribed shows grouped by recency (Today, Yesterday, weekday, Last week, Earlier in the month). The same filter button and menu as a podcast page sits in the header, shown only in New, and the choices are remembered; Oldest First reverses the groups (changed 2026-11-01 from an All / Unplayed toggle group).
- **Podcast page:** a hero with cover, category, title, author and description. Subscribe is the accent button, and becomes a neutral Unsubscribe once subscribed. A More Options menu holds Mark All as Played, Download All Episodes and Copy Feed Address. A filter button in the page header opens one menu, as in GNOME Podcasts, with three groups of radio items: Newest First / Oldest First; All / Played / Unplayed; All / Downloaded / Not Downloaded. The choices are remembered per show (changed 2026-10-29 from a toggle group and a separate sort menu). Episodes group into collapsible seasons, with season names, when the feed carries season tags.
- **Queue:** three groups. Now Playing. Next in Queue, with count and total time, for episodes added by hand. Next from the list the playing episode came from. The behaviour copies a music app: manual queue first, then the rest of the list. Rows have a drag handle at the start, and Move Up and Move Down in the row menu. Starting an episode takes it off the queue and clears its blue dot. Next from the list shows up to ten episodes, the ones after the playing episode in the list it was started from (in that list's order, filters included), leaving out queued ones; the Queue view's own rows keep that list. The list and the last episode played are remembered across restarts, so Next from carries on after it. Once the player holds all of the playing episode (streams are buffered whole), the next one (the queue's first, else the list's next) is loaded in the background so it starts at once, only on a connection that is not metered, and following the connection as it changes (asked by the user 2026-11-08 and 2026-11-09).
- **Favorites** and **Downloaded:** episode lists with status pages when empty. Queue, Favorites and Downloaded have selection mode, as New does, through the same Select button and bar in the content header. Narrow windows leave Select out of the header; a long press (touch), a secondary click or the row menu's Select enters selection mode there (asked by the user 2026-11-10). Queue and Downloaded have a Clear button at the start of the header: Clear Queue (undone from its toast) and Delete All Downloads (after a confirmation), unavailable when there is nothing to clear (asked by the user 2026-11-08).
- **Add:** a page that slides in over the current section (Back returns to it), opened from the + in the sidebar header or the Add Podcast tile. One search field that also accepts a pasted RSS feed URL (an RSS Feed card with Add Feed). Reference only: the real Add view is modeled on what the iTunes Search API returns (see `docs/specs/add-and-itunes.md`), so the category tiles, Popular This Week and the card layout in the mockup are placeholders.
- **Search:** a header toggle (and Ctrl+F) opens a search bar under the header. On a podcast page a This Show / Everywhere toggle appears, and This Show filters that page in place. Otherwise results show matching shows and episodes. With no results, a suggested Search Online button opens the Add page with the query filled in. The button does not appear in This Show scope.
- **Empty states** are status pages with an icon, a title and one line of description.

**Episode rows**

- Episode lists are one recycling list per page (a one-column `GtkGridView` read as a list), as the HIG asks of large or dynamic lists, so only the rows near the screen exist and long feeds scroll smoothly (changed 2026-11-08 from GNOME Podcasts' `boxed-list` `GtkListBox`es, which laid out every row they held). Each recency group or season is drawn as a card under its heading, as a boxed list looks.
- Cover, title (up to two lines in any window, one when a progress bar needs the room; changed 2026-11-08 from one line in wide windows), and a subtitle of season and episode, date and length, in dimmed body text (changed 2026-11-08 from the smaller caption style). The subtitle never shows the show's name, which the cover shows (changed 2026-11-08; screen readers still hear it in the row's name), nor time left. On a podcast page, rows leave out the cover, which is the page's own (changed 2026-11-04).
- Unplayed episodes have a blue dot. Viewing the details removes it, and marking an episode unplayed does not bring it back. Played episodes have a grey check after the title. Favourites have a grey star after the title (and after the check), added 2026-11-08. Partly played episodes have a progress bar.
- The playing episode shows no In Queue marker.
- The row play button has no fill, blends into the row, and gives hover feedback. It shows pause while playing. In the narrow layout rows leave out the play button (the details page, a tap away, has Play) and draw a 48px cover instead of 64px, so the title has room (changed 2026-11-08).
- The row menu holds Show Details, Move Up and Move Down (queue only), Add to or Remove From Queue, Download or Delete Download, Add to or Remove From Favorites, Mark as Played or Unplayed, and Reset Progress, only when the episode has a saved position (added 2026-11-08; it leaves played, favourite and the queue alone, and the playing episode jumps back to its start).
- Marking an episode as played or unplayed never touches the queue. That includes Mark All as Played on a podcast page.

**Details pane**

- Cover, show link, title, subtitle, then Play, Pause or Resume (accent coloured). The episode's other actions (queue, download, favourite, played, Reset Progress) are in More Options (⋮) beside Play, the same as a row's menu without Show Details and Select (changed 2026-11-08 from four icon toggles beside Play, which had grown too many).
- The pane follows the mockup's order: cover (left-aligned), show, title, the long subtitle (Season 3, Episode 2 • date • length), Play, About This Episode, then chapters.
- Description, then chapters. The current chapter has a gray background and its time in the accent colour (changed 2026-11-08 from bold text), and choosing a chapter seeks to it.

**Player bar**

- Chapters (left of Back 10 Seconds; a popover listing the playing episode's chapters, unavailable when it has none; replaced Favourite on 2026-11-08), Back 10 Seconds, a neutral round Play and Pause, Forward 30 Seconds, Next Episode, a seek bar with elapsed and remaining time and a tick where each chapter starts, playback speed, sleep timer and volume. The now-playing area opens the details pane. At launch the bar shows the episode last played, paused where it was left, unless it was played to the end, and starts buffering straight away on a connection that is not metered (or if it is downloaded); on a metered one its audio is opened only when it is played (asked by the user 2026-11-09). The seek bar's trough fills as far as the player holds.

**Look**

- Selectors use the toggle-group style: a rounded track with a lighter pill on the selected option. Search fields are fully rounded.
- Header toggles (Search, details pane) show a darker background while on.
- Pill-shaped buttons only for the suggested action, used sparingly.

