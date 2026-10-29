# Pending screenshot checks

Visual checks that were deferred because their UI change was reviewed without the full set of screenshots. The HIG gate adds a row whenever it skips a size, colour scheme or state that the change affects. The full screenshot pass at the end of each milestone works through every row, records the result in that milestone's review (`docs/hig-reviews/mN.md`) and then deletes the row. A milestone cannot close while this table has rows.

| Added | Change (commit or files) | What to check | Skipped views | How to reach the state |
| --- | --- | --- | --- | --- |
| 2026-10-28 | M0, `app.rs` | Main menu popover: items, access keys, header capitalization | All four views | Click the main menu button, or press F10 |
| 2026-10-28 | M0, `ui/dialogs/about.rs` | About dialog: name, icon, version, licence, credits | 1440×900 light and dark; 360×640 (dialog becomes a bottom sheet) | Main menu → About Beacon |
| 2026-10-28 | M0, `data/shortcuts-dialog.ui` | Shortcuts dialog: section title, three shortcuts with their keys | 1440×900 light; 360×640 dark | Ctrl+? or Main menu → Keyboard Shortcuts |
