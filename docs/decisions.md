# Decisions

Every crate and every custom widget gets an entry here, with the reason. Before adding a crate, record its last release, downloads, repository health and licence. Figures below were checked on 2026-10-28 (crates.io and the GitHub API; "open" counts issues and pull requests together).

## Crates

| Crate | Job | Latest | Last release | Downloads (all / 90 days) | Repository | Licence | Why |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `relm4` (features `libadwaita`, `gnome_50`) | UI framework | 0.11.0 | 2026-04-08 | 1.36M / 783k | Relm4/Relm4, 73 open, pushed 2026-10-27 | Apache-2.0 OR MIT | Required by the plan. Pulls gtk4 0.11 and libadwaita 0.9; `gnome_50` enables `adw/v1_9`, which binds `AdwSidebar`, `AdwToggleGroup` and `AdwShortcutsDialog` |
| `gettext-rs` (feature `gettext-system`) | Translation | 0.8.0 | 2026-08-05 | 5.0M / 663k | gettext-rs/gettext-rs, 23 open, pushed 2026-10-17 | MIT | The standard gettext binding used by GNOME Rust apps. `gettext-system` links the runtime's libintl instead of building gettext |
| `tracing` | Logging | 0.1.44 | 2025-12-18 | 984M / 242M | tokio-rs/tracing, 905 open, pushed 2026-10-09 | MIT | De facto standard; works with tokio, which relm4 commands use. The open count is high but the project is very active |
| `tracing-subscriber` (feature `env-filter`) | Log output | 0.3.23 | 2026-03-13 | 702M / 175M | tokio-rs/tracing | MIT | Companion to `tracing`; `RUST_LOG` filtering |
| `glib-build-tools` (build dependency) | Compile the GResource in `build.rs` | 0.22.8 | 2026-06-29 | 543k / 138k | gtk-rs/gtk-rs-core, 170 open, pushed 2026-10-28 | MIT | Lets a plain `cargo build` embed resources, so clippy and tests run without Meson |

## Runtime

- **GNOME 50 runtime** (checked in the installed `org.gnome.Platform//50`): GTK 4.22.5, libadwaita 1.9 (exports `adw_sidebar_new`, `adw_toggle_group_new`, `adw_shortcuts_dialog_new`, `adw_about_dialog_new`), GStreamer 1.26.11 with `libgstplay`, SQLite 3.50.4. Every widget in the Architecture table is available; no fallbacks needed.
- **Staying on GNOME 50** (2026-10-28). GNOME 51 is newer and matches the host, but relm4 0.11.0 has no `gnome_51` feature yet. Migrate once it does; `.github/workflows/relm4-gnome-51.yml` checks weekly and fails when the feature appears.

## Build and packaging

- **Cargo builds, Meson installs.** Meson calls Cargo through `build-aux/cargo-build.sh` and installs the desktop file, metainfo, schema and icons. The GResource is compiled by `build.rs`, so Cargo alone can build and test.
- **Offline Flatpak build.** `build-aux/cargo-sources.json` is generated from `Cargo.lock` by `build-aux/flatpak-cargo-sources.py`, a small local script that writes the same layout as flatpak-builder-tools' generator for crates.io dependencies. It avoids downloading and running the upstream script on every update. Switch to the upstream generator if a git dependency is ever needed.
- **Sandbox permissions** start minimal (`ipc`, `wayland`, `fallback-x11`, `dri`). Add `--share=network` with M1 (feeds) and `--socket=pulseaudio` with M4 (playback), each with a line here.

## Custom widgets

None yet.
