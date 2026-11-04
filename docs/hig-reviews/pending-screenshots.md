# Pending screenshot checks

Visual checks that were deferred because their UI change was reviewed without the full set of screenshots. The HIG gate adds a row whenever it skips a size, colour scheme or state that the change affects. The full screenshot pass at the end of each milestone works through every row, records the result in that milestone's review (`docs/hig-reviews/mN.md`) and then deletes the row. A milestone cannot close while this table has rows.

| Added | Change (commit or files) | What to check | Skipped views | How to reach the state |
| --- | --- | --- | --- | --- |
| 2026-11-03 | M4, `ui/widgets/episode_row.rs` | Row Play button: square hover background matching More Options beside it | Any row at 1440×900 light and dark, pointer over Play | Hover is not reachable headless (clicking plays); check by hand |
| 2026-11-03 | M4, `ui/show_hero.rs` | Podcast page header in high contrast; at a page width just under and over 610px (sidebar and details open) | 1440×900 dark high contrast; about 1000px wide with details open | Open a podcast page; toggle details |
| 2026-11-04 | M4, `ui/player_bar.rs` | Narrow player bar while playing: stacked (now playing, controls, seek), speed, sleep timer and volume left out, nothing clipped | 360×640 light and dark while an episode plays; 1440×900 dark | Play an episode, then narrow the window; headless, the sidebar page must be left first (the rows sit below a podcast page's header) |
