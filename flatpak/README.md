# Flatpak packaging

`dev.jagjitphull.notes.yml` packages the same release binary the `.deb`/AppImage
targets use (built beforehand via `cargo tauri build`), rather than rebuilding
Rust/Node from source inside the Flatpak sandbox - that would need every Cargo
dependency vendored ahead of time, since `flatpak-builder` disallows network
access mid-build.

## Before building: the tray icon dependency

The app creates a real system tray icon at runtime (`src-tauri/src/lib.rs`) -
closing the window keeps it running in the tray, same as Dropbox's Linux
client. This links `libayatana-appindicator3` as a hard runtime dependency
(confirmed via the `.deb` build's `Depends:` line), and `org.gnome.Platform`
does not ship it. Without it, the packaged binary won't start.

The standard fix is the same one most Flatpak apps with a tray icon use:
pull in the appindicator recipe from the
[flathub/shared-modules](https://github.com/flathub/shared-modules) repo as a
git submodule, and reference its current appindicator module (search that
repo for "appindicator" - the exact JSON filename has moved before, so it's
worth confirming against the live repo rather than a filename hardcoded here)
as a `modules` entry in `dev.jagjitphull.notes.yml`, listed before the
`notes` module.

```
git submodule add https://github.com/flathub/shared-modules.git flatpak/shared-modules
```

## Building

```
cargo tauri build --bundles none   # produces src-tauri/target/release/notes
flatpak-builder --user --install build-dir flatpak/dev.jagjitphull.notes.yml
flatpak run dev.jagjitphull.notes
```
