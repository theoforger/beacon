# Pending screenshot checks

Visual checks that were deferred because their UI change was reviewed without the full set of screenshots. The HIG gate adds a row whenever it skips a size, colour scheme or state that the change affects. The full screenshot pass at the end of each milestone works through every row, records the result in that milestone's review (`docs/hig-reviews/mN.md`) and then deletes the row. A milestone cannot close while this table has rows.

| Added | Change (commit or files) | What to check | Skipped views | How to reach the state |
| --- | --- | --- | --- | --- |
