# Pending screenshot checks

Visual checks that were deferred because their UI change was reviewed without the full set of screenshots. The HIG gate adds a row whenever it skips a size, colour scheme or state that the change affects. The full screenshot pass at the end of each milestone works through every row, records the result in that milestone's review (`docs/hig-reviews/mN.md`) and then deletes the row. A milestone cannot close while this table has rows.

| Added | Change (commit or files) | What to check | Skipped views | How to reach the state |
| --- | --- | --- | --- | --- |
| 2026-11-02 | M3, `ui/shows_grid.rs`, `ui/covers.rs`, `data/style.css` | Shows grid: same 12px gaps and square covers at an in-between width (two to four columns), badge on the cover's corner; placeholder covers now outlined, visible but not heavy in light and dark | 1440×900 dark; 360×640 light; a window around 700px wide; high contrast | Subscribe to fixtures served with `python3 -m http.server`, including one without a cover (`no-guid.xml`) |
| 2026-11-02 | M3, `ui/page_filters.rs` | Reset Sort and Filters greyed out while every choice is the default; on a podcast page as well as in New | Podcast page menu at 1440×900 light; New's menu at the default | Open the funnel menu on a podcast page with the default choices |
