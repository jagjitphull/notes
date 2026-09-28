use serde::Serialize;
use tauri::State;

use crate::db::DbState;

#[derive(Serialize)]
pub struct DbHealth {
    pub folder_count: i64,
    pub note_count: i64,
    pub tag_count: i64,
}

/// Sanity-checks that the SQLite database is initialized and reachable.
/// Used by the frontend to confirm the backend wiring during scaffolding.
#[tauri::command]
pub fn db_health_check(state: State<DbState>) -> Result<DbHealth, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;

    let folder_count = conn
        .query_row("SELECT COUNT(*) FROM folders", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let note_count = conn
        .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let tag_count = conn
        .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    Ok(DbHealth {
        folder_count,
        note_count,
        tag_count,
    })
}
