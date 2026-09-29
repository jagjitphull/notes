use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::note_file::{self, FrontMatter};

pub type StoreResult<T> = Result<T, String>;

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn mtime_rfc3339(path: &Path) -> String {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| DateTime::<Utc>::from(t).to_rfc3339())
        .unwrap_or_else(|_| now_rfc3339())
}

fn rel_path(notes_root: &Path, path: &Path) -> String {
    path.strip_prefix(notes_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// A directory's folder id: its path relative to the notes root, or ""
/// for the notes root itself (shown to the user as "Notes").
fn dir_id(notes_root: &Path, dir: &Path) -> String {
    if dir == notes_root {
        String::new()
    } else {
        rel_path(notes_root, dir)
    }
}

fn parent_dir_id(notes_root: &Path, dir: &Path) -> String {
    match dir.parent() {
        Some(p) if p.starts_with(notes_root) || p == notes_root => dir_id(notes_root, p),
        _ => String::new(),
    }
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// Re-walks the whole notes root, upserting every folder/note found and
/// dropping any DB row whose file/directory no longer exists on disk (e.g.
/// deleted from another synced device). Safe to call repeatedly.
pub fn full_rescan(conn: &Connection, notes_root: &Path) -> StoreResult<()> {
    fs::create_dir_all(notes_root).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO folders (id, name, parent_id) VALUES ('', 'Notes', NULL)
         ON CONFLICT(id) DO NOTHING",
        [],
    )
    .map_err(|e| e.to_string())?;

    let mut seen_folder_ids = vec![String::new()];
    let mut seen_note_ids: Vec<String> = Vec::new();

    for entry in WalkDir::new(notes_root)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
    {
        let Ok(entry) = entry else { continue };
        let path = entry.path();

        if entry.file_type().is_dir() {
            let id = dir_id(notes_root, path);
            let name = entry.file_name().to_string_lossy().to_string();
            let parent_id = parent_dir_id(notes_root, path);
            conn.execute(
                "INSERT INTO folders (id, name, parent_id) VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET name = excluded.name, parent_id = excluded.parent_id",
                params![id, name, parent_id],
            )
            .map_err(|e| e.to_string())?;
            seen_folder_ids.push(id);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            if let Some(id) = upsert_note_file(conn, notes_root, path)? {
                seen_note_ids.push(id);
            }
        }
    }

    for id in all_ids(conn, "notes")? {
        if !seen_note_ids.contains(&id) {
            conn.execute("DELETE FROM notes WHERE id = ?1", [&id])
                .map_err(|e| e.to_string())?;
        }
    }
    for id in all_ids(conn, "folders")? {
        if !seen_folder_ids.contains(&id) {
            conn.execute("DELETE FROM folders WHERE id = ?1", [&id])
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn all_ids(conn: &Connection, table: &str) -> StoreResult<Vec<String>> {
    let sql = format!("SELECT id FROM {table}");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Parses one Markdown file and upserts it into the index. Files without
/// recognizable front matter (dropped in from outside the app) are claimed:
/// assigned a fresh id and rewritten with front matter attached.
pub fn upsert_note_file(
    conn: &Connection,
    notes_root: &Path,
    path: &Path,
) -> StoreResult<Option<String>> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Ok(None);
    };
    let parsed = note_file::parse(&raw);

    let front_matter = match parsed.front_matter {
        Some(fm) => fm,
        None => {
            let fm = FrontMatter {
                id: Uuid::new_v4().to_string(),
                tags: Vec::new(),
                pinned: false,
                created_at: now_rfc3339(),
                deleted_at: None,
            };
            let content = note_file::serialize(&fm, &parsed.body);
            fs::write(path, &content).map_err(|e| e.to_string())?;
            fm
        }
    };

    let title = note_file::extract_title(&parsed.body);
    let preview = note_file::extract_preview(&parsed.body);
    let folder_id = dir_id(notes_root, path.parent().unwrap_or(notes_root));
    let file_rel = rel_path(notes_root, path);
    let updated_at = mtime_rfc3339(path);

    // Defensive: clear out any stale row squatting on this path under a
    // different id (e.g. rapid external delete+recreate between scans).
    conn.execute(
        "DELETE FROM notes WHERE file_path = ?1 AND id != ?2",
        params![file_rel, front_matter.id],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO notes (id, file_path, title, plaintext_content, folder_id, is_pinned, deleted_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
           file_path = excluded.file_path,
           title = excluded.title,
           plaintext_content = excluded.plaintext_content,
           folder_id = excluded.folder_id,
           is_pinned = excluded.is_pinned,
           deleted_at = excluded.deleted_at,
           updated_at = excluded.updated_at",
        params![
            front_matter.id,
            file_rel,
            title,
            preview,
            folder_id,
            front_matter.pinned as i32,
            front_matter.deleted_at,
            front_matter.created_at,
            updated_at,
        ],
    )
    .map_err(|e| e.to_string())?;

    sync_tags(conn, &front_matter.id, &front_matter.tags)?;

    Ok(Some(front_matter.id))
}

fn sync_tags(conn: &Connection, note_id: &str, tag_names: &[String]) -> StoreResult<()> {
    conn.execute("DELETE FROM note_tags WHERE note_id = ?1", [note_id])
        .map_err(|e| e.to_string())?;
    for name in tag_names {
        let tag_id = ensure_tag(conn, name)?;
        conn.execute(
            "INSERT OR IGNORE INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
            params![note_id, tag_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_tag(conn: &Connection, name: &str) -> StoreResult<String> {
    if let Ok(id) = conn.query_row("SELECT id FROM tags WHERE name = ?1", [name], |r| {
        r.get::<_, String>(0)
    }) {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    conn.execute("INSERT INTO tags (id, name) VALUES (?1, ?2)", params![id, name])
        .map_err(|e| e.to_string())?;
    Ok(id)
}

/// Registers `dir` (and every ancestor up to the notes root) as a folder
/// row. Needed before inserting a note into a folder that was just created
/// on disk and hasn't been picked up by a scan yet.
fn ensure_folder_chain(conn: &Connection, notes_root: &Path, dir: &Path) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO folders (id, name, parent_id) VALUES ('', 'Notes', NULL)
         ON CONFLICT(id) DO NOTHING",
        [],
    )
    .map_err(|e| e.to_string())?;

    if dir == notes_root {
        return Ok(());
    }

    let mut current = notes_root.to_path_buf();
    for component in rel_path(notes_root, dir).split('/') {
        current.push(component);
        let id = dir_id(notes_root, &current);
        let parent_id = parent_dir_id(notes_root, &current);
        conn.execute(
            "INSERT INTO folders (id, name, parent_id) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET parent_id = excluded.parent_id",
            params![id, component, parent_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn unique_filename(dir: &Path, desired_title: &str, exclude: Option<&Path>) -> PathBuf {
    let stem = note_file::sanitize_filename(desired_title);
    let mut candidate = dir.join(format!("{stem}.md"));
    let mut n = 2;
    while candidate.exists() && Some(candidate.as_path()) != exclude {
        candidate = dir.join(format!("{stem} {n}.md"));
        n += 1;
    }
    candidate
}

pub fn create_note(conn: &Connection, notes_root: &Path, folder_id: &str) -> StoreResult<String> {
    let dir = if folder_id.is_empty() {
        notes_root.to_path_buf()
    } else {
        notes_root.join(folder_id)
    };
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    ensure_folder_chain(conn, notes_root, &dir)?;

    let path = unique_filename(&dir, "New Note", None);
    let front_matter = FrontMatter {
        id: Uuid::new_v4().to_string(),
        tags: Vec::new(),
        pinned: false,
        created_at: now_rfc3339(),
        deleted_at: None,
    };
    let content = note_file::serialize(&front_matter, "");
    fs::write(&path, content).map_err(|e| e.to_string())?;

    upsert_note_file(conn, notes_root, &path)?;
    Ok(front_matter.id)
}

/// Rewrites a note's body (debounced autosave from the editor). Renames the
/// underlying file when the title — the body's first line — changed, so the
/// file stays browsable outside the app.
pub fn save_note_body(
    conn: &Connection,
    notes_root: &Path,
    note_id: &str,
    body: &str,
) -> StoreResult<()> {
    let old_rel: String = conn
        .query_row(
            "SELECT file_path FROM notes WHERE id = ?1",
            [note_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let old_path = notes_root.join(&old_rel);

    let raw = fs::read_to_string(&old_path).unwrap_or_default();
    let parsed = note_file::parse(&raw);
    let front_matter = parsed
        .front_matter
        .ok_or_else(|| "note file is missing its front matter".to_string())?;

    let new_title = note_file::extract_title(body);
    let old_title = note_file::extract_title(&parsed.body);

    let new_path = if new_title != old_title {
        let dir = old_path.parent().unwrap_or(notes_root);
        unique_filename(dir, &new_title, Some(old_path.as_path()))
    } else {
        old_path.clone()
    };

    let content = note_file::serialize(&front_matter, body);
    fs::write(&new_path, content).map_err(|e| e.to_string())?;
    if new_path != old_path {
        fs::remove_file(&old_path).ok();
    }

    upsert_note_file(conn, notes_root, &new_path)?;
    Ok(())
}

fn mutate_front_matter(
    conn: &Connection,
    notes_root: &Path,
    note_id: &str,
    f: impl FnOnce(&mut FrontMatter),
) -> StoreResult<()> {
    let rel: String = conn
        .query_row(
            "SELECT file_path FROM notes WHERE id = ?1",
            [note_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let path = notes_root.join(&rel);
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let parsed = note_file::parse(&raw);
    let mut front_matter = parsed
        .front_matter
        .ok_or_else(|| "note file is missing its front matter".to_string())?;
    f(&mut front_matter);
    let content = note_file::serialize(&front_matter, &parsed.body);
    fs::write(&path, content).map_err(|e| e.to_string())?;
    upsert_note_file(conn, notes_root, &path)?;
    Ok(())
}

pub fn set_pinned(conn: &Connection, notes_root: &Path, note_id: &str, pinned: bool) -> StoreResult<()> {
    mutate_front_matter(conn, notes_root, note_id, |fm| fm.pinned = pinned)
}

pub fn set_deleted(
    conn: &Connection,
    notes_root: &Path,
    note_id: &str,
    deleted: bool,
) -> StoreResult<()> {
    mutate_front_matter(conn, notes_root, note_id, |fm| {
        fm.deleted_at = if deleted { Some(now_rfc3339()) } else { None };
    })
}

/// Permanently erases a note (right-click "Delete Permanently" from
/// Recently Deleted) — removes the file from disk, not just the DB row.
pub fn delete_note_permanently(conn: &Connection, notes_root: &Path, note_id: &str) -> StoreResult<()> {
    let rel: String = conn
        .query_row(
            "SELECT file_path FROM notes WHERE id = ?1",
            [note_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    fs::remove_file(notes_root.join(&rel)).ok();
    conn.execute("DELETE FROM notes WHERE id = ?1", [note_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// How long a note stays in Recently Deleted before it's erased for good,
/// matching Apple Notes' own retention window.
pub const TRASH_RETENTION_DAYS: i64 = 30;

/// Permanently erases every note that has been in Recently Deleted longer
/// than `max_age_days`. Called on every startup (and whenever the notes
/// root changes), so trash is swept without needing the app to keep
/// running or a background scheduler.
pub fn purge_expired_trash(
    conn: &Connection,
    notes_root: &Path,
    max_age_days: i64,
) -> StoreResult<usize> {
    let cutoff = (Utc::now() - chrono::Duration::days(max_age_days)).to_rfc3339();
    let ids: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT id FROM notes WHERE deleted_at IS NOT NULL AND deleted_at < ?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([&cutoff], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    };
    for id in &ids {
        delete_note_permanently(conn, notes_root, id)?;
    }
    Ok(ids.len())
}

pub fn read_note_body(notes_root: &Path, rel_file_path: &str) -> StoreResult<String> {
    let raw = fs::read_to_string(notes_root.join(rel_file_path)).map_err(|e| e.to_string())?;
    Ok(note_file::parse(&raw).body)
}

/// Turns free-form user search text into an FTS5 MATCH query: each word
/// becomes its own quoted prefix term, so "groc" matches "grocery" as the
/// user types, multiple words combine with FTS5's implicit AND, and
/// quoting keeps the input from ever being interpreted as FTS5 query
/// syntax (column filters, boolean operators, unbalanced quotes, ...).
pub fn build_fts_query(raw: &str) -> String {
    raw.split_whitespace()
        .map(|tok| format!("\"{}\"*", tok.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn unique_path_for_name(dir: &Path, filename: &std::ffi::OsStr) -> PathBuf {
    let name_path = Path::new(filename);
    let stem = name_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = name_path.extension().map(|s| s.to_string_lossy().to_string());

    let mut candidate = dir.join(filename);
    let mut n = 2;
    while candidate.exists() {
        let name = match &ext {
            Some(e) => format!("{stem} {n}.{e}"),
            None => format!("{stem} {n}"),
        };
        candidate = dir.join(name);
        n += 1;
    }
    candidate
}

fn unique_dir(parent: &Path, desired_name: &str) -> PathBuf {
    let mut candidate = parent.join(desired_name);
    let mut n = 2;
    while candidate.exists() {
        candidate = parent.join(format!("{desired_name} {n}"));
        n += 1;
    }
    candidate
}

/// Moves a note into a different folder (right-click "Move to Folder…").
pub fn move_note(
    conn: &Connection,
    notes_root: &Path,
    note_id: &str,
    target_folder_id: &str,
) -> StoreResult<()> {
    let old_rel: String = conn
        .query_row(
            "SELECT file_path FROM notes WHERE id = ?1",
            [note_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let old_path = notes_root.join(&old_rel);

    let target_dir = if target_folder_id.is_empty() {
        notes_root.to_path_buf()
    } else {
        notes_root.join(target_folder_id)
    };
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    ensure_folder_chain(conn, notes_root, &target_dir)?;

    if target_dir == old_path.parent().unwrap_or(notes_root) {
        return Ok(()); // already there
    }

    let file_name = old_path
        .file_name()
        .ok_or_else(|| "note has an invalid file path".to_string())?;
    let new_path = unique_path_for_name(&target_dir, file_name);

    fs::rename(&old_path, &new_path).map_err(|e| e.to_string())?;
    upsert_note_file(conn, notes_root, &new_path)?;
    Ok(())
}

/// Creates a new subfolder (right-click "New Folder" / the sidebar's "+").
pub fn create_folder(
    conn: &Connection,
    notes_root: &Path,
    parent_id: &str,
    name: &str,
) -> StoreResult<String> {
    let parent_dir = if parent_id.is_empty() {
        notes_root.to_path_buf()
    } else {
        notes_root.join(parent_id)
    };
    fs::create_dir_all(&parent_dir).map_err(|e| e.to_string())?;
    ensure_folder_chain(conn, notes_root, &parent_dir)?;

    let dir = unique_dir(&parent_dir, &note_file::sanitize_filename(name));
    fs::create_dir(&dir).map_err(|e| e.to_string())?;
    ensure_folder_chain(conn, notes_root, &dir)?;
    Ok(dir_id(notes_root, &dir))
}

/// Renames a folder (real directory rename), then re-indexes so every note
/// nested under it picks up its new folder id / file path.
pub fn rename_folder(
    conn: &Connection,
    notes_root: &Path,
    folder_id: &str,
    new_name: &str,
) -> StoreResult<String> {
    if folder_id.is_empty() {
        return Err("the root Notes folder can't be renamed".to_string());
    }
    let old_dir = notes_root.join(folder_id);
    let parent_dir = old_dir.parent().unwrap_or(notes_root).to_path_buf();
    let new_dir = unique_dir(&parent_dir, &note_file::sanitize_filename(new_name));

    fs::rename(&old_dir, &new_dir).map_err(|e| e.to_string())?;
    full_rescan(conn, notes_root)?;
    Ok(dir_id(notes_root, &new_dir))
}

/// Deletes a folder, but only if it's empty (no notes, no subfolders) —
/// refuses otherwise rather than risking silent data loss.
pub fn delete_folder(conn: &Connection, notes_root: &Path, folder_id: &str) -> StoreResult<()> {
    if folder_id.is_empty() {
        return Err("the root Notes folder can't be deleted".to_string());
    }
    let has_notes: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM notes WHERE folder_id = ?1)",
            [folder_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let has_subfolders: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM folders WHERE parent_id = ?1)",
            [folder_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if has_notes || has_subfolders {
        return Err("folder is not empty".to_string());
    }

    fs::remove_dir(notes_root.join(folder_id)).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM folders WHERE id = ?1", [folder_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn temp_dir(label: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!("notes-store-test-{label}-{}", Uuid::new_v4()));
        dir
    }

    #[test]
    fn create_edit_rename_and_delete_round_trip() {
        let app_dir = temp_dir("app");
        let notes_root = temp_dir("root");
        let conn = db::init(&app_dir).unwrap();

        full_rescan(&conn, &notes_root).unwrap();

        let id = create_note(&conn, &notes_root, "").unwrap();
        save_note_body(&conn, &notes_root, &id, "Grocery list\nMilk, eggs").unwrap();

        let (title, file_path): (String, String) = conn
            .query_row(
                "SELECT title, file_path FROM notes WHERE id = ?1",
                [&id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(title, "Grocery list");
        assert_eq!(file_path, "Grocery list.md");
        assert!(notes_root.join("Grocery list.md").exists());

        // Renaming the title (first line) should rename the file too.
        save_note_body(&conn, &notes_root, &id, "Shopping list\nMilk, eggs").unwrap();
        let file_path: String = conn
            .query_row("SELECT file_path FROM notes WHERE id = ?1", [&id], |r| r.get(0))
            .unwrap();
        assert_eq!(file_path, "Shopping list.md");
        assert!(!notes_root.join("Grocery list.md").exists());
        assert!(notes_root.join("Shopping list.md").exists());

        set_pinned(&conn, &notes_root, &id, true).unwrap();
        let pinned: bool = conn
            .query_row("SELECT is_pinned FROM notes WHERE id = ?1", [&id], |r| r.get(0))
            .unwrap();
        assert!(pinned);

        set_deleted(&conn, &notes_root, &id, true).unwrap();
        let deleted_at: Option<String> = conn
            .query_row("SELECT deleted_at FROM notes WHERE id = ?1", [&id], |r| r.get(0))
            .unwrap();
        assert!(deleted_at.is_some());

        // A full rescan should reproduce the exact same index state.
        full_rescan(&conn, &notes_root).unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn rescan_removes_notes_deleted_externally() {
        let app_dir = temp_dir("app2");
        let notes_root = temp_dir("root2");
        let conn = db::init(&app_dir).unwrap();

        let id = create_note(&conn, &notes_root, "").unwrap();
        full_rescan(&conn, &notes_root).unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);

        let file_path: String = conn
            .query_row("SELECT file_path FROM notes WHERE id = ?1", [&id], |r| r.get(0))
            .unwrap();
        fs::remove_file(notes_root.join(file_path)).unwrap();

        full_rescan(&conn, &notes_root).unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn note_in_subfolder_gets_that_folders_id() {
        let app_dir = temp_dir("app4");
        let notes_root = temp_dir("root4");
        let conn = db::init(&app_dir).unwrap();

        let id = create_note(&conn, &notes_root, "Work").unwrap();

        let folder_id: String = conn
            .query_row("SELECT folder_id FROM notes WHERE id = ?1", [&id], |r| r.get(0))
            .unwrap();
        assert_eq!(folder_id, "Work");

        // A rescan (the path the app actually runs on startup) must agree.
        full_rescan(&conn, &notes_root).unwrap();
        let folder_id_after_rescan: String = conn
            .query_row("SELECT folder_id FROM notes WHERE id = ?1", [&id], |r| r.get(0))
            .unwrap();
        assert_eq!(folder_id_after_rescan, "Work");
    }

    #[test]
    fn claims_plain_markdown_files_dropped_in_externally() {
        let app_dir = temp_dir("app3");
        let notes_root = temp_dir("root3");
        fs::create_dir_all(&notes_root).unwrap();
        fs::write(notes_root.join("External.md"), "External note\nBody text").unwrap();

        let conn = db::init(&app_dir).unwrap();
        full_rescan(&conn, &notes_root).unwrap();

        let title: String = conn
            .query_row("SELECT title FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "External note");

        let raw = fs::read_to_string(notes_root.join("External.md")).unwrap();
        assert!(raw.starts_with("---\n"));
    }

    #[test]
    fn folder_create_rename_delete_and_note_move() {
        let app_dir = temp_dir("app5");
        let notes_root = temp_dir("root5");
        let conn = db::init(&app_dir).unwrap();
        full_rescan(&conn, &notes_root).unwrap();

        let work_id = create_folder(&conn, &notes_root, "", "Work").unwrap();
        assert_eq!(work_id, "Work");
        assert!(notes_root.join("Work").is_dir());

        let note_id = create_note(&conn, &notes_root, "").unwrap();
        move_note(&conn, &notes_root, &note_id, &work_id).unwrap();
        let folder_id: String = conn
            .query_row("SELECT folder_id FROM notes WHERE id = ?1", [&note_id], |r| r.get(0))
            .unwrap();
        assert_eq!(folder_id, "Work");

        // Can't delete a non-empty folder.
        assert!(delete_folder(&conn, &notes_root, &work_id).is_err());

        let renamed_id = rename_folder(&conn, &notes_root, &work_id, "Projects").unwrap();
        assert_eq!(renamed_id, "Projects");
        assert!(!notes_root.join("Work").exists());
        assert!(notes_root.join("Projects").is_dir());
        let folder_id_after_rename: String = conn
            .query_row("SELECT folder_id FROM notes WHERE id = ?1", [&note_id], |r| r.get(0))
            .unwrap();
        assert_eq!(folder_id_after_rename, "Projects");

        move_note(&conn, &notes_root, &note_id, "").unwrap();
        delete_folder(&conn, &notes_root, &renamed_id).unwrap();
        assert!(!notes_root.join("Projects").exists());
        let folder_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM folders WHERE id = ?1",
                [&renamed_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(folder_count, 0);
    }

    #[test]
    fn delete_note_permanently_removes_file_and_row() {
        let app_dir = temp_dir("app6");
        let notes_root = temp_dir("root6");
        let conn = db::init(&app_dir).unwrap();

        let id = create_note(&conn, &notes_root, "").unwrap();
        let rel: String = conn
            .query_row("SELECT file_path FROM notes WHERE id = ?1", [&id], |r| r.get(0))
            .unwrap();
        assert!(notes_root.join(&rel).exists());

        delete_note_permanently(&conn, &notes_root, &id).unwrap();
        assert!(!notes_root.join(&rel).exists());
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn purge_expired_trash_removes_only_notes_past_retention() {
        let app_dir = temp_dir("app7");
        let notes_root = temp_dir("root7");
        let conn = db::init(&app_dir).unwrap();

        let old_id = create_note(&conn, &notes_root, "").unwrap();
        let recent_id = create_note(&conn, &notes_root, "").unwrap();
        let kept_id = create_note(&conn, &notes_root, "").unwrap(); // never deleted

        let old_deleted_at = (Utc::now() - chrono::Duration::days(31)).to_rfc3339();
        mutate_front_matter(&conn, &notes_root, &old_id, |fm| {
            fm.deleted_at = Some(old_deleted_at);
        })
        .unwrap();
        set_deleted(&conn, &notes_root, &recent_id, true).unwrap(); // deleted "now"

        let purged = purge_expired_trash(&conn, &notes_root, TRASH_RETENTION_DAYS).unwrap();
        assert_eq!(purged, 1);

        let remaining_ids: Vec<String> = {
            let mut stmt = conn.prepare("SELECT id FROM notes ORDER BY id").unwrap();
            stmt.query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
        };
        assert!(!remaining_ids.contains(&old_id));
        assert!(remaining_ids.contains(&recent_id));
        assert!(remaining_ids.contains(&kept_id));
    }

    #[test]
    fn fts_query_quotes_and_prefixes_each_word() {
        assert_eq!(build_fts_query("grocery"), "\"grocery\"*");
        assert_eq!(build_fts_query("buy milk"), "\"buy\"* \"milk\"*");
        assert_eq!(build_fts_query(""), "");
        assert_eq!(build_fts_query("   "), "");
        // A quote in the query must not break out of FTS5's string literal.
        assert_eq!(build_fts_query("say \"hi\""), "\"say\"* \"\"\"hi\"\"\"*");
        // Would otherwise be read as FTS5 boolean/column-filter syntax.
        assert_eq!(build_fts_query("title:foo AND bar"), "\"title:foo\"* \"AND\"* \"bar\"*");
    }
}
