# Test fixtures

| File | Source | Covers |
| --- | --- | --- |
| `tone-30s.ogg` | Generated with `gst-launch-1.0 audiotestsrc` (a 440 Hz sine, mono, Vorbis), GPL-3.0-or-later | 30 seconds of audio for the playback test: long enough that pausing a few seconds in is not "nearly finished" |
| `seasons.xml` | Hand-written | iTunes and Podcasting 2.0 seasons, `MM:SS` and plain-seconds durations |
| `plain.xml`, `plain-updated.xml` | Hand-written | A show without seasons, and the same feed a week later for refresh; one episode has its own image, the other the show's |
| `chapters.xml` | Hand-written | Podlove inline chapters and a Podcasting 2.0 chapters URL under an unusual prefix |
| `no-guid.xml` | Hand-written | Items identified by enclosure URL, then by link |
| `malformed.xml`, `not-a-feed.html` | Hand-written | Parse errors |
| `magic-tavern.xml` | Excerpt of [Hello From The Magic Tavern](https://rss.art19.com/hello-from-magic-tavern), fetched 2026-10-28 | A real ART19 feed: `-0000` dates, `HH:MM:SS` durations, a legacy GUID from an earlier host, seasons only in titles |
| `hey-riddle-riddle.xml` | Excerpt of [Hey Riddle Riddle](https://rss.art19.com/hey-riddle-riddle), fetched 2026-10-28 | Full, bonus and trailer episodes, some without episode numbers |
| `destination-linux.xml` | Excerpt of [Destination Linux](https://feeds.fireside.fm/destinationlinux/rss), fetched 2026-10-28 | A real Fireside feed: Podcasting 2.0 chapter URLs, people tags and a transcript, single-digit-hour and `MM:SS` durations, a legacy WordPress GUID |
| `destination-linux-489-chapters.json` | [Chapters of episode 489](https://feeds.fireside.fm/destinationlinux/json/episodes/2ccead8f-d833-4919-a578-d1ca3b8e2b58/chapters), fetched 2026-10-28 | Podcasting 2.0 JSON chapters (12), for fetching chapters in M4 |
| `itunes/search-recorded.json` | The iTunes Search API's answer to `term=hey riddle riddle&media=podcast&entity=podcast&country=US&limit=2`, fetched 2026-11-10 | A real podcast result, every field Apple returns |
| `itunes/search-mixed.json`, `itunes/search-empty.json` | Hand-written, in the recorded answer's shape | Results without a feed, a repeated feed, an episode among shows, an empty author and a bad date; no results |
| `adventure-zone.xml` | Excerpt of [The Adventure Zone](https://feeds.simplecast.com/cYQVc__c), fetched 2026-10-28 | The only real fixture with season tags (10 seasons, unnamed); a trailer and a bonus episode inside seasons without episode numbers; an episode numbered 0; an episode without artwork; single-digit days in dates |
| `opml/gnome-podcasts.opml` | Written as GNOME Podcasts' exporter writes (`podcasts-data/src/opml.rs`, commit a3f48c6) | OPML 2.0, `text` and `title` both the name, `htmlUrl` |
| `opml/antennapod.opml` | Written as AntennaPod's `OpmlWriter.java` writes (commit 98d67c6) | Android's XML declaration in single quotes, a two-digit-year `dateCreated`, an outline without `htmlUrl` |
| `opml/gpodder.opml` | Written as gPodder's `opml.py` `Exporter` writes (commit 629c652) | Feeds inside section outlines, the description in `text` and the name in `title` |
| `opml/pocket-casts.opml` | Written as Pocket Casts Android's `OpmlExporter.kt` writes (commit e371f66) | OPML 1.0, every feed inside one `feeds` outline, the name only in `text` |
| `opml/edge-cases.opml` | Hand-written | No version, attribute names in other letter cases, an address with spaces, a trailing slash or a fragment, a repeated feed, a feed that answers 404, Overcast-style episode outlines, a link and a `mailto:` address |
| `opml/malformed.opml` | Hand-written | A file cut off part way, for the error page |

The OPML files list the three hand-written shows at `{{BASE}}`. They were written by reading each app's export code rather than exported from the apps, with the shows' names in place of real ones.

Hand-written fixtures are GPL-3.0-or-later and use fictional shows from the mockup. `{{BASE}}` is replaced with the test server's address.

The real-feed excerpts keep a few items of each published feed unchanged (the first line says which). Their text and metadata belong to the shows, and are included only to test parsing. Only The Adventure Zone has season tags, and none of them has season names or Podlove chapters, which is why the hand-written fixtures stay.

To check the parser against full feeds without committing them, download feeds into a folder and run `BEACON_REAL_FEEDS=/path/to/folder cargo test -p beacon-core -- --ignored`.
