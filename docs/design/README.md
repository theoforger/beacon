# Design reference

The Claude Design mockup of Beacon, exported on 2026-10-28.

| File | What it is |
| --- | --- |
| `mockup.html` | The exported, self-contained mockup. Open it in a browser; it is interactive |
| `mockup-source.html` | The mockup's markup, styles and template logic, decoded from the bundle so it can be read and searched. It does not run on its own |
| `mockup-1440x900.png`, `mockup-360x640.png` | Renders of the opening state, for comparison with `build-aux/screenshots.sh`. Regenerate with `build-aux/mockup-screenshots.sh` |

The mockup is the visual reference. Where it conflicts with `CLAUDE.md` or `docs/specs/` (for example the Add view, which must follow what the iTunes Search API returns), those win. Its show names, covers and episode text are sample data.
