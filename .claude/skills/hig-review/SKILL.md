---
name: hig-review
description: Run Beacon's HIG gate on the current UI changes. Lists changed UI files, fetches the relevant GNOME HIG pages, checks the change against docs/hig-checklist.md and the fetched text, takes only the screenshots the change needs (logging skipped views; the full set at milestone end), and reports pass or fail with a quote for every finding. Use after every UI change, before reporting it as done.
---

# HIG review

The gate is a judgment review, not a linter. Every finding must quote the HIG page it relies on. Never cite the HIG from memory; if no fetched page supports a claim, drop it.

## 1. Find what changed

- `git status --short` and `git diff HEAD --stat`, limited to `crates/beacon-gtk/`, `data/*.ui`, `data/style.css` and `po/POTFILES.in`. If the user names files or a milestone, review those instead.
- Read each changed file. List every widget, control, menu, string and state that was added or changed.

## 2. Fetch the guideline pages

For each kind of element that changed, fetch the matching pages from https://developer.gnome.org/hig/ (through the web-fetch agent if there is no WebFetch tool). Typical pages:

| Element | Page |
| --- | --- |
| Header bars, header buttons | `patterns/containers/header-bars.html` |
| Sidebars, split views | `patterns/nav/sidebars.html`, `guidelines/adaptive.html` |
| Buttons, suggested and destructive styles | `patterns/controls/buttons.html` |
| Menus, main menu | `patterns/controls/menus.html` |
| Lists and rows | `patterns/containers/boxed-lists.html`, `patterns/containers/list-column-views.html` |
| Toggle groups | `patterns/controls/toggles.html` |
| Search | `patterns/nav/search.html` |
| Empty states | `patterns/feedback/placeholders.html` |
| Toasts, dialogs | `patterns/feedback/toasts.html`, `patterns/feedback/dialogs.html` |
| Text and capitalization | `guidelines/writing-style.html` |
| Keyboard | `guidelines/keyboard.html` |
| Accessibility | `guidelines/accessibility.html` |

If a URL has moved, find the page from the HIG index instead of guessing.

## 3. Check

1. Every item in `docs/hig-checklist.md` that applies to the change.
2. Anything else in the fetched pages that applies.
3. The accessibility and translation section of the checklist. Always: strings through `gettext`, new files in `po/POTFILES.in`, accessible label and tooltip on icon-only buttons, no fixed-width labels. The walk-throughs (Orca, keyboard reachability, exposed states, large text) are deferred until the accessibility checkpoint after M3: until then, add a row per change to `docs/hig-reviews/pending-accessibility.md` saying what to check and how, instead of running them. From M4 on, run them here.
4. The mockup in `docs/design/` and the Requirements in `CLAUDE.md`, for layout and behaviour.

## 4. Screenshots

Screenshots are costly, so take as few as the change needs, once it is finished. Never take them to check that a command worked.

**Per change.** Decide which views the change affects: size (1440×900, 360×640), colour scheme (light, dark) and state (a dialog, a popover, a narrow-mode page). Take the most informative one or two, for example `SIZES=360x640 SCHEMES=dark build-aux/screenshots.sh`, and look at them: alignment, spacing, truncation, dark-mode contrast, the collapsed layout. Compare with `docs/design/mockup-1440x900.png` or `docs/design/mockup-360x640.png`, and read `docs/design/mockup-source.html` for states the renders do not show.

Then add a row to `docs/hig-reviews/pending-screenshots.md` for every affected view you did not capture, and for states that cannot be reached headless. Be specific: "Queue row drag handle at 360×640 dark, row menu open" rather than "check queue". Nothing affected is left unchecked and unlogged.

**Milestone end** (the user asks for the milestone review, or this is the last change of a milestone). Run `build-aux/screenshots.sh` for all four views, then work through every row of `pending-screenshots.md`: reach each state, check it, record the result in `docs/hig-reviews/mN.md`, and delete the row. Report any rows that still need a human (Orca, interaction that cannot be driven headless). The milestone is not done while rows remain.

If the change is only reachable after interaction (a dialog, a popover, a narrow-mode page), say so and describe what still needs a human look.

## 5. Report

```
HIG review: PASS | FAIL

Files: …
Pages checked: <url>, <url>, …

Findings
1. [FAIL] <file:line> <what is wrong>
   HIG (<page url>): "<exact quote>"
   Fix: <what to change>
2. [PASS] …

Screenshots: <views taken>, <what was seen>
Deferred: <rows added to pending-screenshots.md and pending-accessibility.md, or none>
Needs a human: <Orca, keyboard walk-through, anything not visible headless>
```

Then fix every FAIL and run the review again until it is clean.
