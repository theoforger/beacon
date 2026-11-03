# Accessibility checkpoint (after M3)

The walk-throughs deferred from M0 to M3: Orca, keyboard only, and large text at 360px. Each step below covers one or more rows of `pending-accessibility.md` (the **Rows** column gives the date and file of the row). Mark each step **Pass** or describe what went wrong; fixes get their own commits, and a step's rows are deleted from `pending-accessibility.md` once it passes. M4 cannot start until that table is empty.

## Setup

- Run the Flatpak: `flatpak kill com.theoforger.Beacon; flatpak run com.theoforger.Beacon`.
- Library: at least two shows, one with new episodes, and one show whose feed has season tags. The Adventure Zone has them: add `https://feeds.simplecast.com/cYQVc__c`. Mark one episode as played and set one filter on a podcast page so the dot shows.
- Orca: Super+Alt+S turns it on and off. Run part A and part B with Orca on, part C with it off.
- Keyboard only for parts A and B: no mouse after launch.

## A. Window, sidebar and menus (Orca and keyboard)

| # | Do | Expect | Rows | Result |
| --- | --- | --- | --- | --- |
| A1 | Launch. Tab through the sidebar header, then the sidebar | "Add Podcast" and "Main Menu" read as buttons; the five items read as a list; the selected item (Shows) announced as selected; each count read with its item ("Shows, 2") | 2026-10-28 `ui/sidebar.rs` | PASS, but the selection started at the refresh button in "Show" instead of the left sidebar. Tabbing does not go through the whole list, just the first one. And then I needed to use arrow keys. Although I'm not sure if any of these are intended |
| A2 | F10 (or Enter on Main Menu); arrow through it; Escape | Every item read; Refresh Feeds read as available; focus returns to the button | 2026-10-28 `app.rs`, `ui/sidebar.rs` | PASS |
| A3 | Main Menu → About Beacon; Tab through; Escape. Then Ctrl+? for Keyboard Shortcuts; Escape | Focus goes into each dialog and is read; after Escape, focus is back where it was | 2026-10-28 `about.rs`, `shortcuts-dialog.ui` | PASS |
| A4 | Keep tabbing from the sidebar to the content header, the content, then the player bar | Order: sidebar → content header → content → player bar; Refresh Feeds in the content header is read with its label in Shows and New, and is not in the Tab order of Queue, Favorites or Downloaded | 2026-10-28 `app.rs` (narrow), 2026-10-29 `ui/content.rs` (Refresh) | As mentioned above, the highlight starts on the main view's header bar, instead of sidebar |
| A5 | Toggle Search (Ctrl+F), type, Escape; toggle Episode Details from its header button | Search and Episode Details announce pressed and not pressed; focus moves into the search entry when it opens and back to the toggle when it closes | 2026-10-28 `ui/content.rs` | PASS |
| A6 | Ctrl+R twice (once with something new if you can) | The toast is announced: "No new episodes" or the count | 2026-10-29 `show_page.rs` (toasts), 2026-11-03 `app/library.rs` | PASS |
| A7 | Choose Queue, Favorites, Downloaded | Each empty state's title and description are read | 2026-10-28 `app.rs` (status page) | The empty state message is not announced until I navigate over to the main view on the right |

## B. Shows, podcast pages, New and the details pane (Orca and keyboard)

| # | Do | Expect | Rows | Result |
| --- | --- | --- | --- | --- |
| B1 | Shows: Tab into the grid; arrow keys between tiles | The focus ring frames the whole tile and moves with the arrows; tiles read "Title by Author, N new" (no "new" part when there are none); covers are not announced | 2026-10-29 and 2026-11-02 `shows_grid.rs`, 2026-11-03 `style.css` | PASS |
| B2 | Arrow to Add Podcast; Enter | The Add page opens; focus is in the search field and its label is read ("Search podcasts or paste an RSS feed address") | 2026-10-29 `shows_grid.rs`, 2026-11-02 `add_page.rs` | After adding an rss feed, I can't back out of the "Add Podcast" screen until i move the cursor out of the searchbox |
| B3 | On Add: Shift+Tab to Back; Enter. Open Add again; Alt+Left. Open a podcast page; press the Back key if you have one | Back is reachable with Tab; each way returns to Shows with focus on the tile you came from | 2026-10-29 `ui/content.rs` (Back), 2026-11-02 `add_page.rs` | The found round: Shift+Tab goes to the "Eposide Details" button, and then pressing again goes to back button. On the podcast page, tabbing can't get out of the episode list.  |
| B4 | Open a podcast page; Tab through the hero | The title is read as a heading; Unsubscribe and More Options are read | 2026-10-29 `show_page.rs` | PASS |
| B5 | Unsubscribe; Escape | The dialog takes focus and is read; Escape cancels and nothing is removed | 2026-10-29 `show_page.rs` | PASS |
| B6 | Sort and Filter button (Tab to it, Enter); arrow through the menu | The button reads "Sort and Filter", with "Filters are on" while the dot shows; every radio item read with checked or not; the two "All" items can be told apart (if not, say so: section labels would fix it) | 2026-10-29 `page_filters.rs`, 2026-11-02 `page_filters.rs` (description) | the two "All" items are both read as "All" |
| B7 | In the same menu: Reset Sort and Filters (R); reopen the menu | Reset is read with its access key, and announced as unavailable when nothing is set; after Reset the default items read as checked and the description is gone | 2026-11-02 `page_filters.rs` (Reset) | "Reset" simply cannot be reached via error key |
| B8 | Podcast page with seasons: Tab to a season header; Space | Season headers read as headings; expanders announce expanded and collapsed | 2026-10-29 `new_view.rs`, `show_hero.rs` | PASS |
| B9 | Tab into the episode list; move between rows | Each row reads title, subtitle, "New" when it has the dot and "Played" when played (the dimmed title still read in full); the play and More Options buttons are reachable with Tab; Enter on a row opens the details | 2026-10-29 `episode_row.rs`, 2026-11-03 `episode_row.rs` | The separator are read as "Middle dot", otherwise PASS |
| B10 | More Options on a row (Enter or Space); arrow through; press P (Mark as Played); open it again | The menu opens and every item is read with its access key; the second time it says Mark as Unplayed | 2026-10-29 and 2026-11-03 `episode_row.rs` | PASS |
| B11 | Enter on a row to open the details pane; Tab through it; close it with its toggle | Focus moves into the pane when it opens and returns to the row when it closes | 2026-10-29 `details_pane.rs` | Focus did not move into the view when open. It does not return either since I need to tab to the toggle button first, which is already outside of the panel |
| B12 | New: Tab through the header, then the list | Recency headers read as headings; Sort and Filter is reachable with Tab in New and absent in other sections; its menu reads like B6 | 2026-11-01 `ui/content.rs`, 2026-10-29 `new_view.rs` (headers) | Recency header is not read |

The 2026-10-29 `new_view.rs` row also asks about an All / Unplayed toggle group. That group became the filter menu on 2026-11-01, so B6 and B12 cover it.

## C. Large text at 360px (Orca off)

Turn on Settings → Accessibility → Seeing → Large Text. Make the window as narrow as it goes (360px).

| # | Do | Expect | Rows | Result |
| --- | --- | --- | --- | --- |
| C1 | Look at the sidebar page, then Shows, New and Queue | Header titles and empty-state titles and descriptions wrap or ellipsize; nothing is clipped or overlaps | 2026-10-28 `app.rs` (large text), 2026-10-28 `ui/content.rs` (descriptions) | PASS |
| C2 | Choose an item, then Back by keyboard (Alt+Left) | Focus returns to the sidebar item | 2026-10-28 `app.rs` (narrow) | Alt+Left does not work, but Esc does |
| C3 | Open an episode's details | It is a full page; title and subtitle wrap without clipping; Back returns to the list | 2026-10-29 `details_pane.rs` | PASS |

## Results

Walked through on 2026-11-03. Every pending row was covered; they are cleared from `pending-accessibility.md`, which now holds one recheck per fix below.

**Passed:** A2, A3, A5, A6, B1, B4, B5, B8, B10, C1, C3.

**Passed, with a note:**

- **A1, A4.** Tab enters the sidebar list once and the arrow keys move within it: GTK's list behaviour, as intended. Focus started on Refresh Feeds in the content header; it now starts in the sidebar.
- **A7.** An empty state is read when focus moves into the content, not when the section is chosen in the sidebar, where focus stays. This is how focus-driven screen reading works in GTK apps; no change.
- **B3, first part.** From the Add page's search field, Shift+Tab reaches Episode Details before Back: the header's own order, end to start. No change.

**Fixed (recheck rows added):**

- **B3, second part.** Tab could not leave a podcast page's episode list: `GtkListBox` tabbed through every row. Tab now leaves the list after the focused row.
- **B6.** The two "All" items were read alike. The menu's groups now have titles: Order, Progress, Downloads.
- **B9.** The " · " separators were read as "middle dot". Subtitles are given to screen readers with commas.
- **B11.** Focus stayed in the list when details opened. It now moves into the pane; Escape closes the pane and returns focus to the row.
- **B12.** Recency headers were not read: they are labels, which focus never reaches. Each list is now named after its group or season.
- **C2.** Alt+Left did not return to the sidebar in the narrow layout (Escape did). It does now, and closes the narrow details page.

**Not reproduced (recheck rows added):**

- **B2.** Going back after adding a feed worked headless: Alt+Left went from the new podcast page to Add, then to Shows, with focus in the search field.
- **B7.** R activated Reset Sort and Filters headless with a filter on. With every choice at its default the item is unavailable, so its access key does nothing, by design.

**Raised alongside the walk-through:** grid covers were still softer than the details pane's (now decoded at the size they are drawn), and a podcast page's rows repeated the show's name (now left out).

### Recheck (2026-11-03)

Everything passed except:

- **B9.** "Middle dot" was still read: Orca reads an unnamed row from its text, so naming the subtitle label changed nothing. Rows now have an accessible name (title, New or Played, subtitle with commas), checked through AT-SPI headless.
- **B2.** Escape did nothing in Add's search field (Alt+Left worked): a search entry keeps Escape for itself. Escape now clears the field, and in the empty field goes back.
- **Grid covers** were still softer than the details pane's: the decode size was rounded up to a multiple of 8, so covers were drawn slightly smaller than decoded. They are now decoded at exactly the drawn size (37.8 dB against a Lanczos resize of the source, up from 27.9 dB).

### Second recheck (2026-11-03)

- **B9.** The subtitle was read twice, once from the row's name and once, with "middle dot", from its label. The row's text parts now have the `Presentation` role, so only the name and the two buttons are exposed (checked through AT-SPI).

## Closed

The last rechecks (B2, B9) passed on 2026-11-03. `pending-accessibility.md` is empty; from M4 on, the walk-throughs run in each milestone's HIG gate.
