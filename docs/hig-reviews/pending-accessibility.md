# Pending accessibility checks

Accessibility checks are deferred until after M3. Until then, the HIG gate still requires accessible labels, tooltips and gettext in the code. The checks of how the UI behaves (Orca, keyboard walk-through, large text, exposed states) are logged here instead of run. The accessibility checkpoint between M3 and M4 works through every row, records the result in `docs/hig-reviews/a11y-checkpoint.md` and deletes the row. M4 cannot start while this table has rows. From M4 on, the checks run in each milestone's gate again.

| Added | Change (commit or files) | What to check | How |
| --- | --- | --- | --- |
