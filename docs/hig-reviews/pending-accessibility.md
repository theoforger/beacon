# Pending accessibility checks

Accessibility checks are deferred until after M3. Until then, the HIG gate still requires accessible labels, tooltips and gettext in the code. The checks of how the UI behaves (Orca, keyboard walk-through, large text, exposed states) are logged here instead of run. The accessibility checkpoint between M3 and M4 works through every row, records the result in `docs/hig-reviews/a11y-checkpoint.md` and deletes the row. M4 cannot start while this table has rows. From M4 on, the checks run in each milestone's gate again.

| Added | Change (commit or files) | What to check | How |
| --- | --- | --- | --- |
| 2026-11-03 | M4, `ui/widgets/episode_row.rs`, `app/playback.rs` | A row's Play button, now working, is read as "Play", starts the episode with Enter or Space, and a second press pauses it; a playback error toast is announced | Orca, keyboard |
| 2026-11-03 | M4, `ui/show_hero.rs`, `ui/show_page.rs` | Podcast page: More Options in the header bar reachable with Tab and read with its label; its menu reads Mark All as Played or Unplayed as fits; the header's title still read as a heading in both layouts | Orca, keyboard |
| 2026-11-03 | M4, `beacon_core::text::markup`, `ui/details_pane.rs` | Description links reachable with Tab and read as links, Enter opens them in the browser; until the details pane's Play button (M4 step 3), opening details puts focus on the first link | Orca, keyboard |
| 2026-11-03 | M4, `ui/episode_list.rs` | Paged lists: Down arrow past the last loaded row loads the next page and keeps going; nothing is skipped or read twice | Orca, keyboard |
| 2026-11-03 | M4, `ui/show_hero.rs`, `ui/dialogs/description.rs` | The Read More link is reachable with Tab only when it shows and read as a link; Enter opens the dialog, which takes focus, reads its title (the show's) and the description, Escape closes it and focus returns to Read More | Orca, keyboard |
| 2026-11-03 | M4, `ui/links.rs` | In a description, the arrow keys move between links, and past the first or last link move on as before; each link is read as it gains focus. Descriptions are selectable: Shift with the arrows selects, and taking focus selects nothing | Orca, keyboard |
