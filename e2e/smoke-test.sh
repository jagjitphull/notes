#!/usr/bin/env bash
# Automated E2E smoke test: launches the real built app under Xvfb, drives
# it with xdotool (real X11 input events, not a WebDriver-simulated click -
# see the comment below on why), and asserts on the one thing that matters
# most reliably: what actually landed on disk.
#
# Why not a WebDriver-based test (tauri-driver + WebKitWebDriver)? That's
# the officially documented approach, and most of it worked: the app
# launches, the DOM is correct, and a direct window.__TAURI_INTERNALS__
# .invoke() call resolves fine. But the moment a *click* event triggers an
# invoke() through Vue's handler chain, the WebDriver "Execute Script"/
# "Element Click" command never returns - a reproducible hang, not a
# timing issue (confirmed with an 8s in-page timeout that never fired
# either). That looks like a tauri-driver/webkit2gtk-driver automation-mode
# deadlock, not a bug in this app. xdotool sidesteps it entirely by
# driving real X11 input rather than going through WebDriver at all.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NOTES_BINARY="${NOTES_BINARY:-$SCRIPT_DIR/../src-tauri/target/release/notes}"
DISPLAY_NUM="${E2E_DISPLAY:-:97}"
APP_ID="dev.jagjitphull.notes"

WORKDIR="$(mktemp -d)"
export HOME="$WORKDIR/home"
VAULT="$WORKDIR/vault"
CONFIG_DIR="$HOME/.config/$APP_ID"
# Deliberately NOT under WORKDIR, which cleanup() below removes on exit -
# these are the artifact CI (or a human) inspects after a failure.
SCREENSHOT_DIR="${E2E_SCREENSHOT_DIR:-$SCRIPT_DIR/screenshots}"
mkdir -p "$HOME" "$VAULT" "$CONFIG_DIR" "$SCREENSHOT_DIR"

XVFB_PID=""
APP_PID=""
FAILED=0

cleanup() {
  [ -n "$APP_PID" ] && kill "$APP_PID" >/dev/null 2>&1 || true
  [ -n "$XVFB_PID" ] && kill "$XVFB_PID" >/dev/null 2>&1 || true
  rm -rf "$WORKDIR"
}
trap cleanup EXIT

fail() {
  echo "FAIL: $1" >&2
  FAILED=1
}

if [ ! -x "$NOTES_BINARY" ]; then
  echo "error: no release binary at $NOTES_BINARY - build it first:" >&2
  echo "  npm run build --prefix frontend" >&2
  echo "  cargo build --release --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol" >&2
  exit 1
fi

# Pre-seed the vault choice so the app launches straight into the main UI -
# this is also what keeps the test from depending on the first-run screen's
# own click-through, which is exactly the kind of click->invoke() path the
# WebDriver approach above couldn't get through reliably; xdotool handles
# it fine, but skipping it keeps this script's assertions focused on the
# note-taking flow itself.
cat >"$CONFIG_DIR/config.json" <<EOF
{ "notes_root": "$VAULT" }
EOF

echo "Starting Xvfb on $DISPLAY_NUM"
Xvfb "$DISPLAY_NUM" -screen 0 1400x900x24 >"$WORKDIR/xvfb.log" 2>&1 &
XVFB_PID=$!
export DISPLAY="$DISPLAY_NUM"
sleep 1

echo "Launching $NOTES_BINARY"
"$NOTES_BINARY" >"$WORKDIR/app.log" 2>&1 &
APP_PID=$!

echo "Waiting for the app window"
for _ in $(seq 1 30); do
  if xdotool search --name "Notes" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
if ! xdotool search --name "Notes" >/dev/null 2>&1; then
  fail "app window never appeared"
  cat "$WORKDIR/app.log" >&2
  exit 1
fi
sleep 1
import -window root "$SCREENSHOT_DIR/01-launch.png"

echo "Creating a note"
xdotool mousemove 500 63
xdotool click --clearmodifiers 1
sleep 1
xdotool type --clearmodifiers --delay 20 "E2E Smoke Test Note"
xdotool key Return
xdotool type --clearmodifiers --delay 20 "Body line written by the automated smoke test."
sleep 1.5 # the editor's autosave is debounced by 500ms
import -window root "$SCREENSHOT_DIR/02-note-created.png"

NOTE_FILE=$(find "$VAULT" -iname "*.md" | head -1)
if [ -z "$NOTE_FILE" ]; then
  fail "no .md file appeared in the vault after creating a note"
else
  if ! grep -q "E2E Smoke Test Note" "$NOTE_FILE"; then
    fail "note file is missing its title: $NOTE_FILE"
  fi
  if ! grep -q "Body line written by the automated smoke test." "$NOTE_FILE"; then
    fail "note file is missing its body: $NOTE_FILE"
  fi
fi

echo "Searching for the note"
xdotool mousemove 108 68
xdotool click --clearmodifiers 1
xdotool type --clearmodifiers --delay 20 "Smoke Test"
sleep 1
import -window root "$SCREENSHOT_DIR/03-search.png"

if ! kill -0 "$APP_PID" >/dev/null 2>&1; then
  fail "app process is no longer running (crashed?)"
fi

if [ "$FAILED" -eq 0 ]; then
  echo "PASS"
  exit 0
else
  echo "Screenshots saved to $SCREENSHOT_DIR"
  exit 1
fi
