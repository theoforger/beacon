# Auto-delete of old downloads

Milestone M5. The one feature not in the mockup. Agreed design:

- Preferences gets a Downloads group made of stock libadwaita rows (`AdwSwitchRow`, `AdwSpinRow`).
- **Delete Old Downloads** is a switch and the master control. It is off by default, and the rows below are disabled while it is off.
- **Delete After** is a spin row, in days.
- **Only Delete Played Episodes** is a switch, on by default. On: the clock starts when the episode was played. Off: it starts when the download finished.
- Episodes in the queue are never deleted. That is fixed, not a setting.
- Cleanup runs at startup and once a day while the app is open, and shows a toast when it removes something.
- Nothing is ever downloaded automatically.

## Tests

Unit tests for the rules with an injected clock; an integration test where cleanup removes an old download and keeps a queued one.
