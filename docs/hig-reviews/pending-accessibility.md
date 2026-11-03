# Pending accessibility checks

Accessibility checks are deferred until after M3. Until then, the HIG gate still requires accessible labels, tooltips and gettext in the code. The checks of how the UI behaves (Orca, keyboard walk-through, large text, exposed states) are logged here instead of run. The accessibility checkpoint between M3 and M4 works through every row, records the result in `docs/hig-reviews/a11y-checkpoint.md` and deletes the row. M4 cannot start while this table has rows. From M4 on, the checks run in each milestone's gate again.

| Added | Change (commit or files) | What to check | How |
| --- | --- | --- | --- |
| 2026-11-03 | Checkpoint A1, A4: `app/window.rs` | Recheck: at launch, focus is in the sidebar on Shows, not on Refresh Feeds | Keyboard |
| 2026-11-03 | Checkpoint B2: `ui/add_page.rs` | Recheck, with the exact keys: add a feed from Add, then try to go back. Not reproduced headless (Alt+Left went from the new podcast page to Add, then to Shows); if it still fails, note which page shows and where focus is | Keyboard |
| 2026-11-03 | Checkpoint B3: `ui/episode_list.rs` | Recheck: on a podcast page, Tab moves through the focused row's buttons and then out of the list; the arrow keys move between rows | Keyboard |
| 2026-11-03 | Checkpoint B6: `ui/page_filters.rs` | Recheck: Orca reads the section titles (Order, Progress, Downloads) or otherwise tells the two "All" items apart | Orca |
| 2026-11-03 | Checkpoint B7: `ui/page_filters.rs` | Recheck with a filter on: R activates Reset Sort and Filters (headless it did). With every choice at its default it is unavailable, so R does nothing | Keyboard |
| 2026-11-03 | Checkpoint B9: `ui/format.rs`, `ui/widgets/episode_row.rs`, `ui/details_pane.rs` | Recheck: row and details subtitles are read with pauses, not "middle dot" | Orca |
| 2026-11-03 | Checkpoint B11: `app/window.rs` | Recheck: opening details from a row moves focus into the pane; Escape closes it and focus is back on the row | Orca, keyboard |
| 2026-11-03 | Checkpoint B12: `ui/episode_list.rs` | Recheck: tabbing into a list in New reads its group ("Last Week"), and on a podcast page its season or "Episodes" | Orca |
| 2026-11-03 | Checkpoint C2: `app/window.rs` | Recheck at 360px: Alt+Left from a section returns to the sidebar; from the details page it returns to the list | Keyboard |
