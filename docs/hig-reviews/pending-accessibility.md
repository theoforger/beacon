# Pending accessibility checks

Accessibility checks are deferred until after M3. Until then, the HIG gate still requires accessible labels, tooltips and gettext in the code. The checks of how the UI behaves (Orca, keyboard walk-through, large text, exposed states) are logged here instead of run. The accessibility checkpoint between M3 and M4 works through every row, records the result in `docs/hig-reviews/a11y-checkpoint.md` and deletes the row. M4 cannot start while this table has rows. From M4 on, the checks run in each milestone's gate again.

| Added | Change (commit or files) | What to check | How |
| --- | --- | --- | --- |
| 2026-10-28 | M0, `app.rs` | Main menu button, menu items, status page title and description are read correctly | Orca |
| 2026-10-28 | M0, `ui/dialogs/about.rs`, `data/shortcuts-dialog.ui` | About and Shortcuts dialogs are read correctly; focus goes into the dialog and returns on close | Orca, keyboard |
| 2026-10-28 | M0, `app.rs` | Window at 360px with large text: header titles and status page do not clip | Large text (Settings → Accessibility) |
| 2026-10-28 | M2, `ui/sidebar.rs` | Sidebar items read as a list; selected item announced; badges read with their item; Add Podcast label read; Refresh Feeds read as unavailable in the main menu until M3 | Orca, keyboard |
| 2026-10-28 | M2, `ui/content.rs` | Back, Search and Episode Details toggles announce their pressed state; focus moves into the search entry when the bar opens and back when it closes | Orca, keyboard |
| 2026-10-28 | M2, `app.rs` | Narrow layout: Back from a pushed page returns focus to the sidebar item; Tab order sidebar → content → player bar | Keyboard |
| 2026-10-28 | M2, `ui/content.rs` | Empty-state descriptions wrap at 360px with large text | Large text |
| 2026-10-29 | M2, `ui/content.rs` | Back from a deeper page by keyboard: Alt+Left and the Back key wherever focus is; focus lands on the parent page afterwards; the Add page's Back is reachable with Tab | Keyboard |
| 2026-10-29 | M2, `ui/content.rs` | Refresh Feeds in the Shows and New headers: label read, reachable with Tab, absent from other views' focus order | Orca, keyboard |
| 2026-10-29 | M3, `ui/shows_grid.rs` | Tiles read as "Title by Author, N new"; the Add Podcast tile is reachable and activates with Enter | Orca, keyboard |
| 2026-10-29 | M3, `ui/widgets/episode_row.rs` | Rows read title, subtitle, "New" and "Played"; the play and menu buttons are reachable with Tab; Enter on a row opens the details; the row menu's items have access keys | Orca, keyboard |
| 2026-10-29 | M3, `ui/new_view.rs`, `ui/show_hero.rs` | Recency and season headers read as headings; the All / Unplayed toggle group announces the selected option; season expanders announce expanded or collapsed | Orca, keyboard |
| 2026-10-29 | M3, `ui/show_page.rs`, `app/library.rs` | Podcast page: hero title read as a heading; Unsubscribe dialog takes focus and Escape cancels; toasts are announced | Orca, keyboard |
| 2026-10-29 | M3, `ui/details_pane.rs` | Details pane: focus moves into it when it opens from a row, and back on close; at 360px with large text the title and subtitle wrap without clipping | Orca, keyboard, large text |

