# Pending screenshot checks

Visual checks that were deferred because their UI change was reviewed without the full set of screenshots. The HIG gate adds a row whenever it skips a size, colour scheme or state that the change affects. The full screenshot pass at the end of each milestone works through every row, records the result in that milestone's review (`docs/hig-reviews/mN.md`) and then deletes the row. A milestone cannot close while this table has rows.

| Added | Change (commit or files) | What to check | Skipped views | How to reach the state |
| --- | --- | --- | --- | --- |
| 2026-11-07 | `ui/selection_bar.rs` | Seen: New in selection mode at 360×640 light and dark and 1440×900 light; the count's menu in dark high contrast (2026-11-08). Still to check: a podcast page at about 520sp with the details pane open (icons fold) | 1100×800 with the details pane | Open a podcast page with the details pane open, press Select |
| 2026-11-07 | `ui/player_bar/compact.rs`, `app/window.rs` | Seen: the slim bar, the open sheet, high contrast with a long title, Nothing Playing (`m4.md`). Still to check by hand: resizing from narrow to wide with the sheet open and back; going wide puts the bar back under the content with nothing under it, and no freeze when the sidebar collapses on a long podcast page | Resizing by hand | Play an episode on a long podcast page, open the sheet at 360px, widen the window |
