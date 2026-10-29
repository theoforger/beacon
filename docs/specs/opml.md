# OPML import and export

Milestone M7.

## Import

Offer it from the main menu (Import Subscriptions) and by dropping a file onto the Add page.

1. Choose the file with `GtkFileDialog`, which uses the portal inside Flatpak.
2. Parse with the `opml` crate. Walk nested outlines and flatten them. Read `xmlUrl` (tolerate attribute-case differences), and use `title` or `text` as the name. Skip outlines with no feed URL.
3. Normalise each URL (trim, lower-case the host) and remove duplicates within the file and against existing subscriptions.
4. Subscribe in the background, about four feeds at a time, each fetch validating that the URL is a real feed.
5. Show progress in a dialog, then a summary toast such as "Imported 41 of 44 podcasts". Failures stay listed with the reason and a Retry button.
6. Bad XML shows an error status page. It never crashes the app.

## Export

Main menu, Export Subscriptions, then a save dialog defaulting to `podcasts.opml`. Write OPML 2.0 with a head (title, creation date) and one `outline type="rss"` per subscription with `text`, `title`, `xmlUrl` and `htmlUrl`. Warn that private feed URLs may contain secret tokens before the file is shared.

## Tests

Keep fixture files exported from at least three other podcast apps. The round trip (export, re-import) must lose nothing.
