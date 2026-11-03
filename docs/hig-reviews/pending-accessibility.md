# Pending accessibility checks

Accessibility checks are deferred until after M3. Until then, the HIG gate still requires accessible labels, tooltips and gettext in the code. The checks of how the UI behaves (Orca, keyboard walk-through, large text, exposed states) are logged here instead of run. The accessibility checkpoint between M3 and M4 works through every row, records the result in `docs/hig-reviews/a11y-checkpoint.md` and deletes the row. M4 cannot start while this table has rows. From M4 on, the checks run in each milestone's gate again.

| Added | Change (commit or files) | What to check | How |
| --- | --- | --- | --- |
| 2026-11-03 | Checkpoint B2: `ui/add_page.rs` | Recheck: in Add's search field, Escape clears the text, and a second Escape (empty field) goes back | Keyboard |
| 2026-11-03 | Checkpoint B9: `ui/widgets/episode_row.rs` | Recheck: a row is read as its title, "New" or "Played", then the subtitle with pauses; no "middle dot". AT-SPI gives Orca e.g. "The lost ledger, New, Paper Trails, Oct 26" | Orca |
