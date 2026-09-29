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
  src/embeddings.rs Ollama client + cosine similarity for Smart Search
  src/watcher.rs    Filesystem watcher -> re-index -> notify the frontend
  src/commands.rs   Tauri commands exposed to the frontend
```

## Development

Requires **Node.js 22 or 24 (LTS)** — an `.nvmrc` is checked in, so
`nvm use` picks the right one; some dependencies (jsdom, Vitest) warn or
misbehave on odd-numbered "Current" releases like Node 23/25, which
aren't meant for long-term use. Also requires Rust, and (on Linux) the
WebKitGTK/GTK3 dev packages that Tauri needs to build:

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

## Testing Smart Search

Smart Search (semantic search, see Status below) needs a real
[Ollama](https://ollama.com) install to test — the rest of the app works
with nothing extra, but this one feature depends on an actual local
embedding model, so it can't be verified just by running the test suite.

1. **Install Ollama** and confirm it's actually running:
   ```
   curl -fsSL https://ollama.com/install.sh | sh
   curl http://127.0.0.1:11434/api/tags   # should return JSON, not a connection error
   ```
2. **Pull the embedding model** the app is hardcoded to use:
   ```
   ollama pull nomic-embed-text
   ```
3. **Run the app** (`npm run dev`, or a packaged build) and check:
   - A sparkle toggle appears in the search box, right of the input. The app
     checks Ollama's reachability on launch and then re-checks every ~15s
     for as long as it's open — so the toggle shows up shortly after Ollama
     becomes reachable, whether that's before or after you opened the app,
     with no restart needed either way.
   - Click the toggle, then search. The **first** search after opening the
     app is slower than the rest — every note gets embedded once (after
     that, only a note whose text actually changed gets re-embedded) — this
     is expected, not a bug.
   - The real test is **semantic, not keyword, matching**: write a note like
     *"Need to renew my passport before the trip"* and search *"travel
     documents"* — words that don't appear in the note at all. Regular
     search (toggle off) finds nothing; Smart Search (toggle on) should
     still surface it, because the embedding model understands the two are
     related even without shared words.
   - Stop Ollama (`sudo systemctl stop ollama`) while the app is open and
     search again: it should silently fall back to regular search — no
     crash, no error dialog — and the toggle should disappear within ~15s.
   - Restart Ollama (`sudo systemctl start ollama`): the toggle should
     reappear on its own within ~15s, with no app restart needed.

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
isn't relevant on a Debian-based distro like Pop!_OS.

### Flatpak, Snap, AUR

Manifests for wider cross-distro distribution live alongside the app rather
than in `src-tauri/target/`, since none of them are generated output:

- **`flatpak/dev.jagjitphull.notes.yml`** — packages the same release binary
  the `.deb`/AppImage targets use. See `flatpak/README.md` for the one
  build-time gap that needs addressing first (the system tray icon's
  `libayatana-appindicator3` dependency, which `org.gnome.Platform` doesn't
  ship).
- **`snap/snapcraft.yaml`** — same approach, via the `gnome` extension for
  the webview/GTK stack.
- **`packaging/aur/PKGBUILD`** — a `-git` VCS package (no tagged release
  exists yet to pin a source-tarball checksum against) that builds from
  source, per AUR convention.

None of these have been build-tested here: `flatpak-builder`, `snapcraft`,
and `makepkg` aren't available in this environment. The manifests follow
each format's standard conventions and their YAML/PKGBUILD syntax has been
validated, but an actual `flatpak-builder`/`snapcraft`/`makepkg` run (and,
for the AUR package, an upload + install by someone with an Arch machine)
is still needed before treating any of them as verified.

## Releasing (CI-built, signed, auto-updating)

Pushing a version tag builds, signs, and publishes installers for both
Linux and macOS, as a two-job matrix, via `.github/workflows/release.yml`:

```
git tag v0.2.0
git push origin v0.2.0
```

This creates a **draft** GitHub Release with the `.deb`, the `.AppImage`,
a universal (Apple Silicon + Intel) macOS `.dmg`, and their signatures
attached — review it and hit "Publish" when ready; nothing goes live
automatically. Once published, the app's built-in updater (checks on
launch, and a small "Update to vX.Y.Z" pill appears in the top bar when
one's found) picks it up automatically for anyone already running an
earlier version — no separate store or update server needed, it just
reads `.../releases/latest/download/latest.json`, which `tauri-action`
generates and attaches for you.

**macOS is unsigned** — no Apple Developer account (that's a paid,
identity-verified enrollment, not something that can be set up from
here), so the `.dmg` isn't code-signed or notarized. Gatekeeper will
refuse to open it with a normal double-click; the release notes tell
people to right-click the app in Finder and choose Open instead, which
only needs doing once. The build itself has not been tested on real
macOS hardware (this repo is developed on Linux) - only that the CI
job's config is valid; the first real tag push is effectively also its
first end-to-end test.

Signing only happens here, in CI — a plain local `npm run build` produces
an ordinary unsigned `.deb`/`.AppImage` (fine for testing the app itself)
without needing any key at all. `createUpdaterArtifacts` is deliberately
left out of `tauri.conf.json` and instead passed as a build arg in
`release.yml`, so routine local builds don't need the private key just
to run.

**One-time setup before the first release**: the release workflow signs
every build with a private key so the updater can verify it's really
this project publishing the update, not something injected in transit.
That key needs to exist as two repository secrets
(Settings → Secrets and variables → Actions):

- `TAURI_SIGNING_PRIVATE_KEY` — the private key
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — its password

The matching public key is already committed in
`src-tauri/tauri.conf.json` (`plugins.updater.pubkey`) — that half is
meant to be public, it's what lets the app verify updates, not what
protects anything. If you ever need to rotate the keypair (private key
compromised, lost, whatever), generate a fresh one with
`npx tauri signer generate -w <path>`, update both the pubkey in
`tauri.conf.json` and the two secrets, and every previously-installed
copy of the app will need to update **once** the old-fashioned way
(re-download) since it can no longer verify updates signed by a key it
doesn't know about.

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
  packaging (.deb + AppImage, plus Flatpak/Snap/AUR manifests) are all in
  place.
- **Post-launch polish**: a system tray icon (closing the window hides it
  rather than quitting, so the app keeps syncing in the background; Quit
  from the tray menu exits for real) and a 30-day auto-purge of Recently
  Deleted, matching Apple Notes' retention window.
- **Editor & layout extras**: a text highlighter mark (`==text==`,
  Obsidian-compatible, round-trips through Markdown); resizable and
  collapsible sidebar/note-list panes with drag handles and
  localStorage-persisted widths; and an in-editor tag UI (chip list with
  add/remove) backed by the existing YAML front-matter tags, including
  garbage collection of tag rows no longer referenced by any note.
- **Smart Search (optional, local AI/RAG)**: semantic search over note
  content via a local [Ollama](https://ollama.com) instance — nothing
  leaves the machine. Run `ollama pull nomic-embed-text` and keep Ollama
  running to enable it; a sparkle toggle then appears in the search box
  next to regular search. Notes are embedded lazily (only once actually
  searched, and only re-embedded when their text changes) and ranked by
  brute-force cosine similarity — no vector-index dependency, which is
  overkill for a personal notes vault. If Ollama isn't installed or
  running, the toggle simply doesn't appear and search behaves exactly as
  before; if it stops responding mid-session, Smart Search silently falls
  back to regular FTS5 search.
- **Hardening**: single-instance enforcement (opening the app while it's
  already running just focuses the existing window instead of starting a
  second process — matters here specifically because two processes would
  otherwise both open the same SQLite index and watch the same notes
  folder, racing each other).
- **Distribution**: CI (GitHub Actions) runs the full test suite —
  backend (`cargo fmt`, `clippy -D warnings`, `cargo test`) and frontend
  (`vue-tsc` + `vite build`, a Vitest suite covering the API bindings,
  both layout/theme composables, and the Smart Search toggle's
  availability/fallback behavior) — on every push and PR. Pushing a
  version tag builds, signs, and publishes a signed `.deb`/`.AppImage`
  (Linux) and an unsigned universal `.dmg` (macOS), which the app's
  built-in updater then picks up automatically for existing installs;
  see "Releasing" above.

## License

[MIT](LICENSE)
