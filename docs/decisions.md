# Decisions

Every crate and every custom widget gets an entry here, with the reason. Before adding a crate, record its last release, downloads, repository health and licence. Figures below were checked on 2026-10-28 (crates.io and the GitHub API; "open" counts issues and pull requests together).

## Crates

| Crate | Job | Latest | Last release | Downloads (all / 90 days) | Repository | Licence | Why |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `relm4` (features `libadwaita`, `gnome_50`) | UI framework | 0.11.0 | 2026-04-08 | 1.36M / 783k | Relm4/Relm4, 73 open, pushed 2026-10-27 | Apache-2.0 OR MIT | Required by the build requirements. Pulls gtk4 0.11 and libadwaita 0.9; `gnome_50` enables `adw/v1_9`, which binds `AdwSidebar`, `AdwToggleGroup` and `AdwShortcutsDialog` |
| `gettext-rs` (feature `gettext-system`) | Translation | 0.8.0 | 2026-08-05 | 5.0M / 663k | gettext-rs/gettext-rs, 23 open, pushed 2026-10-17 | MIT | The standard gettext binding used by GNOME Rust apps. `gettext-system` links the runtime's libintl instead of building gettext |
| `tracing` | Logging | 0.1.44 | 2025-12-18 | 984M / 242M | tokio-rs/tracing, 905 open, pushed 2026-10-09 | MIT | De facto standard; works with tokio, which relm4 commands use. The open count is high but the project is very active |
| `tracing-subscriber` (feature `env-filter`) | Log output | 0.3.23 | 2026-03-13 | 702M / 175M | tokio-rs/tracing | MIT | Companion to `tracing`; `RUST_LOG` filtering |
| `glib-build-tools` (build dependency) | Compile the GResource in `build.rs` | 0.22.8 | 2026-06-29 | 543k / 138k | gtk-rs/gtk-rs-core, 170 open, pushed 2026-10-28 | MIT | Lets a plain `cargo build` embed resources, so clippy and tests run without Meson |
| `rss` (no default features) | Feed parsing (M1) | 2.1.2 | 2026-09-10 | 5.1M / 605k | rust-syndication/rss, 12 open, pushed 2026-09-30 | MIT/Apache-2.0 | Exposes every iTunes item field and keeps other namespaces (`podcast:`, `psc:`) in an extension map, so one parse covers seasons, episode numbers, durations and chapters. RSS only; podcasts are RSS 2.0. Chosen over `feed-rs`, see below |
| `reqwest` (default features + `stream`) | HTTP (M1) | 0.13.5 | 2026-09-08 | 850M / 236M | seanmonstar/reqwest, 487 open, pushed 2026-10-19 | MIT OR Apache-2.0 | Standard async HTTP client. Default TLS is rustls with the platform verifier, so it uses the runtime's CA store and needs no OpenSSL |
| `tokio` (`fs`) | Async runtime (M1) | 1.54.1 | 2026-10-21 | 1.12B / 278M | tokio-rs/tokio, 441 open, pushed 2026-10-29 | MIT | Required by reqwest; relm4 commands run on it. Core uses only `tokio::fs` |
| `rusqlite` (`chrono`, `url`) | Storage (M1) | 0.40.2 (0.41.1 exists) | 2026-10-28 for 0.41.1 | 138M / 51M | rusqlite/rusqlite, 172 open, pushed 2026-10-28 | MIT | Links the runtime's SQLite 3.50.4, which has FTS5 (checked in `org.gnome.Platform//50`). Held at 0.40 because `rusqlite_migration` 2.6 requires it; move both together |
| `rusqlite_migration` | Schema migrations (M1) | 2.6.0 | 2026-05-28 | 4.8M / 1.1M | cljoly/rusqlite_migration, 8 open, pushed 2026-10-26 | Apache-2.0 | Versioned migrations through SQLite's `user_version`; SQL lives in `store/migrations/` |
| `thiserror` | Typed errors (M1) | 2.0.21 | 2026-09-23 | 1.73B / 468M | dtolnay/thiserror, 32 open, pushed 2026-10-16 | MIT OR Apache-2.0 | Standard derive for error enums |
| `chrono` | Dates (M1) | 0.4.45 | 2026-06-04 | 925M / 217M | chronotope/chrono, 194 open, pushed 2026-09-07 | MIT OR Apache-2.0 | RFC 2822 and RFC 3339 parsing for `<pubDate>`; rusqlite maps it directly |
| `url` | URLs (M1) | 2.5.8 | 2026-01-05 | 1.0B / 233M | servo/rust-url, 208 open, pushed 2026-07-31 | MIT OR Apache-2.0 | Validated URL type, shared with reqwest and rusqlite |
| `sha2` | Cover cache keys (M1) | 0.11.1 | 2026-10-12 | 1.13B / 335M | RustCrypto/hashes, 38 open, pushed 2026-10-27 | MIT OR Apache-2.0 | Stable file names from image URLs |
| `httpmock` (dev) | Local HTTP server for tests (M1) | 0.9.0 | 2026-10-20 | 33M / 6.6M | httpmock/httpmock, 24 open, pushed 2026-10-26 | MIT | Serves fixture feeds in integration tests. Chosen over `wiremock` (no release or commit since 2025-08-24) |
| `tempfile` (dev) | Temporary files in tests (M1) | 3.27.0 | 2026-03-11 | 936M / 227M | Stebalien/tempfile, 10 open, pushed 2026-10-27 | MIT OR Apache-2.0 | Cover cache and on-disk database tests |

### Rejected

- **`feed-rs` 3.0** (tried in M1). Two problems with podcast feeds: it drops an item's iTunes data (duration, season, episode, image) unless the item also has `<itunes:title>` or `<itunes:summary>`, because `MediaObject::is_not_empty` ignores those fields; and it rejects `<itunes:duration>` written as `MM:SS`. It also does not read chapters. Working around that needed a second `roxmltree` parse of every feed, so it was replaced by `rss`. The first problem is worth reporting upstream.
- **`roxmltree`**: only needed for the `feed-rs` workaround.
- **`wiremock`**: no release or commit since 2025-08-24; `httpmock` is active.

## Planned crates

One maintained crate per job. Before adding each one, check it and move it to the Crates table with its figures.

| Job | Candidate | To check before adding |
| --- | --- | --- |
| Playback | `gstreamer-play` from gstreamer-rs (high-level player: seek, rate, position and state signals) | Use the release that matches gtk4-rs; GStreamer 1.26 is in the runtime |
| OPML | `opml` | That it both parses and writes (both listed on docs.rs) |
| Directory search | iTunes Search API (no key) | Apple's terms and rate limits; see `docs/specs/add-and-itunes.md` |
| Media keys and MPRIS | An MPRIS server crate, not chosen | Compare candidates on last release, downloads and open issues |
| Settings | GSettings through `gio` | Already in use |

**Reference code:** [GNOME Podcasts](https://github.com/alatiera/gnome-podcasts) (Rust, GTK4, libadwaita). Read it for feed refresh and download edge cases; check its licence before reusing any code.

## Runtime

- **GNOME 50 runtime** (checked in the installed `org.gnome.Platform//50`): GTK 4.22.5, libadwaita 1.9 (exports `adw_sidebar_new`, `adw_toggle_group_new`, `adw_shortcuts_dialog_new`, `adw_about_dialog_new`), GStreamer 1.26.11 with `libgstplay`, SQLite 3.50.4. Every widget in the Architecture table is available; no fallbacks needed.
- **Staying on GNOME 50** (2026-10-28). GNOME 51 is newer and matches the host, but relm4 0.11.0 has no `gnome_51` feature yet. Migrate once it does; `.github/workflows/relm4-gnome-51.yml` checks weekly and fails when the feature appears.

## Build and packaging

- **Cargo builds, Meson installs.** Meson calls Cargo through `build-aux/cargo-build.sh` and installs the desktop file, metainfo, schema and icons. The GResource is compiled by `build.rs`, so Cargo alone can build and test.
- **Offline Flatpak build.** `build-aux/cargo-sources.json` is generated from `Cargo.lock` by `build-aux/flatpak-cargo-sources.py`, a small local script that writes the same layout as flatpak-builder-tools' generator for crates.io dependencies. It avoids downloading and running the upstream script on every update. Switch to the upstream generator if a git dependency is ever needed.
- **Sandbox permissions** start minimal (`ipc`, `wayland`, `fallback-x11`, `dri`).
  - `--share=network` (M1): fetching feeds and covers.
  - `--socket=pulseaudio` comes with M4 (playback).

## Behaviour

- **Seen state on subscribe** (M1). A new subscription marks every episode seen except the newest, so subscribing to a show with 300 episodes brings one blue dot, not 300. Episodes found by later refreshes start unseen. Approved by the user on 2026-10-28.
- **Seasons come only from tags** (M1, per the requirements). Some shows put seasons only in titles ("Season 6, Ep 11 - …" in Hello From The Magic Tavern); those do not group into seasons.
- **Navigation: sections in `App`, deeper pages in an `AdwNavigationView`** (M2, decided with the user 2026-10-29). The sidebar switches sections, and `App` passes the choice on; it keeps no navigation state of its own. The content pane is a navigation view whose root page shows the section; deeper pages (Add now, podcast pages in M3) are pushed over whatever section is showing and slide in from the right. The view owns the stack and supplies Back, the two-finger touchpad swipe, the mouse's back button and Alt+Left when focus is inside it; `win.back` (Alt+Left, Back key) covers focus elsewhere. Back therefore appears only on deeper pages, never for switching sections. An earlier breadcrumb `Trail` in `App` duplicated the view's stack and was removed.
- **Episode identity** (M1). The feed's GUID; without one, the enclosure URL, then the item link, then the title. Refreshes match on this, so it must not change between fetches.

## Custom widgets and icons

No custom widgets yet.

- **Queue icon** (`data/icons/scalable/actions/queue-symbolic.svg`, M2). The Adwaita icon theme in GNOME 50 has no queue or playlist icon; this one follows the mockup (three lines and a play triangle).
- **Shows icon** (M2) reuses Beacon's symbolic app icon, which is the broadcast symbol the mockup uses; the theme has no podcast icon.
- **Add is a page opened from the sidebar header** (M2, decided by the user 2026-10-29). The HIG puts add buttons at the start of the header bar ("Buttons for the main user actions, such as _new_, _add_, _open_ and _back_. These are placed at the start of the header bar") and controls that affect the sidebar list above it. So + sits at the start of the sidebar header, and Add slides in over the current section with Back (changed the same day from always opening under Shows). This replaces the mockup's Add item at the bottom of the sidebar.
- **Refresh lives in the main menu and in the Shows and New headers** (M2, asked by the user 2026-10-29). It is the first item of the main menu, in the group with Import and Export, and a button at the start of the content header in Shows and New, where new episodes appear, in the place Back takes on deeper pages. The sidebar header holds only Add Podcast, the title and the main menu.
- **Narrow sidebar style** (M2). Below 760sp both sidebars switch to `AdwSidebarMode::Page` (boxed rows with arrows), libadwaita's pattern for a sidebar collapsed into a page, instead of the mockup's plain rows.
