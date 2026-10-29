# Test strategy

Two rules drive everything else:

1. **Unit tests ship with the code.** Any change that adds or alters logic includes unit tests in the same change. No logic lands untested.
2. **Integration tests ship with the feature.** As soon as a feature ships, it gets an integration test of its main user scenario. A feature without one is not finished.

## Layers

| Layer | What it covers | How |
| --- | --- | --- |
| Unit | Parsing, season grouping, queue and auto-continue logic, played state, auto-delete rules, OPML mapping, relm4 `update` functions | `cargo test` in `beacon-core`, plus component `update` tests that send messages and assert state. No display needed |
| Integration | Feed fetch and refresh, SQLite migrations and queries, downloads and resume, iTunes client, and the main scenario of every feature | A local HTTP server serving fixture feeds and canned iTunes responses, in-memory SQLite, an injected clock for auto-delete. Feature scenarios send messages to the services and relm4 components and assert the resulting state and database rows. No display needed |

## No UI automation

The project does not run end-to-end tests through the real window. What the screen shows is checked by the HIG gate after every UI change (screenshots of the affected views, with skipped views logged and checked at milestone end; keyboard and Orca walk-throughs, deferred to a checkpoint after M3). To keep that gap small, keep logic out of widgets: components only translate messages into core calls and state into widget properties.

## Scenarios

The scenario tests for each milestone are listed under its Tests heading in `MILESTONES.md`.

## Rules for all tests

- CI never calls the real iTunes API or real podcast servers. Use recorded fixtures.
- One scheduled job may call the live iTunes API to check assumptions such as the `feedUrl` field. It must not block merges.
- Playback tests use a short local audio file and a silent output, never a real sound device.
- Unit and integration tests run on every push and pull request.
- A bug fix starts with a failing test that reproduces it.
