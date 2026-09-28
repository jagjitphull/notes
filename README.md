# Notes

A minimalist, local-first note-taking app for Linux (Pop!_OS / Ubuntu), aiming
for the speed and feel of Apple Notes — with notes that sync across machines
by living as plain files in a folder you choose (Dropbox, pCloud, etc).

## Stack

- **Frontend:** Vue 3 + TypeScript, via Tauri's WebView
- **Shell/backend:** Tauri 2 (Rust)
- **Storage:** Markdown files (source of truth, safe to sync) + SQLite
  (`rusqlite`, bundled, local-only rebuildable search index with FTS5)

## How storage works

Notes are **Markdown files** inside a folder you pick on first run (e.g.
`~/Dropbox/Notes`). Folders in the app are real subdirectories; each note is
one `.md` file with a small YAML front matter block (id, tags, pinned,
timestamps) — the first line of the body is the note's title, same as Apple
Notes.

That folder is the only thing that needs to sync. SQLite lives outside it,
in the OS-standard local app-data directory, and holds nothing but a
rebuildable index for fast listing/search — it's rebuilt from the files on
every launch and whenever a background file-watcher notices a change (e.g.
another device syncing in an edit). This avoids the classic failure mode of
syncing a SQLite database file directly through Dropbox/pCloud: worst case
here is a stray "conflicted copy" file from a simultaneous edit, never
database corruption.

## Project layout

```
frontend/     Vue + Vite web app (the UI)
src-tauri/    Rust backend
  migrations/       SQL migrations for the local SQLite index
  src/config.rs     Persists the chosen notes-root path
  src/note_file.rs  Front matter <-> Markdown file parsing/serialization
  src/store.rs      Directory scan, index rebuild, note CRUD (file + DB)
  src/watcher.rs    Filesystem watcher -> re-index -> notify the frontend
  src/commands.rs   Tauri commands exposed to the frontend
```

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

On first launch you'll be asked where to store your notes (with quick-picks
for detected Dropbox/pCloud/Nextcloud/etc folders, or a local-only default).

## Status

- **Phase 1** (scaffolding): done — build system, SQLite schema.
- **Phase 2** (layout): done — three-pane UI, dark/light theme (follows the
  OS, plus a manual override).
- **Phase 3** (editor), in progress: file-backed storage, debounced
  autosave, and basic note/pin/delete actions are wired and working end to
  end against real files. Rich formatting (bold/headers/checklists/code
  blocks) is the remaining piece.
