# Pending accessibility checks

Accessibility checks are deferred until after M3. Until then, the HIG gate still requires accessible labels, tooltips and gettext in the code. The checks of how the UI behaves (Orca, keyboard walk-through, large text, exposed states) are logged here instead of run. The accessibility checkpoint between M3 and M4 works through every row, records the result in `docs/hig-reviews/a11y-checkpoint.md` and deletes the row. M4 cannot start while this table has rows. From M4 on, the checks run in each milestone's gate again.

| Added | Change (commit or files) | What to check | How |
| --- | --- | --- | --- |
| 2026-10-28 | M0, `app.rs` | Main menu button, menu items, status page title and description are read correctly | Orca |
| 2026-10-28 | M0, `ui/dialogs/about.rs`, `data/shortcuts-dialog.ui` | About and Shortcuts dialogs are read correctly; focus goes into the dialog and returns on close | Orca, keyboard |
| 2026-10-28 | M0, `main.rs` | Ctrl+? opens the Shortcuts dialog on a real keyboard (Ctrl+Q, Ctrl+W, F10 and menu arrow keys were verified headless) | Keyboard |
| 2026-10-28 | M0, `app.rs` | Window at 360px with large text: header titles and status page do not clip | Large text (Settings → Accessibility) |
