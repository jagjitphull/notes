use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use notify::RecommendedWatcher;
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use tauri::{AppHandle, Emitter, Manager};

use crate::db::DbState;
use crate::store;

pub struct WatcherState(pub Mutex<Option<Debouncer<RecommendedWatcher>>>);

/// (Re)starts the filesystem watcher on the notes root, so changes synced in
/// from another device (or made outside the app) get picked up live. Any
/// previously running watcher is dropped, stopping its background thread.
pub fn restart(app: &AppHandle, notes_root: PathBuf) {
    let watch_path = notes_root.clone();
    let app_handle = app.clone();

    let debouncer = new_debouncer(
        Duration::from_millis(600),
        move |res: DebounceEventResult| {
            if res.is_err() {
                return;
            }
            let db_state = app_handle.state::<DbState>();
            let conn = db_state.0.lock().expect("db mutex poisoned");
            if let Err(e) = store::full_rescan(&conn, &notes_root) {
                log::warn!("rescan after file change failed: {e}");
            }
            drop(conn);
            let _ = app_handle.emit("notes-changed", ());
        },
    );

    let mut debouncer = match debouncer {
        Ok(d) => d,
        Err(e) => {
            log::warn!("failed to start file watcher: {e}");
            return;
        }
    };

    if let Err(e) = debouncer
        .watcher()
        .watch(&watch_path, notify::RecursiveMode::Recursive)
    {
        log::warn!("failed to watch notes root {watch_path:?}: {e}");
    }

    let state = app.state::<WatcherState>();
    *state.0.lock().expect("watcher mutex poisoned") = Some(debouncer);
}
