# Pending screenshot checks

Visual checks that were deferred because their UI change was reviewed without the full set of screenshots. The HIG gate adds a row whenever it skips a size, colour scheme or state that the change affects. The full screenshot pass at the end of each milestone works through every row, records the result in that milestone's review (`docs/hig-reviews/mN.md`) and then deletes the row. A milestone cannot close while this table has rows.

| Added | Change (commit or files) | What to check | Skipped views | How to reach the state |
| --- | --- | --- | --- | --- |
| 2026-11-02 | M3, `ui/page_filters.rs`, `data/style.css` | The filter dot: accent colour on the funnel's corner, not clipped; gone once every choice is back to the default | 1440×900 dark; 360×640 light and dark; podcast page at 1440×900; high contrast | New with Oldest First (setting `new-sort='oldest'`); a podcast page with Unplayed chosen in its filter menu |
