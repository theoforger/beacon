-- SPDX-License-Identifier: GPL-3.0-or-later

-- How each podcast page lists its episodes, remembered per show.
ALTER TABLE shows ADD COLUMN episode_filter TEXT NOT NULL DEFAULT 'all';
ALTER TABLE shows ADD COLUMN sort_order TEXT NOT NULL DEFAULT 'newest';
