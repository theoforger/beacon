-- SPDX-License-Identifier: GPL-3.0-or-later

-- Sync with nextcloud-gpodder (docs/specs/sync.md).

-- The one sync account. Its app password is in the keyring, never here.
-- The `since` values are the server's own timestamps (Unix seconds).
CREATE TABLE sync_account (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    backend TEXT NOT NULL DEFAULT 'nextcloud',
    server TEXT NOT NULL,
    user TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1,
    subscriptions_since INTEGER,
    actions_since INTEGER,
    last_synced_at TEXT
);

-- Set while changes from the server are written, so the triggers below do
-- not send them back. One row; cleared whenever the library opens.
CREATE TABLE sync_guard (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    applying INTEGER NOT NULL DEFAULT 0
);
INSERT INTO sync_guard (id, applying) VALUES (1, 0);

-- Subscriptions to push. The latest change to a feed replaces an opposite
-- one still pending. `seq` orders changes, so a sync clears only those
-- recorded before it started.
CREATE TABLE sync_shows (
    feed_url TEXT PRIMARY KEY,
    action TEXT NOT NULL CHECK (action IN ('add', 'remove')),
    seq INTEGER NOT NULL
);

-- One pending PLAY per episode. The feed URL, GUID and enclosure URL are
-- copies, so a change survives unsubscribing. `changed_at` is Unix seconds.
CREATE TABLE sync_episodes (
    feed_url TEXT NOT NULL,
    guid TEXT NOT NULL,
    episode_url TEXT NOT NULL,
    started INTEGER NOT NULL,
    position INTEGER NOT NULL,
    total INTEGER NOT NULL,
    changed_at INTEGER NOT NULL,
    seq INTEGER NOT NULL,
    PRIMARY KEY (feed_url, guid)
);

CREATE TRIGGER sync_show_added AFTER INSERT ON shows
WHEN EXISTS (SELECT 1 FROM sync_account WHERE active)
    AND NOT (SELECT applying FROM sync_guard)
BEGIN
    INSERT OR REPLACE INTO sync_shows (feed_url, action, seq)
    VALUES (new.feed_url, 'add', (SELECT coalesce(max(seq), 0) + 1 FROM sync_shows));
END;

CREATE TRIGGER sync_show_removed AFTER DELETE ON shows
WHEN EXISTS (SELECT 1 FROM sync_account WHERE active)
    AND NOT (SELECT applying FROM sync_guard)
BEGIN
    INSERT OR REPLACE INTO sync_shows (feed_url, action, seq)
    VALUES (old.feed_url, 'remove', (SELECT coalesce(max(seq), 0) + 1 FROM sync_shows));
END;

-- Where an episode got to, or that it was played. Played sends the whole
-- length as the position; the length is the duration, or else the furthest
-- position seen. Marking unplayed is not synced (gpodder has no such
-- action), so it drops a pending change instead. Episodes without an audio
-- file have nothing to identify them on the server and are left out.
CREATE TRIGGER sync_episode_played AFTER UPDATE OF position_secs, played ON episodes
WHEN (old.position_secs IS NOT new.position_secs OR old.played IS NOT new.played)
    AND NOT (old.played AND NOT new.played)
    AND new.enclosure_url IS NOT NULL
    AND EXISTS (SELECT 1 FROM sync_account WHERE active)
    AND NOT (SELECT applying FROM sync_guard)
BEGIN
    INSERT INTO sync_episodes
        (feed_url, guid, episode_url, started, position, total, changed_at, seq)
    SELECT shows.feed_url, new.guid, new.enclosure_url, old.position_secs,
        CASE WHEN new.played
            THEN coalesce(new.duration_secs, max(new.position_secs, old.position_secs))
            ELSE new.position_secs END,
        coalesce(new.duration_secs, max(new.position_secs, old.position_secs)),
        CAST(strftime('%s', 'now') AS INTEGER),
        (SELECT coalesce(max(seq), 0) + 1 FROM sync_episodes)
    FROM shows WHERE shows.id = new.show_id
    ON CONFLICT (feed_url, guid) DO UPDATE SET
        episode_url = excluded.episode_url,
        position = CASE WHEN new.played
            THEN max(excluded.total, CASE WHEN new.duration_secs IS NULL
                THEN sync_episodes.total ELSE 0 END)
            ELSE excluded.position END,
        total = max(excluded.total, CASE WHEN new.duration_secs IS NULL
            THEN sync_episodes.total ELSE 0 END),
        changed_at = excluded.changed_at,
        seq = excluded.seq;
END;

CREATE TRIGGER sync_episode_unplayed AFTER UPDATE OF played ON episodes
WHEN old.played AND NOT new.played
    AND NOT (SELECT applying FROM sync_guard)
BEGIN
    DELETE FROM sync_episodes
    WHERE guid = new.guid
        AND feed_url = (SELECT feed_url FROM shows WHERE id = new.show_id);
END;
