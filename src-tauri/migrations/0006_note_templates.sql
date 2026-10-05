-- A note flagged as a template is excluded from "All Notes"/folder views
-- the same way a deleted note is, and listed in a separate "Templates"
-- section instead. Creating a note "from" a template copies its current
-- body into a brand-new note - the two are unrelated after that.
ALTER TABLE notes ADD COLUMN is_template INTEGER NOT NULL DEFAULT 0;

CREATE INDEX idx_notes_is_template ON notes(is_template);
