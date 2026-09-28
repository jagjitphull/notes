# Notes

A minimalist, local-first note-taking app for Linux (Pop!_OS / Ubuntu), aiming
for the speed and feel of Apple Notes.

## Stack

- **Frontend:** Vue 3 + TypeScript, via Tauri's WebView
- **Shell/backend:** Tauri 2 (Rust)
- **Storage:** SQLite (`rusqlite`, bundled), with FTS5 full-text search

## Project layout

```
frontend/     Vue + Vite web app (the UI)
src-tauri/    Rust backend: Tauri app shell, SQLite access, commands
  migrations/ SQL schema migrations, applied automatically on startup
```

The SQLite database lives in the OS-standard app-data directory at runtime
(e.g. `~/.local/share/dev.jagjitphull.notes/notes.sqlite3` on Linux), not in
the repo.

## Data model

- `folders` — id, name, parent_id (nested folders), sort_order
- `notes` — id, title, content (HTML), plaintext_content (for search),
  folder_id, is_pinned, deleted_at (soft delete / trash), timestamps
- `tags` / `note_tags` — many-to-many tagging
- `notes_fts` — FTS5 virtual table mirroring `notes(title, plaintext_content)`
  for instant full-text search, kept in sync via triggers

## Development

Requires Node.js, Rust, and (on Linux) the WebKitGTK/GTK3 dev packages that
Tauri needs to build:

```
sudo apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev pkg-config build-essential
```

```
npm install --prefix frontend
npm install
npm run dev     # launches the Tauri app with the Vite dev server
```

## Status

Phase 1 (scaffolding): project structure, build system, and SQLite schema
are in place. The app currently shows a single screen that confirms the
Rust ↔ SQLite ↔ WebView wiring works end to end.
