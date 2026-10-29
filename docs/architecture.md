# Architecture

See the Architecture and Project structure sections of `PLAN.md` for the full design. This file records what is actually built.

## Crates

- `beacon-core`: domain model, storage and services. No GTK, GLib or relm4. Empty at M0.
- `beacon-gtk`: the app (binary `beacon`). Depends on `beacon-core`, never the reverse.

## Current state (M0)

- `main.rs`: logging, gettext, GResource registration, `AdwApplication`, `app.quit` (Ctrl+Q), `window.close` (Ctrl+W).
- `app.rs`: the `App` component. An `AdwApplicationWindow` holding an `AdwNavigationSplitView` (sidebar and content, each an `AdwToolbarView` with its own `AdwHeaderBar`). An `AdwBreakpoint` at 760sp collapses the split view. The window size is saved in GSettings.
- `ui/dialogs/about.rs`: `AdwAboutDialog`.
- `data/shortcuts-dialog.ui`: loaded automatically by `AdwApplication`, which also provides `app.shortcuts` (Ctrl+?).
- `data/style.css`: loaded automatically from the resource base path `/com/theoforger/Beacon/`.
