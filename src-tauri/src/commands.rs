use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::config::{self, AppConfig};
use crate::db::DbState;
use crate::{store, watcher};

pub struct NotesRootState(pub Mutex<Option<PathBuf>>);

#[derive(Serialize)]
pub struct DbHealth {
    pub folder_count: i64,
    pub note_count: i64,
    pub tag_count: i64,
}

/// Sanity-checks that the SQLite database is initialized and reachable.
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

#[tauri::command]
pub fn get_notes_root(state: State<NotesRootState>) -> Option<String> {
    state
        .0
        .lock()
        .expect("notes root mutex poisoned")
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
}

#[derive(Serialize)]
pub struct CloudFolderSuggestion {
    pub name: String,
    pub path: String,
}

/// Looks for common cloud-sync folders under the user's home directory, so
/// first-run setup can offer them as one-click picks.
#[tauri::command]
pub fn detect_cloud_folders(app: AppHandle) -> Vec<CloudFolderSuggestion> {
    let Some(home) = app.path().home_dir().ok() else {
        return Vec::new();
    };

    [
        ("Dropbox", "Dropbox"),
        ("pCloud Drive", "pCloud Drive"),
        ("Nextcloud", "Nextcloud"),
        ("Google Drive", "Google Drive"),
        ("OneDrive", "OneDrive"),
    ]
    .into_iter()
    .filter_map(|(name, dir)| {
        let path = home.join(dir);
        path.is_dir().then(|| CloudFolderSuggestion {
            name: name.to_string(),
            path: path.to_string_lossy().to_string(),
        })
    })
    .collect()
}

/// Points the app at a (possibly new) notes root: persists it to config,
/// creates the directory if needed, rebuilds the SQLite index from whatever
/// Markdown files are already there, and (re)starts the file watcher.
#[tauri::command]
pub fn set_notes_root(
    app: AppHandle,
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    path: String,
) -> Result<(), String> {
    let path = PathBuf::from(path);
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;

    let app_config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?;
    config::save(
        &app_config_dir,
        &AppConfig {
            notes_root: Some(path.clone()),
        },
    )
    .map_err(|e| e.to_string())?;

    {
        let conn = db_state.0.lock().map_err(|e| e.to_string())?;
        store::full_rescan(&conn, &path)?;
        store::purge_expired_trash(&conn, &path, store::TRASH_RETENTION_DAYS)?;
    }

    *root_state.0.lock().map_err(|e| e.to_string())? = Some(path.clone());
    watcher::restart(&app, path);

    Ok(())
}

fn require_notes_root(state: &State<NotesRootState>) -> Result<PathBuf, String> {
    state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "notes root is not configured yet".to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderDto {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
}

#[tauri::command]
pub fn list_folders(db_state: State<DbState>) -> Result<Vec<FolderDto>, String> {
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, parent_id FROM folders ORDER BY name")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(FolderDto {
                id: r.get(0)?,
                name: r.get(1)?,
                parent_id: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteListItemDto {
    pub id: String,
    pub title: String,
    pub plaintext_content: String,
    pub folder_id: String,
    pub tag_ids: Vec<String>,
    pub is_pinned: bool,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

const NOTE_LIST_ITEM_COLUMNS: &str = "n.id, n.title, n.plaintext_content, n.folder_id, n.is_pinned,
     n.deleted_at, n.updated_at,
     COALESCE((SELECT GROUP_CONCAT(nt.tag_id) FROM note_tags nt WHERE nt.note_id = n.id), '')";

fn map_note_list_item(r: &rusqlite::Row) -> rusqlite::Result<NoteListItemDto> {
    let tag_ids_raw: String = r.get(7)?;
    Ok(NoteListItemDto {
        id: r.get(0)?,
        title: r.get(1)?,
        plaintext_content: r.get(2)?,
        folder_id: r.get(3)?,
        is_pinned: r.get::<_, i64>(4)? != 0,
        deleted_at: r.get(5)?,
        updated_at: r.get(6)?,
        tag_ids: if tag_ids_raw.is_empty() {
            Vec::new()
        } else {
            tag_ids_raw.split(',').map(str::to_string).collect()
        },
    })
}

#[tauri::command]
pub fn list_notes(db_state: State<DbState>) -> Result<Vec<NoteListItemDto>, String> {
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(&format!("SELECT {NOTE_LIST_ITEM_COLUMNS} FROM notes n"))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], map_note_list_item)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Searches notes via the FTS5 index (title + full body), ranked by
/// relevance. Each whitespace-separated word becomes its own quoted prefix
/// term ("word"*) so results update sensibly as the user keeps typing, and
/// so free-form query text can't be misread as FTS5 query syntax (column
/// filters, boolean operators, etc).
#[tauri::command]
pub fn search_notes(db_state: State<DbState>, query: String) -> Result<Vec<NoteListItemDto>, String> {
    let fts_query = store::build_fts_query(&query);
    if fts_query.is_empty() {
        return Ok(Vec::new());
    }

    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {NOTE_LIST_ITEM_COLUMNS}
             FROM notes n
             JOIN notes_fts ON notes_fts.rowid = n.rowid
             WHERE notes_fts MATCH ?1
             ORDER BY bm25(notes_fts)"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([fts_query], map_note_list_item)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Fast reachability check so the frontend can decide whether to show the
/// Smart Search toggle at all — Ollama not being installed/running is the
/// expected common case, not an error, so this returns a plain bool rather
/// than surfacing a failure.
#[tauri::command]
pub fn smart_search_available() -> bool {
    crate::embeddings::is_available()
}

/// Semantic search over note content via local Ollama embeddings, ranked
/// by cosine similarity (see `store::smart_search`). Errors here (almost
/// always: Ollama isn't reachable) are meant to be caught by the frontend
/// and silently fall back to `search_notes`.
#[tauri::command]
pub fn smart_search(db_state: State<DbState>, query: String) -> Result<Vec<NoteListItemDto>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    let ranked_ids = store::smart_search(&conn, trimmed, 30)?;
    if ranked_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = ranked_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("SELECT {NOTE_LIST_ITEM_COLUMNS} FROM notes n WHERE n.id IN ({placeholders})");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let query_params: Vec<&dyn rusqlite::ToSql> =
        ranked_ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
    let rows = stmt
        .query_map(query_params.as_slice(), map_note_list_item)
        .map_err(|e| e.to_string())?;

    let mut by_id: std::collections::HashMap<String, NoteListItemDto> = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|dto| (dto.id.clone(), dto))
        .collect();

    // Re-apply the ranking: the SQL above came back in arbitrary IN(...) order.
    Ok(ranked_ids.into_iter().filter_map(|id| by_id.remove(&id)).collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDto {
    pub id: String,
    pub name: String,
}

#[tauri::command]
pub fn list_tags(db_state: State<DbState>) -> Result<Vec<TagDto>, String> {
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    // Only tags currently applied to at least one non-deleted note — a tag
    // that only exists on a note in Recently Deleted (or that's had its
    // last reference removed) shouldn't linger in the sidebar as a dead end.
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT t.id, t.name
             FROM tags t
             JOIN note_tags nt ON nt.tag_id = t.id
             JOIN notes n ON n.id = nt.note_id
             WHERE n.deleted_at IS NULL
             ORDER BY t.name",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(TagDto {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_note_body(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    id: String,
) -> Result<String, String> {
    let notes_root = require_notes_root(&root_state)?;
    let rel_path: String = {
        let conn = db_state.0.lock().map_err(|e| e.to_string())?;
        conn.query_row("SELECT file_path FROM notes WHERE id = ?1", [&id], |r| r.get(0))
            .map_err(|e| e.to_string())?
    };
    store::read_note_body(&notes_root, &rel_path)
}

#[tauri::command]
pub fn save_note_body(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    id: String,
    body: String,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::save_note_body(&conn, &notes_root, &id, &body)
}

#[tauri::command]
pub fn create_note(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    folder_id: String,
) -> Result<String, String> {
    let notes_root = require_notes_root(&root_state)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::create_note(&conn, &notes_root, &folder_id)
}

#[tauri::command]
pub fn set_note_pinned(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    id: String,
    pinned: bool,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::set_pinned(&conn, &notes_root, &id, pinned)
}

#[tauri::command]
pub fn add_note_tag(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    id: String,
    tag_name: String,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::add_tag_to_note(&conn, &notes_root, &id, &tag_name)
}

#[tauri::command]
pub fn remove_note_tag(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    id: String,
    tag_name: String,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::remove_tag_from_note(&conn, &notes_root, &id, &tag_name)
}

#[tauri::command]
pub fn set_note_deleted(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    id: String,
    deleted: bool,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::set_deleted(&conn, &notes_root, &id, deleted)
}

#[tauri::command]
pub fn delete_note_permanently(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    id: String,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::delete_note_permanently(&conn, &notes_root, &id)
}

#[tauri::command]
pub fn move_note(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    id: String,
    folder_id: String,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::move_note(&conn, &notes_root, &id, &folder_id)
}

#[tauri::command]
pub fn create_folder(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    parent_id: String,
    name: String,
) -> Result<String, String> {
    let notes_root = require_notes_root(&root_state)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::create_folder(&conn, &notes_root, &parent_id, &name)
}

#[tauri::command]
pub fn rename_folder(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    id: String,
    name: String,
) -> Result<String, String> {
    let notes_root = require_notes_root(&root_state)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::rename_folder(&conn, &notes_root, &id, &name)
}

#[tauri::command]
pub fn delete_folder(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    id: String,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::delete_folder(&conn, &notes_root, &id)
}
