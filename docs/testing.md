# Test strategy

Two rules drive everything else:

1. **Unit tests ship with the code.** Any change that adds or alters logic includes unit tests in the same change. No logic lands untested.
2. **Integration tests ship with the feature.** As soon as a feature ships, it gets an integration test of its main user scenario. A feature without one is not finished.

## Layers

| Layer | What it covers | How |
| --- | --- | --- |
| Unit | Parsing, season grouping, queue and auto-continue logic, played state, auto-delete rules, OPML mapping, views' input handling | `cargo test` in `beacon-core`, plus view tests that `send` input and assert state. No display needed |
| Integration | Feed fetch and refresh, SQLite migrations and queries, downloads and resume, iTunes client, and the main scenario of every feature | A local HTTP server serving fixture feeds and canned iTunes responses, in-memory SQLite, an injected clock for auto-delete. Feature scenarios send messages to the services and views and assert the resulting state and database rows. No display needed |

## Layout tests

`ui/display_tests.rs` builds widgets on a real display and checks how they lay out: the narrowest window, rows, lists, the podcast header's fit, chapter ticks. Run them on a private display, `xvfb-run cargo test`: on a desktop session that is locked or idle the windows never lay out and the checks fail with zero sizes. `resize_cost` there is not a check but a timing probe, ignored by default: it lays real feeds out at every width and prints each frame's work (`BEACON_PROBE_FEEDS=dir xvfb-run cargo test -p beacon-gtk resize_cost -- --ignored --nocapture`, with `NAME.xml` and `NAME.xml.cover` in the folder).

## No UI automation

The project does not run end-to-end tests through the real window. What the screen shows is checked by the HIG gate after every UI change (screenshots of the affected views, with skipped views logged and checked at milestone end; keyboard and Orca walk-throughs, logged and deferred until the UI is final, in M9). To keep that gap small, keep logic out of widgets: views only translate messages into core calls and state into widget properties.

## Scenarios

The scenario tests for each milestone are listed under its Tests heading in `MILESTONES.md`.

## Rules for all tests

- CI never calls the real iTunes API or real podcast servers. Use recorded fixtures.
- `beacon-core/tests/live_sync.rs` (ignored by default) runs the sync scenario with two libraries against a real nextcloud-gpodder server, such as a local nc-gpodder-lite with a feed served by `python3 -m http.server`; its header lists the variables it needs. Run it by hand after changing the sync engine.
- One scheduled job may call the live iTunes API to check assumptions such as the `feedUrl` field. It must not block merges. It is `.github/workflows/itunes-live.yml` (Mondays, or by hand), running `cargo test -p beacon-core --test live_itunes -- --ignored`: six calls checking `feedUrl`, the thumbnail and store link, a lookup by ID, a 400 for a country without a directory, and three storefronts.
- Playback tests use a short local audio file and a silent output, never a real sound device.
- Unit and integration tests run on every push and pull request.
- A bug fix starts with a failing test that reproduces it.
