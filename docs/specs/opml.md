# OPML import and export

Milestone M8.

## Import

Offer it from the main menu (Import Subscriptions) and by dropping a file onto the Add page.

1. Choose the file with `GtkFileDialog`, which uses the portal inside Flatpak.
2. Parse with the `opml` crate. Walk nested outlines and flatten them. Read `xmlUrl` (tolerate attribute-case differences), and use `title` or `text` as the name. Skip outlines with no feed URL.
3. Normalise each URL (trim, lower-case the host) and remove duplicates within the file and against existing subscriptions.
4. Subscribe in the background, about four feeds at a time, each fetch validating that the URL is a real feed.
5. Show Shows, and the progress in the thin bar under the content header ("12 of 44 podcasts"), the new shows appearing as they arrive; then a summary toast such as "Imported 41 of 44 podcasts". While some podcasts are not added, a banner says how many, and its Show lists each with the reason, with Try Again and Dismiss (changed 2026-11-12 from a progress dialog, which the HIG advises against: "In general, progress windows are not recommended").
6. Bad XML, a file that is not OPML, and a file with nothing new are toasts. It never crashes the app.

## Export

Main menu, Export Subscriptions, then a save dialog defaulting to `podcasts.opml`. Write OPML 2.0 with a head (title, creation date) and one `outline type="rss"` per subscription with `text`, `title`, `xmlUrl` and `htmlUrl`. Warn that private feed URLs may contain secret tokens before the file is shared.

## Tests

Keep fixture files exported from at least three other podcast apps. The round trip (export, re-import) must lose nothing.
