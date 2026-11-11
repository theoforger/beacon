# Add page and iTunes

Milestone M7. Add is a page that slides in over the current section, opened from the + in the sidebar header or the Add Podcast tile. The mockup's Add view is reference only. Build the real one around what the iTunes Search API returns, and drop mockup features it cannot support.

## What Apple documents

Read again 2026-11-10 from https://performance-partners.apple.com/search-api (the affiliate programme's page, which replaced the old developer.apple.com archive):

- Search: `https://itunes.apple.com/search` with `term` (required) and `country` ("The two-letter country code for the store you want to search", default US). Optional: `media` (`podcast`), `entity` (for podcasts `podcastAuthor`, `podcast`, `podcastEpisode`), `attribute` (for podcasts `titleTerm`, `languageTerm`, `authorTerm`, `genreIndex`, `artistTerm`, `ratingIndex`, `keywordsTerm`, `descriptionTerm`), `limit` (1 to 200, default 50), `lang` (`en_us` or `ja_jp`), `version`, `explicit` (default Yes).
- Lookup: `https://itunes.apple.com/lookup?id=…`; "ID-based lookups are faster and contain fewer false-positive results."
- Rate limit: "The Search API is limited to approximately 20 calls per minute (subject to change)." Apple adds: "Large websites should set up caching logic for the search and lookup requests sent to the Search API."
- Promotional content: developers may use "previews of songs, music videos, album art and App icons" "only to promote store content and not for entertainment purposes", and "Use of sound samples and other assets from the API must be proximate to a store badge." The legal section's conditions: placed only on pages that promote the content it is based on; near a "Download on iTunes" or "Download on App Store" badge linking to the store; "provided courtesy of iTunes" attribution only for song or music video previews; not used for its own entertainment value; not used to promote other goods.

What the page does not cover: podcasts. Its result table, its example and its legal section are about songs, music videos, albums and apps, and it speaks of search fields "in your website". Podcast artwork is not named, nor are native apps.

From the Apple Podcasts identity guidelines (https://marketing.services.apple/apple-podcasts-identity-guidelines), 5.4: "By default, Apple Podcasts makes the shows included within the directory available to third party apps and services." The rest of that page governs badges in podcasters' marketing (the "Listen on Apple Podcasts" badge, never altered, never implying Apple's endorsement).

## What it does not document

Genre browsing, top charts, and the `feedUrl` field that podcast results carry.

**Checked 2026-10-28 and again 2026-11-10:** `https://itunes.apple.com/search?term=hey+riddle+riddle&media=podcast&entity=podcast&country=US` returns `feedUrl` (`https://rss.art19.com/hey-riddle-riddle`), with `collectionId`, `collectionName`, `artistName`, `artworkUrl30` to `artworkUrl600`, `primaryGenreName`, `genres`, `trackCount`, `releaseDate`, `collectionExplicitness` and `collectionViewUrl` (the show's page on podcasts.apple.com). The response is served with `cache-control: max-age=86400`. A country Apple has no store for (`KP`, `AQ`, `ZZ`) answers 400. Some results have no `feedUrl`; they cannot be subscribed to and are left out.

## What other apps do

- **AntennaPod** (`net/discovery/ItunesPodcastSearcher.java`): `search?media=podcast&term=…`, no `country`; skips results without `feedUrl`; shows `artworkUrl100`, `collectionName` and `artistName`. A pasted podcasts.apple.com link (`…/id123`) is looked up by ID to find its feed. Top charts come from the old `itunes.apple.com/{country}/rss/toppodcasts` feed, then a lookup for each feed.
- **GNOME Podcasts** (`podcasts-data/src/discovery/itunes.rs`, `discovery_page.ui`, `discovery_found_podcast.ui`): `search?entity=podcast&limit=10`, no `country`; a boxed list of results with a 64px cover (`artworkUrl100`), title, author, episode count, last publication and a Subscribe button in the row. The page says "Search queries will be sent to these platforms" and lets the listener switch iTunes and fyyd on and off.

Both show Apple's artwork in results, without a badge.

## Decisions (2026-11-10)

1. **Search first.** Add is a search field (which still accepts a pasted RSS URL, or a podcasts.apple.com link) plus a results list. With no query it shows a status page, "Search for Podcasts", instead of Popular This Week.
2. **Categories and charts stay out.** Apple does not document them for podcasts. The old `toppodcasts` RSS feed and the RSS generator at rss.marketingtools.apple.com both answered on 2026-11-10, but neither is documented for third-party apps, and each result would need a lookup (another call against the 20 a minute) to find its feed. Revisit with a verified source.
3. **Result rows use the fields the API returns:** cover, title, author, and the genre and episode count. Results without `feedUrl` are dropped. A row opens a preview of the show (its feed, fetched, but not stored) with Subscribe; a show already subscribed opens its own page. A row has no Subscribe button of its own, unlike GNOME Podcasts: the preview shows what the listener subscribes to, and the row stays readable at 360px.
4. **Respect the rate limit.** Search on Enter or after a pause (800ms), never per keystroke. Results are cached in memory for a day, as Apple's `max-age` allows. A limiter allows 20 calls in any 60 seconds and, past that, says to wait rather than calling; a 403 or 429 from Apple says the same.
5. **Artwork.** Apple's terms do not name podcast art, and the conditions they set are for promoting store content on websites. The art in a result is the podcaster's own cover, which the feed carries too, and each result promotes only its own show, which Apple makes available to third-party apps by default. So results show Apple's 100px thumbnails (`artworkUrl100`, the size AntennaPod and GNOME Podcasts use), fetched into a scratch cache that is emptied at each start, never into the library's covers. The preview shows the feed's own cover once the feed has loaded, and a subscribed show only ever uses the feed's. The preview's More Options has View on Apple Podcasts (`collectionViewUrl`), the link to the store. No badge: the badges Apple names are for its own stores' downloads and for podcasters' marketing.
6. **Say where searches go.** Under the field, a line says that searches are sent to Apple Podcasts (as GNOME Podcasts says where its queries go), which is also the attribution.
7. **`country` is required.** It comes from the system locale (`LC_MESSAGES`, then `LANG`), falling back to US when the locale has no country or Apple has no store there. Preferences has a Directory Country or Region row: Automatic, or one of the countries that answered on 2026-11-10 (`STOREFRONTS` in `services/itunes.rs`).
8. **Apple Podcasts links.** A pasted `podcasts.apple.com/…/id123` or `itunes.apple.com/…/id123` link is looked up by ID, as AntennaPod does, and opens that show's preview.
9. **No explicit filter.** `explicit` stays at Apple's default; the row does not mark explicit shows, as the feed's own `itunes:explicit` is not shown elsewhere either.

## Search in the library

The search bar searches what is subscribed, with SQLite FTS5 (`shows_fts`, `episodes_fts`): matching shows as cover tiles under Shows, then matching episodes in one card. With no results, a status page with a suggested Search Online button that opens Add with the query. On a podcast page, a This Show / Everywhere toggle: This Show filters the page's own episodes in place (and has no Search Online); Everywhere shows the results as elsewhere.
