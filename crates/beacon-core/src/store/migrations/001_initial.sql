-- SPDX-License-Identifier: GPL-3.0-or-later

CREATE TABLE shows (
    id INTEGER PRIMARY KEY,
    feed_url TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    author TEXT,
    description TEXT,
    link TEXT,
    image_url TEXT,
    category TEXT,
    cover_path TEXT,
    subscribed_at TEXT NOT NULL,
    last_refreshed_at TEXT
);

CREATE TABLE episodes (
    id INTEGER PRIMARY KEY,
    show_id INTEGER NOT NULL REFERENCES shows (id) ON DELETE CASCADE,
    guid TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    published_at TEXT,
    duration_secs INTEGER,
    enclosure_url TEXT,
    enclosure_type TEXT,
    enclosure_length INTEGER,
    season_number INTEGER,
    season_name TEXT,
    episode_number REAL,
    image_url TEXT,
    chapters_url TEXT,
    seen INTEGER NOT NULL DEFAULT 0,
    played INTEGER NOT NULL DEFAULT 0,
    played_at TEXT,
    position_secs INTEGER NOT NULL DEFAULT 0,
    favorite INTEGER NOT NULL DEFAULT 0,
    download_path TEXT,
    downloaded_at TEXT,
    added_at TEXT NOT NULL,
    UNIQUE (show_id, guid)
);

CREATE INDEX episodes_by_show ON episodes (show_id, published_at);
CREATE INDEX episodes_by_date ON episodes (published_at);

CREATE TABLE chapters (
    episode_id INTEGER NOT NULL REFERENCES episodes (id) ON DELETE CASCADE,
    start_ms INTEGER NOT NULL,
    title TEXT NOT NULL,
    url TEXT,
    image_url TEXT,
    PRIMARY KEY (episode_id, start_ms)
);

CREATE TABLE queue (
    episode_id INTEGER PRIMARY KEY REFERENCES episodes (id) ON DELETE CASCADE,
    position INTEGER NOT NULL
);

-- Full-text search over shows and episodes, kept in sync by triggers.
CREATE VIRTUAL TABLE shows_fts USING fts5 (
    title, author, description, content = 'shows', content_rowid = 'id'
);
CREATE TRIGGER shows_fts_insert AFTER INSERT ON shows BEGIN
    INSERT INTO shows_fts (rowid, title, author, description)
    VALUES (new.id, new.title, new.author, new.description);
END;
CREATE TRIGGER shows_fts_delete AFTER DELETE ON shows BEGIN
    INSERT INTO shows_fts (shows_fts, rowid, title, author, description)
    VALUES ('delete', old.id, old.title, old.author, old.description);
END;
CREATE TRIGGER shows_fts_update AFTER UPDATE OF title, author, description ON shows BEGIN
    INSERT INTO shows_fts (shows_fts, rowid, title, author, description)
    VALUES ('delete', old.id, old.title, old.author, old.description);
    INSERT INTO shows_fts (rowid, title, author, description)
    VALUES (new.id, new.title, new.author, new.description);
END;

CREATE VIRTUAL TABLE episodes_fts USING fts5 (
    title, description, content = 'episodes', content_rowid = 'id'
);
CREATE TRIGGER episodes_fts_insert AFTER INSERT ON episodes BEGIN
    INSERT INTO episodes_fts (rowid, title, description)
    VALUES (new.id, new.title, new.description);
END;
CREATE TRIGGER episodes_fts_delete AFTER DELETE ON episodes BEGIN
    INSERT INTO episodes_fts (episodes_fts, rowid, title, description)
    VALUES ('delete', old.id, old.title, old.description);
END;
CREATE TRIGGER episodes_fts_update AFTER UPDATE OF title, description ON episodes BEGIN
    INSERT INTO episodes_fts (episodes_fts, rowid, title, description)
    VALUES ('delete', old.id, old.title, old.description);
    INSERT INTO episodes_fts (rowid, title, description)
    VALUES (new.id, new.title, new.description);
END;
