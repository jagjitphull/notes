-- Local semantic search: one row per note holding its embedding vector
-- (as raw little-endian f32 bytes) from a local Ollama model, plus a hash
-- of the text it was computed from so unchanged notes aren't re-embedded.
-- Entirely optional/rebuildable — if Ollama isn't installed this table
-- just stays empty and Smart Search stays hidden in the UI.
CREATE TABLE IF NOT EXISTS note_embeddings (
    note_id      TEXT PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
    model        TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    vector       BLOB NOT NULL,
    updated_at   TEXT NOT NULL
);
