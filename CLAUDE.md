# Beacon

A podcast app for GNOME. Rust, relm4 0.11, libadwaita 1.9, GNOME 50 runtime, packaged as a Flatpak. App ID `com.theoforger.Beacon`. Licence GPL-3.0-or-later.

- `MILESTONES.md`: milestone checklist; tick items as they land.
- `docs/architecture.md`: target layout, components and the widget map, plus what is built. `docs/testing.md`: test strategy. `docs/specs/`: feature specs (OPML, Add page and iTunes, auto-delete).
- `docs/decisions.md`: every crate and custom widget, with the reason, plus planned crates. `docs/hig-checklist.md`: the HIG gate. `docs/design/`: the mockup.

## Commands

```sh
# Fast loop (no Meson). Compile the schema once, then run.
glib-compile-schemas --strict --targetdir=target/schemas data/
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

Headless screenshots for the HIG gate: `build-aux/screenshots.sh` writes 1440×900 and 360×640, light and dark, to `target/screenshots/`; limit it with `SIZES=360x640 SCHEMES=dark build-aux/screenshots.sh`. It runs on a private session bus (`dbus-run-session`); without one, a Beacon you have open gets activated instead and the capture is black. To screenshot a state reached by clicks or keys, use `build-aux/headless/shot.sh SIZE SCHEME OUT key:F10 click:X,Y …` (it prints the frame's brightness; 0 means black). Run the HIG gate with `/hig-review`. Meson is not installed on the host; use `uvx --with ninja meson …`.

## Architecture

Cargo workspace. `crates/beacon-core` holds the model, SQLite store and services (feeds, iTunes, downloads, cleanup, OPML) and never imports GTK, GLib or relm4. `crates/beacon-gtk` is the app: `App` owns the window and navigation history; `Sidebar`, `ContentPages`, `DetailsPane` and `PlayerBar` are relm4 components; services run as relm4 workers or commands. Views send typed messages and render state; they never touch the network, database or files. Cargo builds the binary and embeds the GResource (`build.rs`); Meson installs data files and runs gettext.

## Rules

1. Dependencies point one way: `beacon-gtk` → `beacon-core`.
2. No I/O in the UI layer.
3. One component per file, under about 400 lines.
4. Typed messages only; no stringly-typed events.
5. All CSS in `data/style.css`, libadwaita named colours and style classes only.
6. Every user-visible string through gettext (`gettextrs::gettext`), including tooltips and accessible labels. Add new files with strings to `po/POTFILES.in`.
7. Every icon-only button gets `set_tooltip_text` and an accessible `Label` property.
8. Prefer maintained crates and stock libadwaita widgets. Every new crate (with last release, downloads, repository health, licence) and every custom widget gets an entry in `docs/decisions.md`. Add crates with `cargo add` (latest release, no exact pin); keep relm4, gtk4-rs, libadwaita-rs and gstreamer-rs in step; the GNOME 50 runtime is the ceiling.
9. SPDX header `GPL-3.0-or-later` in every source file.
10. Errors are typed in core and shown as toasts or status pages, never panics. The UI maps each error variant to its own gettext message; an error's `Display` text is for logs only, since core has no translations.
11. Unit tests ship with every logic change; every feature ships with an integration test of its main scenario. A bug fix starts with a failing test. CI never calls live iTunes or podcast servers.
12. Sandbox permissions stay as narrow as works; file access goes through portals (`GtkFileDialog`). Record each new permission in `docs/decisions.md`.

## HIG gate

After every UI change, before reporting it done:

1. Go through `docs/hig-checklist.md`.
2. Fetch the relevant pages from https://developer.gnome.org/hig/ and quote them for each finding. Do not cite the HIG from memory.
3. Screenshots, sparingly. Take only the views the change affects (often one size and one scheme), once the change is finished, not while iterating. Every affected view you skip goes in `docs/hig-reviews/pending-screenshots.md`: what to check, which views, and how to reach the state. At the end of each milestone take the full set (1440×900 and 360×640, light and dark), work through every pending row, record the results in `docs/hig-reviews/mN.md` and clear the rows. A milestone cannot close with pending rows.
4. Run the translation and label checks in the checklist (gettext, `POTFILES.in`, accessible labels and tooltips). Until the accessibility checkpoint after M3, do not run the accessibility walk-throughs (Orca, keyboard, large text, exposed states): log what each change needs checked in `docs/hig-reviews/pending-accessibility.md`. From M4 on, run them in the gate.
5. Fix and rerun until clean. A human looks at the end of each milestone.

## Requirements

This section is the contract for Claude Code. It records everything agreed in the design conversation.

### Build requirements

1. Project setup comes before any feature work (done in M0).
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
- The sidebar runs from the top edge of the window and has its own header: Add Podcast (+) on the left, the app title in the centre, Main Menu on the right. It has no group title. Refresh Feeds is the first item of the main menu, and also a button at the start of the content header in Shows and New. (Changed on 2026-10-29: Add moved from the bottom of the sidebar to the header, following the HIG's header-bar guidance, and Refresh moved from the header into the menu.)
- Sidebar items: Shows, New, Queue, Favorites, Downloaded. Shows carries a count of podcasts that have new episodes. New shows the number of new episodes. Queue shows its length.
- The centre header has Back on the left (or Refresh Feeds, in Shows and New), the view title in the centre, then Search and the details-pane toggle on the right. There is no Forward button. Back follows the page hierarchy, not history (decided 2026-10-29): it appears only on a deeper page (such as a podcast page under Shows) and returns to its parent. Switching sections never adds a Back. Deeper pages slide in from the right over their parent, and go back with Back, a two-finger touchpad swipe, the mouse's back button, Alt+Left or the Back key. Search is hidden on the Add page.
- The details pane also runs from the top edge. Its header says Episode Details. The window close button sits in the pane header while the pane is open, and in the centre header when it is closed.
- On narrow windows the sidebar becomes a full page. Choosing an item pushes the content on top, and Back returns to the sidebar. The details pane becomes a full page with its own Back button, starts closed, and its header toggle is hidden.
- The app opens on Shows. Dark mode and the accent colour follow the system.

**Views**

- **Shows:** a grid of cover art with a badge for unplayed episodes. The last tile is Add Podcast, which opens the Add page.
- **New:** episodes from all subscribed shows grouped by recency (Today, Yesterday, weekday, Last week, Earlier in the month). An All / Unplayed toggle group sits above, and the choice is remembered.
- **Podcast page:** a hero with cover, category, title, author and description. Subscribe is the accent button, and becomes a neutral Unsubscribe once subscribed. A More Options menu holds Mark All as Played, Download All Episodes and Copy Feed Address. Below sit an All / Unplayed toggle group (remembered per show) and a sort menu with radio items, Newest First and Oldest First. Episodes group into collapsible seasons, with season names, when the feed carries season tags.
- **Queue:** three groups. Now Playing. Next in Queue, with count and total time, for episodes added by hand. Next from the list the playing episode came from. The behaviour copies a music app: manual queue first, then the rest of the list. Rows have a drag handle at the start, and Move Up and Move Down in the row menu.
- **Favorites** and **Downloaded:** episode lists with status pages when empty.
- **Add:** a page under Shows (with Back to Shows), opened from the + in the sidebar header or the Add Podcast tile. One search field that also accepts a pasted RSS feed URL (an RSS Feed card with Add Feed). Reference only: the real Add view is modeled on what the iTunes Search API returns (see `docs/specs/add-and-itunes.md`), so the category tiles, Popular This Week and the card layout in the mockup are placeholders.
- **Search:** a header toggle (and Ctrl+F) opens a search bar under the header. On a podcast page a This Show / Everywhere toggle appears, and This Show filters that page in place. Otherwise results show matching shows and episodes. With no results, a suggested Search Online button opens the Add page with the query filled in. The button does not appear in This Show scope.
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

