# Test fixtures

| File | Source | Covers |
| --- | --- | --- |
| `seasons.xml` | Hand-written | iTunes and Podcasting 2.0 seasons, `MM:SS` and plain-seconds durations |
| `plain.xml`, `plain-updated.xml` | Hand-written | A show without seasons, and the same feed a week later for refresh |
| `chapters.xml` | Hand-written | Podlove inline chapters and a Podcasting 2.0 chapters URL under an unusual prefix |
| `no-guid.xml` | Hand-written | Items identified by enclosure URL, then by link |
| `malformed.xml`, `not-a-feed.html` | Hand-written | Parse errors |
| `magic-tavern.xml` | Excerpt of [Hello From The Magic Tavern](https://rss.art19.com/hello-from-magic-tavern), fetched 2026-10-28 | A real ART19 feed: `-0000` dates, `HH:MM:SS` durations, a legacy GUID from an earlier host, seasons only in titles |
| `hey-riddle-riddle.xml` | Excerpt of [Hey Riddle Riddle](https://rss.art19.com/hey-riddle-riddle), fetched 2026-10-28 | Full, bonus and trailer episodes, some without episode numbers |
| `destination-linux.xml` | Excerpt of [Destination Linux](https://feeds.fireside.fm/destinationlinux/rss), fetched 2026-10-28 | A real Fireside feed: Podcasting 2.0 chapter URLs, people tags and a transcript, single-digit-hour and `MM:SS` durations, a legacy WordPress GUID |
| `destination-linux-489-chapters.json` | [Chapters of episode 489](https://feeds.fireside.fm/destinationlinux/json/episodes/2ccead8f-d833-4919-a578-d1ca3b8e2b58/chapters), fetched 2026-10-28 | Podcasting 2.0 JSON chapters (12), for fetching chapters in M4 |

Hand-written fixtures are GPL-3.0-or-later and use fictional shows from the mockup. `{{BASE}}` is replaced with the test server's address.

The real-feed excerpts keep a few items of each published feed unchanged (the first line says which). Their text and metadata belong to the shows, and are included only to test parsing. None of the three has season tags or Podlove chapters, which is why the hand-written fixtures stay.

To check the parser against full feeds without committing them, download feeds into a folder and run `BEACON_REAL_FEEDS=/path/to/folder cargo test -p beacon-core -- --ignored`.
