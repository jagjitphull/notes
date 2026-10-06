use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::config::{self, AppConfig};
use crate::db::DbState;
use crate::store::SaveOutcome;
use crate::vault::{NoteCodec, VaultState};
use crate::{store, vault, versions, watcher};

pub struct NotesRootState(pub Mutex<Option<PathBuf>>);

/// The codec to read/write this notes_root's note files with: `Plain` if
/// it isn't an encrypted vault at all, `Encrypted` with the session's
/// unlocked key if it is and has been unlocked. Refuses (rather than
/// guessing) if it's encrypted but still locked - every command that
/// touches note content calls this before touching any, so a locked vault
/// can't be read from or written to at all, let alone have plaintext
/// accidentally written into it.
fn require_codec(vault_state: &State<VaultState>, notes_root: &Path) -> Result<NoteCodec, String> {
    if !vault::is_encrypted(notes_root) {
        return Ok(NoteCodec::Plain);
    }
    let key = vault_state.0.lock().map_err(|e| e.to_string())?;
    key.map(NoteCodec::Encrypted).ok_or_else(|| "vault is locked".to_string())
}

/// Tracks, per note id, the content hash of the body each note's editor
/// last loaded - populated by [`get_note_body`], consulted by
/// [`save_note_body`] to detect a sync conflict (the file changed
/// externally since that load). In-memory only: a restart simply means
/// every note looks freshly loaded again, which is correct.
pub struct LoadedHashState(pub Mutex<HashMap<String, String>>);

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

    let app_config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    config::save(
        &app_config_dir,
        &AppConfig {
            notes_root: Some(path.clone()),
        },
    )
    .map_err(|e| e.to_string())?;

    // An encrypted vault can't be indexed here - there's no key yet, before
    // the frontend has shown the unlock screen for it. The unlock command
    // does its own full_rescan (and trash purge) once it actually unlocks.
    if !vault::is_encrypted(&path) {
        let conn = db_state.0.lock().map_err(|e| e.to_string())?;
        store::full_rescan(&conn, &path, &NoteCodec::Plain)?;
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
    pub color: Option<String>,
}

#[tauri::command]
pub fn list_folders(db_state: State<DbState>) -> Result<Vec<FolderDto>, String> {
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, parent_id, color FROM folders ORDER BY name")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(FolderDto {
                id: r.get(0)?,
                name: r.get(1)?,
                parent_id: r.get(2)?,
                color: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
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
    pub created_at: String,
    pub updated_at: String,
    pub is_template: bool,
}

const NOTE_LIST_ITEM_COLUMNS: &str = "n.id, n.title, n.plaintext_content, n.folder_id, n.is_pinned,
     n.deleted_at, n.created_at, n.updated_at, n.is_template,
     COALESCE((SELECT GROUP_CONCAT(nt.tag_id) FROM note_tags nt WHERE nt.note_id = n.id), '')";

fn map_note_list_item(r: &rusqlite::Row) -> rusqlite::Result<NoteListItemDto> {
    let tag_ids_raw: String = r.get(9)?;
    Ok(NoteListItemDto {
        id: r.get(0)?,
        title: r.get(1)?,
        plaintext_content: r.get(2)?,
        folder_id: r.get(3)?,
        is_pinned: r.get::<_, i64>(4)? != 0,
        deleted_at: r.get(5)?,
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
        is_template: r.get::<_, i64>(8)? != 0,
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
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Searches notes via the FTS5 index (title + full body), ranked by
/// relevance. Each whitespace-separated word becomes its own quoted prefix
/// term ("word"*) so results update sensibly as the user keeps typing, and
/// so free-form query text can't be misread as FTS5 query syntax (column
/// filters, boolean operators, etc).
#[tauri::command]
pub fn search_notes(
    db_state: State<DbState>,
    query: String,
) -> Result<Vec<NoteListItemDto>, String> {
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
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Fast reachability check so the frontend can decide whether to show the
/// Smart Search toggle at all — Ollama not being installed/running is the
/// expected common case, not an error, so this returns a plain bool rather
/// than surfacing a failure.
#[tauri::command]
pub fn smart_search_available() -> bool {
    crate::embeddings::is_available()
}

/// Looks up the full row for each of `ranked_ids`, in that same order -
/// shared by every command that ranks notes by something other than a
/// SQL ORDER BY (embedding similarity), so the ranking survives an
/// IN (...) query's arbitrary row order.
fn note_list_items_in_order(
    conn: &rusqlite::Connection,
    ranked_ids: Vec<String>,
) -> Result<Vec<NoteListItemDto>, String> {
    if ranked_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = ranked_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql =
        format!("SELECT {NOTE_LIST_ITEM_COLUMNS} FROM notes n WHERE n.id IN ({placeholders})");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let query_params: Vec<&dyn rusqlite::ToSql> = ranked_ids
        .iter()
        .map(|id| id as &dyn rusqlite::ToSql)
        .collect();
    let rows = stmt
        .query_map(query_params.as_slice(), map_note_list_item)
        .map_err(|e| e.to_string())?;

    let mut by_id: std::collections::HashMap<String, NoteListItemDto> = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|dto| (dto.id.clone(), dto))
        .collect();

    Ok(ranked_ids
        .into_iter()
        .filter_map(|id| by_id.remove(&id))
        .collect())
}

/// Semantic search over note content via local Ollama embeddings, ranked
/// by cosine similarity (see `store::smart_search`). Errors here (almost
/// always: Ollama isn't reachable) are meant to be caught by the frontend
/// and silently fall back to `search_notes`.
#[tauri::command]
pub fn smart_search(
    db_state: State<DbState>,
    query: String,
) -> Result<Vec<NoteListItemDto>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let ranked_ids = store::smart_search(&db_state.0, trimmed, 30)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    note_list_items_in_order(&conn, ranked_ids)
}

/// Other notes most semantically similar to note `id` (see
/// `store::related_notes`). Same error behavior as `smart_search`: the
/// frontend treats a failure as "nothing to show" rather than an error.
#[tauri::command]
pub fn related_notes(
    db_state: State<DbState>,
    id: String,
) -> Result<Vec<NoteListItemDto>, String> {
    let ranked_ids = store::related_notes(&db_state.0, &id, 5)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    note_list_items_in_order(&conn, ranked_ids)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDto {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentDto {
    pub path: String,
    pub name: String,
    pub size: u64,
}

impl From<store::AttachmentInfo> for AttachmentDto {
    fn from(info: store::AttachmentInfo) -> Self {
        Self {
            path: info.path,
            name: info.name,
            size: info.size,
        }
    }
}

#[tauri::command]
pub fn list_tags(db_state: State<DbState>) -> Result<Vec<TagDto>, String> {
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    // Only tags currently applied to at least one non-deleted note — a tag
    // that only exists on a note in Recently Deleted (or that's had its
    // last reference removed) shouldn't linger in the sidebar as a dead end.
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT t.id, t.name, t.color
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
                color: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// `color` of `None` clears back to the default (no tint).
#[tauri::command]
pub fn set_tag_color(
    db_state: State<DbState>,
    id: String,
    color: Option<String>,
) -> Result<(), String> {
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE tags SET color = ?1 WHERE id = ?2",
        rusqlite::params![color, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_note_body(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    vault_state: State<VaultState>,
    hash_state: State<LoadedHashState>,
    id: String,
) -> Result<String, String> {
    let notes_root = require_notes_root(&root_state)?;
    let codec = require_codec(&vault_state, &notes_root)?;
    let rel_path: String = {
        let conn = db_state.0.lock().map_err(|e| e.to_string())?;
        conn.query_row("SELECT file_path FROM notes WHERE id = ?1", [&id], |r| {
            r.get(0)
        })
        .map_err(|e| e.to_string())?
    };
    let body = store::read_note_body(&notes_root, &rel_path, &codec)?;
    hash_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .insert(id, store::hash_body(&body));
    Ok(body)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "outcome")]
pub enum SaveNoteBodyResult {
    Saved,
    Conflict {
        conflicted_note_id: String,
        conflicted_title: String,
        original_body: String,
    },
}

#[tauri::command]
pub fn save_note_body(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    vault_state: State<VaultState>,
    hash_state: State<LoadedHashState>,
    id: String,
    body: String,
) -> Result<SaveNoteBodyResult, String> {
    let notes_root = require_notes_root(&root_state)?;
    let codec = require_codec(&vault_state, &notes_root)?;
    let expected_hash = hash_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&id)
        .cloned();

    let outcome = {
        let conn = db_state.0.lock().map_err(|e| e.to_string())?;
        store::save_note_body(&conn, &notes_root, &id, &body, expected_hash.as_deref(), &codec)?
    };

    let (result, new_hash) = match outcome {
        SaveOutcome::Saved { hash } => (SaveNoteBodyResult::Saved, hash),
        SaveOutcome::Conflict {
            conflicted_note_id,
            conflicted_title,
            original_body,
            hash,
        } => (
            SaveNoteBodyResult::Conflict {
                conflicted_note_id,
                conflicted_title,
                original_body,
            },
            hash,
        ),
    };
    hash_state.0.lock().map_err(|e| e.to_string())?.insert(id, new_hash);
    Ok(result)
}

#[tauri::command]
pub fn list_note_versions(
    root_state: State<NotesRootState>,
    id: String,
) -> Result<Vec<versions::VersionInfo>, String> {
    let notes_root = require_notes_root(&root_state)?;
    versions::list(&notes_root, &id)
}

#[tauri::command]
pub fn restore_note_version(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    vault_state: State<VaultState>,
    hash_state: State<LoadedHashState>,
    id: String,
    timestamp: String,
) -> Result<String, String> {
    let notes_root = require_notes_root(&root_state)?;
    let codec = require_codec(&vault_state, &notes_root)?;
    let rel_path: String = {
        let conn = db_state.0.lock().map_err(|e| e.to_string())?;
        store::restore_version(&conn, &notes_root, &id, &timestamp, &codec)?;
        conn.query_row("SELECT file_path FROM notes WHERE id = ?1", [&id], |r| {
            r.get(0)
        })
        .map_err(|e| e.to_string())?
    };
    let body = store::read_note_body(&notes_root, &rel_path, &codec)?;
    hash_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .insert(id, store::hash_body(&body));
    Ok(body)
}

#[tauri::command]
pub fn import_markdown_folder(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    vault_state: State<VaultState>,
    path: String,
) -> Result<usize, String> {
    let notes_root = require_notes_root(&root_state)?;
    let codec = require_codec(&vault_state, &notes_root)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::import_markdown_folder(&conn, &notes_root, std::path::Path::new(&path), &codec)
}

/// Writes arbitrary text content to a path the user picked themselves via
/// a native save dialog (not resolved against notes_root, unlike every
/// other file command here) - used for exporting a note as a standalone
/// HTML file.
#[tauri::command]
pub fn export_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

/// Zips the entire notes folder - every note, subfolder, attachment, and
/// version-history file - to a path the user picked via a save dialog.
/// See `backup::export_vault`.
#[tauri::command]
pub fn export_vault_backup(root_state: State<NotesRootState>, path: String) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    crate::backup::export_vault(&notes_root, std::path::Path::new(&path))
}

/// Extracts a vault backup zip into a destination folder the user picked
/// (must be empty - see `backup::restore_vault`), returning the number of
/// files restored. Doesn't touch the app's own notes folder setting -
/// the frontend calls `set_notes_root` separately once the user confirms
/// they want to switch to the restored notes.
#[tauri::command]
pub fn restore_vault_backup(zip_path: String, dest_dir: String) -> Result<usize, String> {
    crate::backup::restore_vault(
        std::path::Path::new(&zip_path),
        std::path::Path::new(&dest_dir),
    )
}

#[tauri::command]
pub fn create_note(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    vault_state: State<VaultState>,
    folder_id: String,
    is_template: bool,
) -> Result<String, String> {
    let notes_root = require_notes_root(&root_state)?;
    let codec = require_codec(&vault_state, &notes_root)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::create_note(&conn, &notes_root, &folder_id, is_template, &codec)
}

#[tauri::command]
pub fn create_note_from_template(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    vault_state: State<VaultState>,
    folder_id: String,
    template_id: String,
) -> Result<String, String> {
    let notes_root = require_notes_root(&root_state)?;
    let codec = require_codec(&vault_state, &notes_root)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::create_note_from_template(&conn, &notes_root, &folder_id, &template_id, &codec)
}

#[tauri::command]
pub fn set_note_template(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    vault_state: State<VaultState>,
    id: String,
    is_template: bool,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let codec = require_codec(&vault_state, &notes_root)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::set_template(&conn, &notes_root, &id, is_template, &codec)
}

#[tauri::command]
pub fn get_or_create_daily_note(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    vault_state: State<VaultState>,
    date: String,
) -> Result<String, String> {
    let notes_root = require_notes_root(&root_state)?;
    let codec = require_codec(&vault_state, &notes_root)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::get_or_create_daily_note(&conn, &notes_root, &date, &codec)
}

#[tauri::command]
pub fn set_note_pinned(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    vault_state: State<VaultState>,
    id: String,
    pinned: bool,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let codec = require_codec(&vault_state, &notes_root)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::set_pinned(&conn, &notes_root, &id, pinned, &codec)
}

#[tauri::command]
pub fn add_note_tag(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    vault_state: State<VaultState>,
    id: String,
    tag_name: String,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let codec = require_codec(&vault_state, &notes_root)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::add_tag_to_note(&conn, &notes_root, &id, &tag_name, &codec)
}

#[tauri::command]
pub fn remove_note_tag(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    vault_state: State<VaultState>,
    id: String,
    tag_name: String,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let codec = require_codec(&vault_state, &notes_root)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::remove_tag_from_note(&conn, &notes_root, &id, &tag_name, &codec)
}

#[tauri::command]
pub fn set_note_deleted(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    vault_state: State<VaultState>,
    id: String,
    deleted: bool,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let codec = require_codec(&vault_state, &notes_root)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::set_deleted(&conn, &notes_root, &id, deleted, &codec)
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
    vault_state: State<VaultState>,
    id: String,
    folder_id: String,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let codec = require_codec(&vault_state, &notes_root)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::move_note(&conn, &notes_root, &id, &folder_id, &codec)
}

#[tauri::command]
pub fn save_attachment(
    root_state: State<NotesRootState>,
    note_id: String,
    filename: String,
    bytes: Vec<u8>,
) -> Result<AttachmentDto, String> {
    let notes_root = require_notes_root(&root_state)?;
    store::save_attachment(&notes_root, &note_id, &filename, &bytes).map(Into::into)
}

#[tauri::command]
pub fn get_attachment_size(root_state: State<NotesRootState>, path: String) -> Result<u64, String> {
    let notes_root = require_notes_root(&root_state)?;
    store::attachment_size(&notes_root, &path)
}

#[tauri::command]
pub fn open_attachment(
    app: AppHandle,
    root_state: State<NotesRootState>,
    path: String,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let abs = store::attachment_absolute_path(&notes_root, &path)?;
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(abs.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

// A note's hyperlink can hold any text a user (or a pasted/imported file)
// typed as a URL, including a `javascript:`/`file:` scheme - rejecting
// anything but http(s)/mailto here is what actually stops that from doing
// something unexpected when clicked, not validation on the editor side.
fn allowed_link_scheme(url: &str) -> bool {
    let scheme = url.split(':').next().unwrap_or("").to_ascii_lowercase();
    scheme == "http" || scheme == "https" || scheme == "mailto"
}

#[tauri::command]
pub fn open_external_link(app: AppHandle, url: String) -> Result<(), String> {
    if !allowed_link_scheme(&url) {
        return Err("Refusing to open an unsupported link scheme".into());
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub encrypted: bool,
    pub unlocked: bool,
}

#[tauri::command]
pub fn vault_status(root_state: State<NotesRootState>, vault_state: State<VaultState>) -> Result<VaultStatus, String> {
    let notes_root = require_notes_root(&root_state)?;
    let encrypted = vault::is_encrypted(&notes_root);
    let unlocked = !encrypted || vault_state.0.lock().map_err(|e| e.to_string())?.is_some();
    Ok(VaultStatus { encrypted, unlocked })
}

/// Turns on encryption for the current (not-yet-encrypted) vault: creates
/// its keyring, re-encrypts every existing note in place, and unlocks the
/// new vault for the rest of this session. Returns the recovery key,
/// formatted for display - this is the only time it's ever recoverable, so
/// the frontend must show it to the user before this resolves into
/// anything else happening.
#[tauri::command]
pub fn enable_vault_encryption(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    vault_state: State<VaultState>,
    password: String,
) -> Result<String, String> {
    let notes_root = require_notes_root(&root_state)?;
    let (vault_key, recovery_key) = vault::enable(&notes_root, &password)?;

    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::migrate_to_encrypted(&conn, &notes_root, &vault_key)?;
    drop(conn);

    *vault_state.0.lock().map_err(|e| e.to_string())? = Some(vault_key);
    Ok(recovery_key)
}

/// Shared by both unlock commands: once a key is recovered (by whichever
/// means), stores it for the session and re-indexes the vault, which the
/// locked startup/notes-root-selection path deliberately skipped.
fn finish_unlock(
    db_state: &State<DbState>,
    notes_root: &Path,
    vault_state: &State<VaultState>,
    vault_key: [u8; 32],
) -> Result<(), String> {
    *vault_state.0.lock().map_err(|e| e.to_string())? = Some(vault_key);
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::full_rescan(&conn, notes_root, &NoteCodec::Encrypted(vault_key))?;
    store::purge_expired_trash(&conn, notes_root, store::TRASH_RETENTION_DAYS)?;
    Ok(())
}

#[tauri::command]
pub fn unlock_vault_with_password(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    vault_state: State<VaultState>,
    password: String,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let vault_key = vault::unlock_with_password(&notes_root, &password)?;
    finish_unlock(&db_state, &notes_root, &vault_state, vault_key)
}

#[tauri::command]
pub fn unlock_vault_with_recovery_key(
    db_state: State<DbState>,
    root_state: State<NotesRootState>,
    vault_state: State<VaultState>,
    recovery_key: String,
) -> Result<(), String> {
    let notes_root = require_notes_root(&root_state)?;
    let vault_key = vault::unlock_with_recovery_key(&notes_root, &recovery_key)?;
    finish_unlock(&db_state, &notes_root, &vault_state, vault_key)
}

/// Clears the session's unlocked key, so the vault needs its password (or
/// recovery key) again - a deliberate action from the UI, since this app
/// otherwise only ever prompts once per launch.
#[tauri::command]
pub fn lock_vault(vault_state: State<VaultState>) -> Result<(), String> {
    *vault_state.0.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_link_scheme_accepts_http_https_and_mailto() {
        assert!(allowed_link_scheme("http://example.com"));
        assert!(allowed_link_scheme("https://example.com/path?q=1"));
        assert!(allowed_link_scheme("mailto:a@example.com"));
        assert!(allowed_link_scheme("HTTPS://Example.com"));
    }

    #[test]
    fn allowed_link_scheme_rejects_everything_else() {
        assert!(!allowed_link_scheme("javascript:alert(1)"));
        assert!(!allowed_link_scheme("file:///etc/passwd"));
        assert!(!allowed_link_scheme("data:text/html,hi"));
        assert!(!allowed_link_scheme("not a url"));
        assert!(!allowed_link_scheme(""));
    }
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
    vault_state: State<VaultState>,
    id: String,
    name: String,
) -> Result<String, String> {
    let notes_root = require_notes_root(&root_state)?;
    let codec = require_codec(&vault_state, &notes_root)?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    store::rename_folder(&conn, &notes_root, &id, &name, &codec)
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

/// `color` of `None` clears back to the default (no tint).
#[tauri::command]
pub fn set_folder_color(
    db_state: State<DbState>,
    id: String,
    color: Option<String>,
) -> Result<(), String> {
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE folders SET color = ?1 WHERE id = ?2",
        rusqlite::params![color, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
