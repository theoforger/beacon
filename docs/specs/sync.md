# Sync with nextcloud-gpodder

Milestone M6. Decided with the user on 2026-11-08, replacing "Sync: none for now". Beacon syncs subscriptions, play positions and played state with [nextcloud-gpodder](https://github.com/thrillfall/nextcloud-gpodder), a Nextcloud app that AntennaPod, Kasts and GNOME Podcasts (nightly) already speak. Downloads are not synced: they stay manual and local, and nextcloud-gpodder 3.13.3 and later ignore DOWNLOAD and DELETE actions anyway.

Only nextcloud-gpodder for now. The protocol sits behind a `SyncBackend` trait so that a gpodder.net-style backend (mygpo, oPodSync) can follow; see the end.

## The API

From the README, read on 2026-11-08. Everything is under `/index.php/apps/gpoddersync/`, with HTTP basic auth (user and app password).

| Call | Request | Response |
| --- | --- | --- |
| `GET subscriptions` | `?since=<unix seconds>` (optional) | `{"add": [feed URL], "remove": [feed URL], "timestamp": <unix>}` |
| `POST subscription_change/create` | `{"add": [...], "remove": [...]}` | `{"timestamp": <unix>}` |
| `GET episode_action` | `?since=<unix>`; `allActions=true` adds DOWNLOAD, DELETE and NEW | `{"actions": [action], "timestamp": <unix>}` |
| `POST episode_action/create` | a bare JSON array of actions | `{"timestamp": <unix>}` |

An action:

```json
{
  "podcast": "http://example.com/feed.rss",
  "episode": "http://example.com/files/s01e20.mp3",
  "guid": "s01e20-example-org",
  "action": "PLAY",
  "timestamp": "2009-12-12T09:00:00",
  "started": 15,
  "position": 120,
  "total": 500
}
```

- `podcast` is the feed URL and `episode` the enclosure URL. The server identifies an episode by `guid`, or by `episode` when there is no GUID. Send `guid` whenever there is one.
- `started`, `position` and `total` are seconds; -1 means unset.
- `since` and the response `timestamp` are Unix seconds. The server hands out the actions whose own `timestamp` is after `since` (nextcloud-gpodder and nc-gpodder-lite filter on `timestamp_epoch > since`), so an action dated before another device last fetched never reaches that device. An action's `timestamp` is a date-time string, UTC, with or without a zone (servers since 3.11.0 send one). Parse both.
- A 404 on these paths means the app is not installed on that server.
- The README's examples have trailing commas and mix `PLAY` with `play`. Send upper case and accept either.

## Logging in

Two ways, chosen with a Browser / Password toggle group, as in GNOME Podcasts:

1. **Browser (Nextcloud Login Flow v2).** `POST /index.php/login/v2` returns `{"poll": {"token", "endpoint"}, "login"}`. Open `login` in the browser (the OpenURI portal) and poll `endpoint` with the form field `token`. The poll returns 404 until the user approves, then 200 with `{"server", "loginName", "appPassword"}`. The token lasts 20 minutes. Poll every few seconds, and stop on Cancel.
2. **Password.** `GET /ocs/v2.php/core/getapppassword?format=json` with basic auth and `OCS-APIRequest: true` turns a real password into an app password. A 403 means the password already is an app password, so keep it as it is.

Beacon stores only an app password, never the real one, in the keyring through `oo7` (the Secret portal in the Flatpak). The server and user go in the database.

Log Out sends `DELETE /ocs/v2.php/core/apppassword` and then removes the account and keyring item, even if the request fails, as the Nextcloud docs advise. Log Out also clears the pending changes.

## Recording local changes

SQLite triggers record every change while an active account exists, so nothing depends on each call site remembering:

- inserting a show records `add`, deleting one records `remove`, and each cancels a pending opposite;
- a change to an episode's `position_secs` or `played` upserts one pending PLAY per episode: the first `started` stays, while `position`, `total` and the time update. Played sends `position = total`; `total` is the duration, or the furthest position seen when the duration is unknown. Marking unplayed records nothing and drops the pending PLAY. Episodes without an enclosure are not recorded.

Pending rows copy the feed URL, GUID and enclosure URL, so they survive an unsubscribe.

Changes applied from the server are written with a guard flag set, which the triggers check, so they are not sent back.

## A sync

One at a time. In order:

1. **Subscriptions.** Fetch the changes since the stored subscription timestamp. Subscribe to each add (already subscribed is fine) and remove each remove.
2. **Episodes.** Fetch the actions since the stored action timestamp and apply each one. Match by feed URL and GUID first, then by enclosure URL.
   - A position later than a pending local change wins, and an earlier one is ignored.
   - A play within 5 seconds of `total` marks the episode played.
   - Played is sticky: a partial play from elsewhere never un-plays an episode. It does move the position, so the episode resumes there if played again.
   - An action for a known show whose episode is not stored refreshes that show once, then retries. If it still does not match, it is logged and dropped.
   - An action already matching the episode (an echo of Beacon's own) changes nothing.
3. **Push** the pending subscriptions, then the pending episode actions. `guid` is sent only when it differs from the enclosure URL, Beacon's fallback for a feed without GUIDs. Each action is dated the later of when it changed and the server's `timestamp` from step 2, so it is never older than a `since` the server has already handed out.
4. **Clear** the pending rows recorded before the sync started. Store the timestamps the server returned in steps 1 and 2, less one second, as the next `since`, never the local clock, so a wrong clock cannot skip changes and a change another device sent in the same second is not missed.

**First sync** (no stored timestamps): fetch everything, then push every local subscription and a PLAY for each episode with a position or marked played, leaving out what the server already has. Subscriptions are merged as a union: the first sync never removes a local show, even one the server lists as removed. A played episode's change dates from when it was played; a position has no date, so the server's wins.

Outside the first sync, a pending local subscription change wins over the server's opposite one.

## When it runs

- after Refresh Feeds and the refresh at startup;
- after subscribing or unsubscribing;
- about 30 seconds after playback pauses or stops (debounced);
- on Sync Now;
- once when the window closes, for at most 5 seconds.

Never while signed out or with the Sync switch off.

## Preferences

A Sync page in the Preferences dialog. GNOME Podcasts uses the same rows in a separate navigation page from its main menu; Beacon keeps every setting in one dialog (`docs/decisions.md`).

- **Signed out:** the server (`AdwEntryRow`); Log In With, a Browser / Password `AdwToggleGroup`; for Password, the username (`AdwEntryRow`) and password (`AdwPasswordEntryRow`); and a suggested Log In button. While the browser flow waits it shows a spinner and Cancel.
- **Signed in:** the account (server and user) with Log Out; Sync (`AdwSwitchRow`); Last Synced; and Sync Now.

## Errors

`SyncError` variants, each with its own message in the UI: not configured, network, unauthorized (401; asks the user to log in again), app missing (404), another HTTP status, a response that does not parse, login expired (20 minutes passed), keyring unavailable, and library errors while applying changes. Sync Now and login show them as toasts. A background sync logs network errors and shows a toast only for errors the user can fix (unauthorized, app missing, keyring).

## Open questions

1. **Marking unplayed** is not synced. gpodder has no "unplayed" action; NEW exists, but only `allActions=true` returns it, and it is unclear whether AntennaPod and Kasts act on it. Check before adding.
2. ~~**Made-up GUIDs.**~~ Settled 2026-11-10: `guid` is sent only when it differs from the enclosure URL. Beacon's fallbacks after the enclosure URL (the link, the title) apply only to items without audio, which are never synced.
3. **Feed URL changes.** nextcloud-gpodder has no `update_urls`. A feed that moves stays subscribed under its old URL on the server.

## Later: gpodder.net-style servers

The same payloads, with these differences: subscriptions belong to a registered device (`/api/2/subscriptions/{user}/{device}.json`) and reach other devices through a sync group; auth is basic auth or a session cookie, with no app passwords; and POST responses carry `update_urls`, which should rewrite stored feed URLs. That means a second `SyncBackend`, device registration and a second login form. The engine, triggers, merge rules and most of the page stay as they are.
