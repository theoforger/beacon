# HIG checklist

Rules found while auditing the mockup. Run through them after every UI change, together with the pages of https://developer.gnome.org/hig/ that cover the changed widgets. Quote the page; do not rely on memory.

## Layout and controls

- [ ] One suggested (accent) button per view at most.
- [ ] Outside header bars, a button has an icon or a label, not both.
- [ ] Header capitalization for titles, buttons, menu items and tooltips.
- [ ] At most two controls per row; rows that hold controls are not focusable themselves.
- [ ] Rows with a drag handle also offer Move Up and Move Down in their menu.
- [ ] Menus have 3 to 12 items and do not mix item types within a section.
- [ ] Circular buttons used sparingly.
- [ ] Pill buttons only for the suggested action.
- [ ] Empty states are `AdwStatusPage` with an icon, a title and one line of description.
- [ ] Search is a bar under the header, toggled from the header and by Ctrl+F.
- [ ] Layout works from 360px wide upwards; the 760sp breakpoint collapses to pages.
- [ ] Libadwaita named colours and style classes only; no hex values in CSS.

## Accessibility and translation

- [ ] Every user-visible string goes through gettext, including tooltips, accessible labels and status pages.
- [ ] Every icon-only button has an accessible label and a tooltip.
- [ ] State changes (pressed, selected, current) are exposed to assistive technology.
- [ ] A keyboard-only walk-through reaches every new control.
- [ ] No fixed-width labels; layouts survive longer translated text.
- [ ] Checked with Orca and with large text at 360px wide.

## Screenshots

- [ ] 1440×900 light, 1440×900 dark, 360×640 light, 360×640 dark, compared with the mockup in `docs/design/`.
