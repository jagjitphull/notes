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

## Packaging / installing

```
npm run build     # tauri build — produces a release .deb and .AppImage
```

Output lands in `src-tauri/target/release/bundle/`:

- **`deb/Notes_<version>_amd64.deb`** — the native, recommended install for
  Pop!_OS/Ubuntu: `sudo apt install ./Notes_<version>_amd64.deb` (apt
  resolves the `libwebkit2gtk-4.1-0`/`libgtk-3-0` runtime deps
  automatically). Installs to `/usr/bin/notes` with a proper `.desktop`
  entry and icon, so it shows up in the app launcher like any other app.
- **`appimage/Notes_<version>_amd64.AppImage`** — a portable, no-install
  option: `chmod +x` it and run it directly, on any distro.

Both were built and smoke-tested against this exact repo state: the `.deb`
installs cleanly via `apt`, and both it and the AppImage launch to the real
first-run screen.

The `.deb`'s dependencies (`libwebkit2gtk-4.1-0`, `libgtk-3-0`) are only
available on **Pop!_OS 24.04+ / Ubuntu 24.04+ (noble)** — 22.04 (jammy)
ships webkit2gtk 4.0, not 4.1, so `apt install` would fail to resolve them
there. The AppImage bundles its own webkit2gtk and works on either.

RPM isn't built here since `rpmbuild` isn't part of this toolchain and
isn't relevant on a Debian-based distro like Pop!_OS; Flatpak was left out
too, since between the native `.deb` and the portable AppImage there wasn't
a gap it would fill for this app's target platform — worth adding later if
cross-distro store distribution becomes a goal.

## Status

- **Phase 1** (scaffolding): done — build system, SQLite schema.
- **Phase 2** (layout): done — three-pane UI, dark/light theme (follows the
  OS, plus a manual override).
- **Phase 3** (editor): done — file-backed storage with debounced autosave,
  a TipTap rich-text editor (bold/italic/underline/strike, headings,
  checklists, lists, code blocks, blockquotes, basic image drag-and-drop),
  and right-click context menus for notes (pin/move/delete/restore) and
  folders (new subfolder/rename/delete).
- **Phase 4** (state management): effectively complete as a side effect of
  Phase 3 — folder clicks, note selection, and note editing are all backed
  by real data, not mocks.
- **Phase 5** (search & polish): done — FTS5-backed search, keyboard
  shortcuts (Ctrl+N, Ctrl+F, arrow-key list navigation), and Linux
  packaging (.deb + AppImage) are all in place.
