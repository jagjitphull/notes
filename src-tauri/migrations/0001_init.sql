-- Core schema for the notes app.

CREATE TABLE IF NOT EXISTS folders (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    parent_id  TEXT REFERENCES folders(id) ON DELETE CASCADE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_folders_parent_id ON folders(parent_id);

CREATE TABLE IF NOT EXISTS notes (
    id                TEXT PRIMARY KEY,
    title             TEXT NOT NULL DEFAULT '',
    content           TEXT NOT NULL DEFAULT '',
    plaintext_content TEXT NOT NULL DEFAULT '',
    folder_id         TEXT REFERENCES folders(id) ON DELETE SET NULL,
    is_pinned         INTEGER NOT NULL DEFAULT 0,
    deleted_at        TEXT,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_notes_folder_id  ON notes(folder_id);
CREATE INDEX IF NOT EXISTS idx_notes_updated_at ON notes(updated_at);
CREATE INDEX IF NOT EXISTS idx_notes_deleted_at ON notes(deleted_at);

CREATE TABLE IF NOT EXISTS tags (
    id   TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS note_tags (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag_id  TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (note_id, tag_id)
);

CREATE INDEX IF NOT EXISTS idx_note_tags_tag_id ON note_tags(tag_id);

-- Full-text search over notes, kept in sync with the notes table via triggers.
CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
    title,
    plaintext_content,
    content = 'notes',
    content_rowid = 'rowid',
    tokenize = 'porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS notes_ai AFTER INSERT ON notes BEGIN
    INSERT INTO notes_fts(rowid, title, plaintext_content)
    VALUES (new.rowid, new.title, new.plaintext_content);
END;

CREATE TRIGGER IF NOT EXISTS notes_ad AFTER DELETE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, plaintext_content)
    VALUES ('delete', old.rowid, old.title, old.plaintext_content);
END;

CREATE TRIGGER IF NOT EXISTS notes_au AFTER UPDATE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, plaintext_content)
    VALUES ('delete', old.rowid, old.title, old.plaintext_content);
    INSERT INTO notes_fts(rowid, title, plaintext_content)
    VALUES (new.rowid, new.title, new.plaintext_content);
END;

-- Default folder so the app always has somewhere to put new notes.
INSERT INTO folders (id, name, parent_id, sort_order)
SELECT 'default', 'Notes', NULL, 0
WHERE NOT EXISTS (SELECT 1 FROM folders WHERE id = 'default');
