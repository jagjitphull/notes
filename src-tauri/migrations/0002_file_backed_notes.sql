-- Re-architects storage so notes live as Markdown files under a
-- user-chosen "notes root" directory (safe to sync via Dropbox/pCloud/etc),
-- with folders mapping 1:1 to real subdirectories. SQLite becomes a
-- local-only, rebuildable index/cache over those files rather than the
-- source of truth. Written as a full redefinition rather than ALTERs
-- since no real user data predates this migration.

DROP TABLE IF EXISTS note_tags;
DROP TABLE IF EXISTS tags;
DROP TABLE IF EXISTS notes_fts;
DROP TRIGGER IF EXISTS notes_ai;
DROP TRIGGER IF EXISTS notes_ad;
DROP TRIGGER IF EXISTS notes_au;
DROP TABLE IF EXISTS notes;
DROP TABLE IF EXISTS folders;

-- id is the folder's path relative to the notes root ('' = the root
-- folder itself, shown to the user as "Notes").
CREATE TABLE folders (
    id        TEXT PRIMARY KEY,
    name      TEXT NOT NULL,
    parent_id TEXT REFERENCES folders(id) ON DELETE CASCADE
);

CREATE INDEX idx_folders_parent_id ON folders(parent_id);

CREATE TABLE notes (
    id                TEXT PRIMARY KEY,
    file_path         TEXT NOT NULL UNIQUE,
    title             TEXT NOT NULL DEFAULT '',
    plaintext_content TEXT NOT NULL DEFAULT '',
    folder_id         TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    is_pinned         INTEGER NOT NULL DEFAULT 0,
    deleted_at        TEXT,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);

CREATE INDEX idx_notes_folder_id  ON notes(folder_id);
CREATE INDEX idx_notes_updated_at ON notes(updated_at);
CREATE INDEX idx_notes_deleted_at ON notes(deleted_at);

CREATE TABLE tags (
    id   TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE note_tags (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag_id  TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (note_id, tag_id)
);

CREATE INDEX idx_note_tags_tag_id ON note_tags(tag_id);

CREATE VIRTUAL TABLE notes_fts USING fts5(
    title,
    plaintext_content,
    content = 'notes',
    content_rowid = 'rowid',
    tokenize = 'porter unicode61'
);

CREATE TRIGGER notes_ai AFTER INSERT ON notes BEGIN
    INSERT INTO notes_fts(rowid, title, plaintext_content)
    VALUES (new.rowid, new.title, new.plaintext_content);
END;

CREATE TRIGGER notes_ad AFTER DELETE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, plaintext_content)
    VALUES ('delete', old.rowid, old.title, old.plaintext_content);
END;

CREATE TRIGGER notes_au AFTER UPDATE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, plaintext_content)
    VALUES ('delete', old.rowid, old.title, old.plaintext_content);
    INSERT INTO notes_fts(rowid, title, plaintext_content)
    VALUES (new.rowid, new.title, new.plaintext_content);
END;

INSERT INTO folders (id, name, parent_id) VALUES ('', 'Notes', NULL);
