# E2E smoke test

`smoke-test.sh` launches the real built app under Xvfb and drives it with
`xdotool` (real X11 input events) to:

1. Create a note and type a title + body into it.
2. Confirm the note actually landed on disk, in the right vault folder,
   with the right content - not just that the UI *looked* right.
3. Search for it.
4. Confirm the app process is still alive (didn't crash).

Screenshots from each step are saved to `e2e/screenshots/` (gitignored).

## Running it locally

```
npm run build --prefix frontend
cargo build --release --manifest-path ../src-tauri/Cargo.toml --features tauri/custom-protocol
./smoke-test.sh
```

`NOTES_BINARY` overrides the binary path (default:
`src-tauri/target/release/notes`); `E2E_DISPLAY` overrides the Xvfb display
number (default `:97`); `E2E_SCREENSHOT_DIR` overrides where screenshots
land. The script uses its own throwaway `$HOME` (so it never touches your
real notes or app config) and cleans up the app/Xvfb processes on exit.

## Why `cargo build` needs `--features tauri/custom-protocol`

This isn't just an E2E-test quirk - it's a real gap this test caught. The
`tauri` crate only loads the bundled frontend (`frontendDist`) instead of
the Vite dev server (`devUrl`) when its `custom-protocol` feature is
enabled. `cargo tauri build`/`cargo tauri dev` manage that automatically;
a bare `cargo build` doesn't, and nothing in `src-tauri/Cargo.toml` turns
it on either. Without the flag, the built binary launches to a permanently
blank "could not connect to localhost" window - this is exactly what
`packaging/aur/PKGBUILD`'s `build()` step was doing before it was fixed
(see the README's "Flatpak, Snap, AUR" section).

## Why `xdotool` instead of WebDriver

Tauri's documented E2E approach is `tauri-driver` + WebKitWebDriver, driven
through a WebDriver client (WebdriverIO is the usual choice). That was the
first thing tried here, and most of it worked: the app launches under
`tauri-driver`, the DOM is exactly what's expected, and a direct
`window.__TAURI_INTERNALS__.invoke(...)` call from injected JS resolves
immediately. But the moment a **click** event triggers an `invoke()` call
through Vue's handler chain (e.g. clicking the first-run "use this folder"
button, or the "New Note" button), the WebDriver command driving that click
- `Element Click` or a plain `element.click()` inside `execute()` - never
returns. This reproduced consistently, including with an in-page
`setTimeout` fallback that never fired either, which rules out it just
being slow. That points to a `tauri-driver`/`webkit2gtk-driver`
automation-mode deadlock specific to this environment, not a bug in the
app - the same interaction (click, type, scroll) is completely reliable
when driven by real `xdotool` input instead, which was used throughout
this project's own manual verification all along.

If a future `tauri-driver`/`webkit2gtk-driver` release fixes this, the
WebDriver approach is worth revisiting - it gives proper DOM-level
assertions instead of fixed pixel coordinates, which is the main downside
of the `xdotool` approach here (the coordinates in `smoke-test.sh` assume
the app's current layout at a 1400x900 Xvfb screen; a layout change that
moves the "New Note" button or search box will need the coordinates
updated too).
