# Add view and iTunes

Milestone M6. The mockup's Add view is reference only. Build the real one around what the iTunes Search API returns, and drop mockup features it cannot support.

## What Apple documents

From the Search API page, as read during the design:

- Search needs a `term` and a `country`. Optional parameters include `media`, `entity`, `attribute`, `limit` (1 to 200, default 50), `lang` and `explicit`.
- Results include the show name, artist, artwork URLs and explicitness. Lookup by iTunes ID is also available.
- The limit is about 20 calls per minute, and Apple says that may change.
- Apple restricts how promotional content such as artwork may be used: to promote store content, near a store badge.

## What it does not document

Genre browsing, top charts, and the `feedUrl` field that podcast results carry. Many apps rely on `feedUrl`, so confirm it in a live response before building on it. Subscribing then stays plain RSS.

**Checked 2026-10-28:** `https://itunes.apple.com/search?term=hey+riddle+riddle&media=podcast&entity=podcast&country=US` returned `feedUrl` (`https://rss.art19.com/hey-riddle-riddle`), as did the same search for Hello From The Magic Tavern. Still to check before M6: the terms and the rate limit.

## Design consequences

1. **Search first.** Add is a search field (which still accepts a pasted RSS URL) plus a results list. With no query it shows a status page, "Search for Podcasts", instead of Popular This Week.
2. **Categories and charts only if there is a source.** The tiles and Popular This Week stay out until a real source is found and tested. Apple's top-chart feeds are a candidate but unverified.
3. **Result rows use the fields the API actually returns.** Open a result to a show preview with Subscribe.
4. **Respect the rate limit.** Search on Enter or after a pause, never per keystroke, and cache results.
5. **Check the artwork terms** before showing Apple-supplied covers in results, or show the feed's own artwork once the show is subscribed.
6. **`country` is required.** Derive it from the system locale and make it a setting.

## Open

Check Apple's terms, attribution rules and rate limits before building on the API.
