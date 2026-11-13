# OPML import and export

Milestone M8.

## Import

Main menu, Import Subscriptions, opens a dialog (decided by the user 2026-11-13, replacing a file chooser straight from the menu and a drop target on the Add page).

1. The dialog first shows a drop box: drop an OPML file on it, or press Choose File…, which opens `GtkFileDialog` (the portal inside Flatpak).
2. Parse with `quick-xml`. Walk nested outlines and flatten them. Read `xmlUrl` (tolerate attribute-case differences), and use `title` or `text` as the name. Skip outlines with no feed URL.
3. Normalise each URL (trim, lower-case the host) and remove duplicates within the file and against existing subscriptions. A file that cannot be read, is not OPML, or has nothing new says so in a toast on the dialog, which stays on the drop box.
4. The dialog then lists the file's new podcasts, each with a check box (all checked), its cover, its title and its feed address, under a note of how many the library has already. Each feed is fetched in the background, about four at a time, with one progress bar under the dialog's header: its row takes the feed's own title and cover, or, when it fails, a warning with the reason and its check box cleared. Select All or Select None sits over the list; the header counts what is selected.
5. Import (unavailable while nothing is checked) closes the dialog and subscribes to the checked feeds, from the feeds already fetched, else fetching them. Shows opens, the progress in the thin bar under the content header ("12 of 44 podcasts"), the new shows appearing as they arrive; then a toast such as "Imported 41 of 44 podcasts".
6. Nothing in a file crashes the app.

## Export

Main menu, Export Subscriptions, opens a dialog listing every subscription the same way, all checked (decided by the user 2026-11-13). Export… (unavailable while nothing is checked) opens a save dialog defaulting to `podcasts.opml`, and writes the checked shows as OPML 2.0 with a head (title, creation date) and one `outline type="rss"` per show with `text`, `title`, `xmlUrl` and `htmlUrl`. There is no warning about private feed addresses (dropped by the user 2026-11-13): the list shows every address, and a private feed can be left out.

## Tests

Keep fixture files exported from at least three other podcast apps. The round trip (export, re-import) must lose nothing.
