-- SPDX-License-Identifier: GPL-3.0-or-later

-- A podcast page also filters by download state, remembered per show.
ALTER TABLE shows ADD COLUMN download_filter TEXT NOT NULL DEFAULT 'all';
