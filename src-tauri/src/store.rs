use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, Utc};
use rusqlite::{Connection, params};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::embeddings;
use crate::note_file::{self, FrontMatter};
use crate::versions;

pub type StoreResult<T> = Result<T, String>;

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

/// Exposed for commands.rs, which tracks each open note's last-loaded body
/// hash to detect a sync conflict on save (see [`save_note_body`]).
pub fn hash_body(body: &str) -> String {
    content_hash(body)
}

fn content_hash(text: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
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
        } else if path.extension().and_then(|e| e.to_str()) == Some("md")
            && let Some(id) = upsert_note_file(conn, notes_root, path)?
        {
            seen_note_ids.push(id);
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

/// Copies every .md/.markdown file found under `source` (recursively,
/// skipping dot-prefixed directories like Obsidian's own .obsidian/
/// config folder) into a new top-level folder inside notes_root, mirroring
/// the source's own subfolder layout, then indexes each one - the same
/// claiming logic `upsert_note_file` already applies to any plain file
/// dropped into the notes folder from outside the app, since none of these
/// will carry our own front matter. Note text survives as-is: this app's
/// `[[Title]]` link and `![[path]]` attachment-embed syntax already
/// matches Obsidian's, so cross-note links keep working unchanged.
/// Non-Markdown files (images, PDFs the imported notes may reference)
/// are not migrated - only the Markdown text itself.
pub fn import_markdown_folder(
    conn: &Connection,
    notes_root: &Path,
    source: &Path,
) -> StoreResult<usize> {
    let source_name = source
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Imported".to_string());
    let dest_root = unique_dir(notes_root, &note_file::sanitize_filename(&source_name));
    fs::create_dir_all(&dest_root).map_err(|e| e.to_string())?;
    ensure_folder_chain(conn, notes_root, &dest_root)?;

    let mut imported = 0usize;
    for entry in WalkDir::new(source)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !is_hidden(e))
    {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_lowercase();
        if ext != "md" && ext != "markdown" {
            continue;
        }
        let Ok(rel) = path.strip_prefix(source) else {
            continue;
        };
        let dest_dir = match rel.parent() {
            Some(p) if !p.as_os_str().is_empty() => {
                let d = dest_root.join(p);
                fs::create_dir_all(&d).map_err(|e| e.to_string())?;
                ensure_folder_chain(conn, notes_root, &d)?;
                d
            }
            _ => dest_root.clone(),
        };
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Note");
        let Ok(raw) = fs::read_to_string(path) else {
            continue;
        };
        let dest_path = unique_filename(&dest_dir, stem, None);
        fs::write(&dest_path, raw).map_err(|e| e.to_string())?;
        if upsert_note_file(conn, notes_root, &dest_path)?.is_some() {
            imported += 1;
        }
    }
    Ok(imported)
}

fn all_ids(conn: &Connection, table: &str) -> StoreResult<Vec<String>> {
    let sql = format!("SELECT id FROM {table}");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
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
                is_template: false,
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
        "INSERT INTO notes (id, file_path, title, plaintext_content, folder_id, is_pinned, deleted_at, created_at, updated_at, is_template)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
           file_path = excluded.file_path,
           title = excluded.title,
           plaintext_content = excluded.plaintext_content,
           folder_id = excluded.folder_id,
           is_pinned = excluded.is_pinned,
           deleted_at = excluded.deleted_at,
           updated_at = excluded.updated_at,
           is_template = excluded.is_template",
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
            front_matter.is_template as i32,
        ],
    )
    .map_err(|e| e.to_string())?;

    sync_tags(conn, &front_matter.id, &front_matter.tags)?;

    Ok(Some(front_matter.id))
}

fn sync_tags(conn: &Connection, note_id: &str, tag_names: &[String]) -> StoreResult<()> {
    let previously_linked: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT tag_id FROM note_tags WHERE note_id = ?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([note_id], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

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

    // A tag no longer referenced by any note (this one just dropped its
    // last reference to it) is dead weight — remove it rather than let
    // unused tags accumulate forever.
    for tag_id in previously_linked {
        let still_used: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM note_tags WHERE tag_id = ?1)",
                [&tag_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !still_used {
            conn.execute("DELETE FROM tags WHERE id = ?1", [&tag_id])
                .map_err(|e| e.to_string())?;
        }
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
    conn.execute(
        "INSERT INTO tags (id, name) VALUES (?1, ?2)",
        params![id, name],
    )
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

fn write_new_note(
    conn: &Connection,
    notes_root: &Path,
    folder_id: &str,
    body: &str,
    is_template: bool,
) -> StoreResult<String> {
    let dir = if folder_id.is_empty() {
        notes_root.to_path_buf()
    } else {
        notes_root.join(folder_id)
    };
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    ensure_folder_chain(conn, notes_root, &dir)?;

    let title = note_file::extract_title(body);
    let stem = if title.is_empty() { "New Note" } else { &title };
    let path = unique_filename(&dir, stem, None);
    let front_matter = FrontMatter {
        id: Uuid::new_v4().to_string(),
        tags: Vec::new(),
        pinned: false,
        created_at: now_rfc3339(),
        deleted_at: None,
        is_template,
    };
    let content = note_file::serialize(&front_matter, body);
    fs::write(&path, content).map_err(|e| e.to_string())?;

    upsert_note_file(conn, notes_root, &path)?;
    Ok(front_matter.id)
}

pub fn create_note(
    conn: &Connection,
    notes_root: &Path,
    folder_id: &str,
    is_template: bool,
) -> StoreResult<String> {
    write_new_note(conn, notes_root, folder_id, "", is_template)
}

/// Replaces `{{date}}`/`{{time}}` placeholders with the current local
/// date/time - the one point a template's body is ever rewritten, so a
/// saved template keeps its literal `{{date}}` text until actually used
/// (see `create_note_from_template`, the only caller).
fn substitute_template_variables(body: &str) -> String {
    let now = Local::now();
    body.replace("{{date}}", &now.format("%Y-%m-%d").to_string())
        .replace("{{time}}", &now.format("%H:%M").to_string())
}

/// Copies a template note's current body into a brand-new, ordinary note -
/// a one-time copy, not a link back to the template: editing either one
/// afterwards never affects the other.
pub fn create_note_from_template(
    conn: &Connection,
    notes_root: &Path,
    folder_id: &str,
    template_id: &str,
) -> StoreResult<String> {
    let template_rel: String = conn
        .query_row(
            "SELECT file_path FROM notes WHERE id = ?1 AND is_template = 1",
            [template_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let body = read_note_body(notes_root, &template_rel)?;
    let body = substitute_template_variables(&body);
    write_new_note(conn, notes_root, folder_id, &body, false)
}

pub fn set_template(
    conn: &Connection,
    notes_root: &Path,
    note_id: &str,
    is_template: bool,
) -> StoreResult<()> {
    mutate_front_matter(conn, notes_root, note_id, |fm| fm.is_template = is_template)
}

const DAILY_NOTES_FOLDER: &str = "Daily Notes";

/// "YYYY-MM-DD" shape check - enough to use `date` as a filename stem
/// safely, not a full calendar validity check. `date` is the caller's
/// local date (computed frontend-side, where "local" is simplest to get
/// right); rejecting anything else keeps a malformed value from becoming
/// a surprising file on disk.
fn is_valid_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
}

/// Finds today's daily note (notes_root/Daily Notes/<date>.md), creating
/// it blank if this is the first visit today. Idempotent by construction,
/// since the same date always maps to the same filename: a second call
/// the same day returns the existing note instead of creating a
/// duplicate (unlike create_note's unique_filename, which is
/// deliberately the opposite - always a fresh file).
pub fn get_or_create_daily_note(
    conn: &Connection,
    notes_root: &Path,
    date: &str,
) -> StoreResult<String> {
    if !is_valid_date(date) {
        return Err("invalid date".to_string());
    }
    let dir = notes_root.join(DAILY_NOTES_FOLDER);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    ensure_folder_chain(conn, notes_root, &dir)?;

    let path = dir.join(format!("{date}.md"));
    if !path.exists() {
        let front_matter = FrontMatter {
            id: Uuid::new_v4().to_string(),
            tags: Vec::new(),
            pinned: false,
            created_at: now_rfc3339(),
            deleted_at: None,
            is_template: false,
        };
        let content = note_file::serialize(&front_matter, "");
        fs::write(&path, content).map_err(|e| e.to_string())?;
    }
    upsert_note_file(conn, notes_root, &path)?
        .ok_or_else(|| "failed to read daily note".to_string())
}

/// What a note's front matter + body currently are on disk, re-read fresh
/// so a save always starts from the latest tags/pin-state/etc. (which may
/// have changed from outside this call, e.g. the context menu) rather than
/// a stale copy.
fn read_current(conn: &Connection, notes_root: &Path, note_id: &str) -> StoreResult<(PathBuf, FrontMatter, String)> {
    let rel: String = conn
        .query_row(
            "SELECT file_path FROM notes WHERE id = ?1",
            [note_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let path = notes_root.join(&rel);
    let raw = fs::read_to_string(&path).unwrap_or_default();
    let parsed = note_file::parse(&raw);
    let front_matter = parsed
        .front_matter
        .ok_or_else(|| "note file is missing its front matter".to_string())?;
    Ok((path, front_matter, parsed.body))
}

/// Overwrites a note's body in place, renaming the underlying file when the
/// title — the body's first line — changed, so the file stays browsable
/// outside the app. Always reads the current front matter fresh (see
/// [`read_current`]) rather than trusting a caller-supplied copy.
fn write_body(
    conn: &Connection,
    notes_root: &Path,
    note_id: &str,
    body: &str,
) -> StoreResult<()> {
    let (old_path, front_matter, old_body) = read_current(conn, notes_root, note_id)?;

    let new_title = note_file::extract_title(body);
    let old_title = note_file::extract_title(&old_body);

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

#[derive(Debug, PartialEq)]
pub enum SaveOutcome {
    /// Saved in place; `hash` is the content hash of what's now on disk,
    /// to use as `expected_hash` on the next save.
    Saved { hash: String },
    /// The file changed externally (e.g. synced in from another device)
    /// since the editor last loaded it, and the edit being saved would
    /// have silently clobbered that change. Instead of overwriting, the
    /// edit was written to a brand-new "Conflicted Copy" note, and the
    /// original note's file was left untouched - the caller should reload
    /// the original note's body (now `original_body`) into the editor.
    Conflict {
        conflicted_note_id: String,
        conflicted_title: String,
        original_body: String,
        hash: String,
    },
}

/// Rewrites a note's body (debounced autosave from the editor), guarding
/// against a sync conflict: if `expected_hash` (the hash of the body the
/// editor last loaded, from [`read_note_body`]'s caller) doesn't match
/// what's actually on disk right now, something else changed the file in
/// the meantime and this write must not clobber it - see
/// [`SaveOutcome::Conflict`]. Pass `None` to always save in place
/// (skipping the check), e.g. for a deliberate action like restoring a
/// version.
pub fn save_note_body(
    conn: &Connection,
    notes_root: &Path,
    note_id: &str,
    body: &str,
    expected_hash: Option<&str>,
) -> StoreResult<SaveOutcome> {
    let (old_path, _front_matter, old_body) = read_current(conn, notes_root, note_id)?;
    let on_disk_hash = content_hash(&old_body);

    if let Some(expected) = expected_hash
        && expected != on_disk_hash
    {
        let dir = old_path.parent().unwrap_or(notes_root);
        let conflicted_title = note_file::extract_title(body);
        let stem = if conflicted_title.is_empty() {
            "New Note"
        } else {
            &conflicted_title
        };
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let conflict_path = unique_filename(dir, &format!("{stem} (Conflicted Copy {date})"), None);
        let conflict_fm = FrontMatter {
            id: Uuid::new_v4().to_string(),
            tags: Vec::new(),
            pinned: false,
            created_at: now_rfc3339(),
            deleted_at: None,
            is_template: false,
        };
        fs::write(&conflict_path, note_file::serialize(&conflict_fm, body)).map_err(|e| e.to_string())?;
        let conflicted_note_id = upsert_note_file(conn, notes_root, &conflict_path)?
            .ok_or_else(|| "failed to index conflicted copy".to_string())?;

        return Ok(SaveOutcome::Conflict {
            conflicted_note_id,
            conflicted_title,
            original_body: old_body,
            hash: on_disk_hash,
        });
    }

    versions::maybe_snapshot(notes_root, note_id, &old_body, false)?;
    write_body(conn, notes_root, note_id, body)?;
    Ok(SaveOutcome::Saved {
        hash: content_hash(body),
    })
}

/// Overwrites a note's body with one of its own past snapshots. The
/// content it replaces is itself snapshotted first (unconditionally), so
/// restoring is always undoable too.
pub fn restore_version(
    conn: &Connection,
    notes_root: &Path,
    note_id: &str,
    timestamp: &str,
) -> StoreResult<()> {
    let (_, _, current_body) = read_current(conn, notes_root, note_id)?;
    versions::maybe_snapshot(notes_root, note_id, &current_body, true)?;
    let restored_body = versions::get(notes_root, note_id, timestamp)?;
    write_body(conn, notes_root, note_id, &restored_body)
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

pub fn set_pinned(
    conn: &Connection,
    notes_root: &Path,
    note_id: &str,
    pinned: bool,
) -> StoreResult<()> {
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

pub fn add_tag_to_note(
    conn: &Connection,
    notes_root: &Path,
    note_id: &str,
    tag_name: &str,
) -> StoreResult<()> {
    let tag_name = tag_name.trim().to_string();
    if tag_name.is_empty() {
        return Err("tag name can't be empty".to_string());
    }
    mutate_front_matter(conn, notes_root, note_id, |fm| {
        if !fm.tags.iter().any(|t| t.eq_ignore_ascii_case(&tag_name)) {
            fm.tags.push(tag_name);
        }
    })
}

pub fn remove_tag_from_note(
    conn: &Connection,
    notes_root: &Path,
    note_id: &str,
    tag_name: &str,
) -> StoreResult<()> {
    mutate_front_matter(conn, notes_root, note_id, |fm| {
        fm.tags.retain(|t| !t.eq_ignore_ascii_case(tag_name));
    })
}

/// Permanently erases a note (right-click "Delete Permanently" from
/// Recently Deleted) — removes the file from disk, not just the DB row.
pub fn delete_note_permanently(
    conn: &Connection,
    notes_root: &Path,
    note_id: &str,
) -> StoreResult<()> {
    let rel: String = conn
        .query_row(
            "SELECT file_path FROM notes WHERE id = ?1",
            [note_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    fs::remove_file(notes_root.join(&rel)).ok();
    fs::remove_dir_all(notes_root.join(ATTACHMENTS_DIR).join(note_id)).ok();
    versions::delete_all(notes_root, note_id);
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
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
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

/// Where a note's dropped/pasted non-image files live on disk, keyed by
/// note id rather than alongside the note's own file - a note can be
/// renamed or moved between folders without that breaking its attachments'
/// `![[...]]` embeds, which store this path (see [`save_attachment`]).
const ATTACHMENTS_DIR: &str = ".attachments";

pub struct AttachmentInfo {
    pub path: String,
    pub name: String,
    pub size: u64,
}

/// Resolves an attachment's stored relative path (e.g.
/// ".attachments/<note-id>/<filename>", as embedded in a note's markdown)
/// to an absolute path, rejecting anything that would land outside
/// notes_root/.attachments - a note is a plain file a user (or a sync
/// conflict) could hand-edit, so a `![[../../../etc/passwd]]` embed must
/// not be stat-able or openable.
fn resolve_attachment_path(notes_root: &Path, rel_path: &str) -> StoreResult<PathBuf> {
    let attachments_root = notes_root.join(ATTACHMENTS_DIR);
    let canonical_root = attachments_root.canonicalize().map_err(|e| e.to_string())?;
    let canonical_candidate = notes_root
        .join(rel_path)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err("attachment path is outside the attachments folder".to_string());
    }
    Ok(canonical_candidate)
}

fn unique_attachment_path(dir: &Path, filename: &str) -> PathBuf {
    let path = Path::new(filename);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|s| s.to_str());
    let name = |n: u32| match (ext, n) {
        (Some(ext), 1) => format!("{stem}.{ext}"),
        (Some(ext), n) => format!("{stem} {n}.{ext}"),
        (None, 1) => stem.to_string(),
        (None, n) => format!("{stem} {n}"),
    };
    let mut n = 1;
    let mut candidate = dir.join(name(n));
    while candidate.exists() {
        n += 1;
        candidate = dir.join(name(n));
    }
    candidate
}

/// Saves a dropped/pasted file's bytes under notes_root/.attachments/<note
/// id>/, returning the path (relative to notes_root, forward-slashed) to
/// embed as `![[path]]` in the note's markdown. Only the filename's own
/// basename is trusted - a dragged-in file could be named with path
/// separators.
pub fn save_attachment(
    notes_root: &Path,
    note_id: &str,
    filename: &str,
    bytes: &[u8],
) -> StoreResult<AttachmentInfo> {
    let safe_name = Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .unwrap_or("file");
    let dir = notes_root.join(ATTACHMENTS_DIR).join(note_id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = unique_attachment_path(&dir, safe_name);
    fs::write(&path, bytes).map_err(|e| e.to_string())?;

    let rel_path = path
        .strip_prefix(notes_root)
        .map_err(|e| e.to_string())?
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    Ok(AttachmentInfo {
        path: rel_path,
        name,
        size: bytes.len() as u64,
    })
}

pub fn attachment_size(notes_root: &Path, rel_path: &str) -> StoreResult<u64> {
    let abs = resolve_attachment_path(notes_root, rel_path)?;
    fs::metadata(&abs)
        .map(|m| m.len())
        .map_err(|e| e.to_string())
}

pub fn attachment_absolute_path(notes_root: &Path, rel_path: &str) -> StoreResult<PathBuf> {
    resolve_attachment_path(notes_root, rel_path)
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

/// Re-embeds any active note whose text has changed since it was last
/// embedded (or that's never been embedded), and drops embedding rows for
/// notes that are gone or moved to Recently Deleted. Runs lazily from
/// `smart_search` rather than on every save, so a note that's never
/// searched never costs an Ollama round trip; a note whose text hasn't
/// changed since its last embedding is skipped via `content_hash`.
///
/// Takes the DB mutex rather than an already-locked `Connection` and
/// re-locks it around each individual DB read/write, specifically so the
/// lock is *not* held across `embeddings::embed`'s blocking HTTP call —
/// otherwise indexing an un-embedded vault would stall every other DB-
/// touching command (autosave, note list refresh, ...) for as long as
/// re-embedding takes.
pub fn ensure_embeddings_current(db: &std::sync::Mutex<Connection>) -> StoreResult<()> {
    let active_notes: Vec<(String, String, String)> = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id, title, plaintext_content FROM notes WHERE deleted_at IS NULL")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };
    let active_ids: Vec<String> = active_notes.iter().map(|(id, _, _)| id.clone()).collect();

    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let embedded_ids: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT note_id FROM note_embeddings")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?
        };
        for id in &embedded_ids {
            if !active_ids.contains(id) {
                conn.execute("DELETE FROM note_embeddings WHERE note_id = ?1", [id])
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    for (id, title, body) in active_notes {
        let text = format!("{title}\n\n{body}");
        if text.trim().is_empty() {
            continue;
        }
        let hash = content_hash(&text);

        let existing_hash: Option<String> = {
            let conn = db.lock().map_err(|e| e.to_string())?;
            conn.query_row(
                "SELECT content_hash FROM note_embeddings WHERE note_id = ?1",
                [&id],
                |r| r.get(0),
            )
            .ok()
        };
        if existing_hash.as_deref() == Some(hash.as_str()) {
            continue;
        }

        let vector = embeddings::embed(&text)?; // no lock held during this HTTP call
        let bytes = embeddings::vector_to_bytes(&vector);

        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO note_embeddings (note_id, model, content_hash, vector, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(note_id) DO UPDATE SET
               model = excluded.model,
               content_hash = excluded.content_hash,
               vector = excluded.vector,
               updated_at = excluded.updated_at",
            params![id, embeddings::MODEL, hash, bytes, now_rfc3339()],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Semantic ("Smart") search: embeds `query`, ranks every active note's
/// stored embedding by cosine similarity, and returns the top matches'
/// ids (best first). Re-embeds any stale/missing notes first, so results
/// always reflect current on-disk content. Errors (most commonly: Ollama
/// isn't running) propagate as-is — the caller falls back to FTS5 search.
///
/// See `ensure_embeddings_current` for why this takes the DB mutex
/// instead of a `Connection`: the query embedding is also a blocking
/// HTTP call and must not be made while holding the lock.
fn all_note_vectors(conn: &Connection) -> StoreResult<Vec<(String, Vec<u8>)>> {
    let mut stmt = conn
        .prepare(
            "SELECT ne.note_id, ne.vector
             FROM note_embeddings ne
             JOIN notes n ON n.id = ne.note_id
             WHERE n.deleted_at IS NULL",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn rank_by_similarity(target: &[f32], rows: Vec<(String, Vec<u8>)>, limit: usize) -> Vec<String> {
    let mut scored: Vec<(String, f32)> = rows
        .into_iter()
        .map(|(id, bytes)| {
            let vector = embeddings::bytes_to_vector(&bytes);
            let score = embeddings::cosine_similarity(target, &vector);
            (id, score)
        })
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.truncate(limit);
    scored.into_iter().map(|(id, _)| id).collect()
}

pub fn smart_search(
    db: &std::sync::Mutex<Connection>,
    query: &str,
    limit: usize,
) -> StoreResult<Vec<String>> {
    ensure_embeddings_current(db)?;

    let query_vector = embeddings::embed(query)?;

    let rows = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        all_note_vectors(&conn)?
    };

    Ok(rank_by_similarity(&query_vector, rows, limit))
}

/// Notes most semantically similar to `note_id`, ranked by cosine
/// similarity between their stored embeddings (see `smart_search` for the
/// same mechanism applied to a typed query instead of another note).
/// Empty if `note_id` has no embedding yet (e.g. a blank note, or Ollama
/// has never run) - there's nothing to compare it against.
pub fn related_notes(
    db: &std::sync::Mutex<Connection>,
    note_id: &str,
    limit: usize,
) -> StoreResult<Vec<String>> {
    ensure_embeddings_current(db)?;

    let rows = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        all_note_vectors(&conn)?
    };

    let Some(target_bytes) = rows.iter().find(|(id, _)| id == note_id).map(|(_, v)| v.clone())
    else {
        return Ok(Vec::new());
    };
    let target_vector = embeddings::bytes_to_vector(&target_bytes);
    let others: Vec<(String, Vec<u8>)> = rows.into_iter().filter(|(id, _)| id != note_id).collect();

    Ok(rank_by_similarity(&target_vector, others, limit))
}

fn unique_path_for_name(dir: &Path, filename: &std::ffi::OsStr) -> PathBuf {
    let name_path = Path::new(filename);
    let stem = name_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = name_path
        .extension()
        .map(|s| s.to_string_lossy().to_string());

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
    fn substitute_template_variables_fills_in_date_and_time() {
        let now = Local::now();
        let rendered = substitute_template_variables("Meeting {{date}} at {{time}}\n{{date}} again");
        assert_eq!(
            rendered,
            format!(
                "Meeting {} at {}\n{} again",
                now.format("%Y-%m-%d"),
                now.format("%H:%M"),
                now.format("%Y-%m-%d"),
            )
        );
    }

    #[test]
    fn create_note_from_template_substitutes_variables_but_leaves_the_template_itself_alone() {
        let app_dir = temp_dir("tmpl-app");
        let notes_root = temp_dir("tmpl-root");
        let conn = db::init(&app_dir).unwrap();

        let template_id = create_note(&conn, &notes_root, "", true).unwrap();
        save_note_body(&conn, &notes_root, &template_id, "Daily Log\n{{date}}: ", None).unwrap();

        let new_id =
            create_note_from_template(&conn, &notes_root, "", &template_id).unwrap();

        let new_rel: String = conn
            .query_row("SELECT file_path FROM notes WHERE id = ?1", [&new_id], |r| r.get(0))
            .unwrap();
        let new_body = read_note_body(&notes_root, &new_rel).unwrap();
        assert!(!new_body.contains("{{date}}"), "the new note should have a real date, not the placeholder");
        assert!(new_body.contains(&Local::now().format("%Y-%m-%d").to_string()));

        // The template's own saved file must still hold the literal
        // placeholder - only the copy made from it gets substituted.
        let template_rel: String = conn
            .query_row(
                "SELECT file_path FROM notes WHERE id = ?1",
                [&template_id],
                |r| r.get(0),
            )
            .unwrap();
        let template_body = read_note_body(&notes_root, &template_rel).unwrap();
        assert!(template_body.contains("{{date}}"));
    }

    #[test]
    fn create_edit_rename_and_delete_round_trip() {
        let app_dir = temp_dir("app");
        let notes_root = temp_dir("root");
        let conn = db::init(&app_dir).unwrap();

        full_rescan(&conn, &notes_root).unwrap();

        let id = create_note(&conn, &notes_root, "", false).unwrap();
        save_note_body(&conn, &notes_root, &id, "Grocery list\nMilk, eggs", None).unwrap();

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
        save_note_body(&conn, &notes_root, &id, "Shopping list\nMilk, eggs", None).unwrap();
        let file_path: String = conn
            .query_row("SELECT file_path FROM notes WHERE id = ?1", [&id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(file_path, "Shopping list.md");
        assert!(!notes_root.join("Grocery list.md").exists());
        assert!(notes_root.join("Shopping list.md").exists());

        set_pinned(&conn, &notes_root, &id, true).unwrap();
        let pinned: bool = conn
            .query_row("SELECT is_pinned FROM notes WHERE id = ?1", [&id], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(pinned);

        set_deleted(&conn, &notes_root, &id, true).unwrap();
        let deleted_at: Option<String> = conn
            .query_row("SELECT deleted_at FROM notes WHERE id = ?1", [&id], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(deleted_at.is_some());

        // A full rescan should reproduce the exact same index state.
        full_rescan(&conn, &notes_root).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn save_with_matching_hash_saves_in_place() {
        let app_dir = temp_dir("conflict-app1");
        let notes_root = temp_dir("conflict-root1");
        let conn = db::init(&app_dir).unwrap();

        let id = create_note(&conn, &notes_root, "", false).unwrap();
        let outcome = save_note_body(&conn, &notes_root, &id, "Original\nbody", None).unwrap();
        let hash = match outcome {
            SaveOutcome::Saved { hash } => hash,
            SaveOutcome::Conflict { .. } => panic!("expected a clean save"),
        };

        let outcome =
            save_note_body(&conn, &notes_root, &id, "Original\nedited body", Some(&hash)).unwrap();
        assert!(matches!(outcome, SaveOutcome::Saved { .. }));
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "no conflicted copy should have been created");
    }

    #[test]
    fn save_with_stale_hash_creates_conflicted_copy_without_clobbering() {
        let app_dir = temp_dir("conflict-app2");
        let notes_root = temp_dir("conflict-root2");
        let conn = db::init(&app_dir).unwrap();

        let id = create_note(&conn, &notes_root, "", false).unwrap();
        save_note_body(&conn, &notes_root, &id, "Original\nbody", None).unwrap();

        // Simulate another device syncing in a change after this editor
        // loaded the note but before it saved.
        save_note_body(&conn, &notes_root, &id, "Original\nsynced in from elsewhere", None).unwrap();

        let outcome = save_note_body(
            &conn,
            &notes_root,
            &id,
            "Original\nthis editor's own edit",
            Some(&content_hash("Original\nbody")),
        )
        .unwrap();

        let (conflicted_note_id, original_body) = match outcome {
            SaveOutcome::Conflict {
                conflicted_note_id,
                original_body,
                ..
            } => (conflicted_note_id, original_body),
            SaveOutcome::Saved { .. } => panic!("expected a conflict"),
        };
        assert_eq!(original_body, "Original\nsynced in from elsewhere");

        // The original note's file must be untouched...
        let on_disk_body = read_note_body(&notes_root, &{
            let rel: String = conn
                .query_row("SELECT file_path FROM notes WHERE id = ?1", [&id], |r| r.get(0))
                .unwrap();
            rel
        })
        .unwrap();
        assert_eq!(on_disk_body, "Original\nsynced in from elsewhere");

        // ...and this editor's edit must have landed in a new note instead
        // of being lost.
        let conflicted_body: String = {
            let rel: String = conn
                .query_row(
                    "SELECT file_path FROM notes WHERE id = ?1",
                    [&conflicted_note_id],
                    |r| r.get(0),
                )
                .unwrap();
            read_note_body(&notes_root, &rel).unwrap()
        };
        assert_eq!(conflicted_body, "Original\nthis editor's own edit");

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn restore_version_brings_back_old_content_and_snapshots_current() {
        let app_dir = temp_dir("restore-app");
        let notes_root = temp_dir("restore-root");
        let conn = db::init(&app_dir).unwrap();

        let id = create_note(&conn, &notes_root, "", false).unwrap();
        save_note_body(&conn, &notes_root, &id, "Version one", None).unwrap();
        // Force a real snapshot of "Version one" before overwriting it,
        // bypassing the normal throttle so the test doesn't depend on
        // wall-clock timing.
        versions::maybe_snapshot(&notes_root, &id, "Version one", true).unwrap();
        save_note_body(&conn, &notes_root, &id, "Version two", None).unwrap();

        let snapshots = versions::list(&notes_root, &id).unwrap();
        let old_version = snapshots
            .iter()
            .find(|v| v.preview == "Version one")
            .expect("the first version should be listed");

        restore_version(&conn, &notes_root, &id, &old_version.timestamp).unwrap();

        let rel: String = conn
            .query_row("SELECT file_path FROM notes WHERE id = ?1", [&id], |r| r.get(0))
            .unwrap();
        assert_eq!(read_note_body(&notes_root, &rel).unwrap(), "Version one");

        // Restoring must itself have snapshotted "Version two" so the
        // restore is undoable.
        let snapshots_after = versions::list(&notes_root, &id).unwrap();
        assert!(snapshots_after.iter().any(|v| v.preview == "Version two"));
    }

    #[test]
    fn rescan_removes_notes_deleted_externally() {
        let app_dir = temp_dir("app2");
        let notes_root = temp_dir("root2");
        let conn = db::init(&app_dir).unwrap();

        let id = create_note(&conn, &notes_root, "", false).unwrap();
        full_rescan(&conn, &notes_root).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let file_path: String = conn
            .query_row("SELECT file_path FROM notes WHERE id = ?1", [&id], |r| {
                r.get(0)
            })
            .unwrap();
        fs::remove_file(notes_root.join(file_path)).unwrap();

        full_rescan(&conn, &notes_root).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn import_markdown_folder_claims_files_and_preserves_subfolders() {
        let app_dir = temp_dir("import-app");
        let notes_root = temp_dir("import-root");
        let conn = db::init(&app_dir).unwrap();
        full_rescan(&conn, &notes_root).unwrap();

        let source = temp_dir("import-source");
        fs::create_dir_all(source.join("Sub")).unwrap();
        fs::create_dir_all(source.join(".obsidian")).unwrap();
        fs::write(source.join("Top level.md"), "Top level\nsome text").unwrap();
        fs::write(
            source.join("Sub").join("Nested.md"),
            "---\ntags: [foo]\n---\nNested\nlinks to [[Top level]]",
        )
        .unwrap();
        fs::write(source.join("Sub").join("ignored.txt"), "not markdown").unwrap();
        fs::write(source.join(".obsidian").join("config.md"), "should be skipped").unwrap();

        let imported = import_markdown_folder(&conn, &notes_root, &source).unwrap();
        assert_eq!(imported, 2, "only the two real .md files should be imported");

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);

        let source_name = source.file_name().unwrap().to_string_lossy().to_string();
        assert!(notes_root.join(&source_name).join("Top level.md").exists());
        assert!(
            notes_root
                .join(&source_name)
                .join("Sub")
                .join("Nested.md")
                .exists()
        );

        // The foreign YAML front matter must have been stripped, not kept
        // as literal body text, and the file claimed with a fresh id.
        let nested_body = read_note_body(
            &notes_root,
            &format!("{source_name}/Sub/Nested.md"),
        )
        .unwrap();
        assert_eq!(nested_body, "Nested\nlinks to [[Top level]]");
    }

    #[test]
    fn note_in_subfolder_gets_that_folders_id() {
        let app_dir = temp_dir("app4");
        let notes_root = temp_dir("root4");
        let conn = db::init(&app_dir).unwrap();

        let id = create_note(&conn, &notes_root, "Work", false).unwrap();

        let folder_id: String = conn
            .query_row("SELECT folder_id FROM notes WHERE id = ?1", [&id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(folder_id, "Work");

        // A rescan (the path the app actually runs on startup) must agree.
        full_rescan(&conn, &notes_root).unwrap();
        let folder_id_after_rescan: String = conn
            .query_row("SELECT folder_id FROM notes WHERE id = ?1", [&id], |r| {
                r.get(0)
            })
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
    fn add_and_remove_note_tags() {
        let app_dir = temp_dir("app8");
        let notes_root = temp_dir("root8");
        let conn = db::init(&app_dir).unwrap();

        let id = create_note(&conn, &notes_root, "", false).unwrap();
        add_tag_to_note(&conn, &notes_root, &id, "Work").unwrap();
        add_tag_to_note(&conn, &notes_root, &id, "urgent").unwrap();
        // Case-insensitive de-dup: adding "work" again should be a no-op.
        add_tag_to_note(&conn, &notes_root, &id, "work").unwrap();

        let tag_names: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT t.name FROM tags t
                     JOIN note_tags nt ON nt.tag_id = t.id
                     WHERE nt.note_id = ?1 ORDER BY t.name",
                )
                .unwrap();
            stmt.query_map([&id], |r| r.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(tag_names, vec!["Work".to_string(), "urgent".to_string()]);

        remove_tag_from_note(&conn, &notes_root, &id, "Work").unwrap();
        let tag_names_after: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT t.name FROM tags t
                     JOIN note_tags nt ON nt.tag_id = t.id
                     WHERE nt.note_id = ?1",
                )
                .unwrap();
            stmt.query_map([&id], |r| r.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(tag_names_after, vec!["urgent".to_string()]);

        // Removing a tag's last reference should garbage-collect the tag
        // row itself, not just the note_tags link (regression: it used to
        // linger forever and still show up as a filter with no notes in it).
        let work_tag_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tags WHERE name = 'Work')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            !work_tag_exists,
            "orphaned tag should have been garbage-collected"
        );

        let urgent_tag_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tags WHERE name = 'urgent')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(urgent_tag_exists, "still-used tag should not be removed");
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

        let note_id = create_note(&conn, &notes_root, "", false).unwrap();
        move_note(&conn, &notes_root, &note_id, &work_id).unwrap();
        let folder_id: String = conn
            .query_row(
                "SELECT folder_id FROM notes WHERE id = ?1",
                [&note_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(folder_id, "Work");

        // Can't delete a non-empty folder.
        assert!(delete_folder(&conn, &notes_root, &work_id).is_err());

        let renamed_id = rename_folder(&conn, &notes_root, &work_id, "Projects").unwrap();
        assert_eq!(renamed_id, "Projects");
        assert!(!notes_root.join("Work").exists());
        assert!(notes_root.join("Projects").is_dir());
        let folder_id_after_rename: String = conn
            .query_row(
                "SELECT folder_id FROM notes WHERE id = ?1",
                [&note_id],
                |r| r.get(0),
            )
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

        let id = create_note(&conn, &notes_root, "", false).unwrap();
        let rel: String = conn
            .query_row("SELECT file_path FROM notes WHERE id = ?1", [&id], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(notes_root.join(&rel).exists());

        delete_note_permanently(&conn, &notes_root, &id).unwrap();
        assert!(!notes_root.join(&rel).exists());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn purge_expired_trash_removes_only_notes_past_retention() {
        let app_dir = temp_dir("app7");
        let notes_root = temp_dir("root7");
        let conn = db::init(&app_dir).unwrap();

        let old_id = create_note(&conn, &notes_root, "", false).unwrap();
        let recent_id = create_note(&conn, &notes_root, "", false).unwrap();
        let kept_id = create_note(&conn, &notes_root, "", false).unwrap(); // never deleted

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
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
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
        assert_eq!(
            build_fts_query("title:foo AND bar"),
            "\"title:foo\"* \"AND\"* \"bar\"*"
        );
    }

    #[test]
    fn related_notes_ranks_by_similarity_and_excludes_self() {
        let app_dir = temp_dir("related-app");
        let notes_root = temp_dir("related-root");
        let conn = db::init(&app_dir).unwrap();

        let id_a = create_note(&conn, &notes_root, "", false).unwrap();
        save_note_body(&conn, &notes_root, &id_a, "Note A\nbody a", None).unwrap();
        let id_b = create_note(&conn, &notes_root, "", false).unwrap();
        save_note_body(&conn, &notes_root, &id_b, "Note B\nbody b", None).unwrap();
        let id_c = create_note(&conn, &notes_root, "", false).unwrap();
        save_note_body(&conn, &notes_root, &id_c, "Note C\nbody c", None).unwrap();

        // Pre-seed embeddings whose content_hash already matches each
        // note's current body, so ensure_embeddings_current (called by
        // related_notes) treats them as up to date and never reaches out
        // to Ollama - this test is about the ranking math, not the HTTP
        // call. B's vector is deliberately close to A's, C's far from it.
        for (id, title, body, vector) in [
            (&id_a, "Note A", "body a", vec![1.0_f32, 0.0]),
            (&id_b, "Note B", "body b", vec![0.9_f32, 0.1]),
            (&id_c, "Note C", "body c", vec![0.0_f32, 1.0]),
        ] {
            let hash = content_hash(&format!("{title}\n\n{body}"));
            let bytes = embeddings::vector_to_bytes(&vector);
            conn.execute(
                "INSERT INTO note_embeddings (note_id, model, content_hash, vector, updated_at)
                 VALUES (?1, 'test', ?2, ?3, ?4)",
                params![id, hash, bytes, now_rfc3339()],
            )
            .unwrap();
        }

        let db_mutex = std::sync::Mutex::new(conn);
        let ranked = related_notes(&db_mutex, &id_a, 5).unwrap();

        assert_eq!(ranked, vec![id_b, id_c], "B (closer vector) should rank before C, and A must not rank itself");
    }

    #[test]
    fn save_attachment_writes_file_and_dedupes_names() {
        let notes_root = temp_dir("attach-root");
        fs::create_dir_all(&notes_root).unwrap();

        let info = save_attachment(&notes_root, "note-1", "report.pdf", b"first").unwrap();
        assert_eq!(info.path, ".attachments/note-1/report.pdf");
        assert_eq!(info.name, "report.pdf");
        assert_eq!(info.size, 5);

        // A second attachment with the same filename gets a distinct name
        // instead of overwriting the first.
        let info2 = save_attachment(&notes_root, "note-1", "report.pdf", b"second").unwrap();
        assert_eq!(info2.path, ".attachments/note-1/report 2.pdf");

        assert_eq!(
            fs::read(notes_root.join(&info.path)).unwrap(),
            b"first".to_vec()
        );
        assert_eq!(
            fs::read(notes_root.join(&info2.path)).unwrap(),
            b"second".to_vec()
        );

        // A hostile filename can't escape into an arbitrary location -
        // only its basename is used.
        let escaped = save_attachment(&notes_root, "note-2", "../../etc/evil.txt", b"x").unwrap();
        assert_eq!(escaped.path, ".attachments/note-2/evil.txt");
    }

    #[test]
    fn attachment_size_rejects_paths_outside_attachments_dir() {
        let notes_root = temp_dir("attach-size-root");
        fs::create_dir_all(&notes_root).unwrap();

        let info = save_attachment(&notes_root, "note-1", "photo.png", b"bytes!!").unwrap();
        assert_eq!(attachment_size(&notes_root, &info.path).unwrap(), 7);

        // A secret file elsewhere under notes_root is not reachable by
        // crafting a path that starts with "..".
        let secret_dir = notes_root.join("secret");
        fs::create_dir_all(&secret_dir).unwrap();
        fs::write(secret_dir.join("private.txt"), b"shh").unwrap();
        assert!(attachment_size(&notes_root, "../secret/private.txt").is_err());
        assert!(attachment_size(&notes_root, ".attachments/../secret/private.txt").is_err());
    }

    #[test]
    fn delete_note_permanently_removes_its_attachments_dir() {
        let app_dir = temp_dir("attach-del-app");
        let notes_root = temp_dir("attach-del-root");
        let conn = db::init(&app_dir).unwrap();

        let id = create_note(&conn, &notes_root, "", false).unwrap();
        save_attachment(&notes_root, &id, "notes.txt", b"hi").unwrap();
        assert!(notes_root.join(ATTACHMENTS_DIR).join(&id).exists());

        delete_note_permanently(&conn, &notes_root, &id).unwrap();
        assert!(!notes_root.join(ATTACHMENTS_DIR).join(&id).exists());
    }

    #[test]
    fn daily_note_is_found_not_duplicated_on_a_second_call() {
        let app_dir = temp_dir("daily-app");
        let notes_root = temp_dir("daily-root");
        let conn = db::init(&app_dir).unwrap();

        let first = get_or_create_daily_note(&conn, &notes_root, "2026-10-05").unwrap();
        assert!(notes_root.join("Daily Notes/2026-10-05.md").exists());

        let second = get_or_create_daily_note(&conn, &notes_root, "2026-10-05").unwrap();
        assert_eq!(first, second);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // A different date gets its own, separate note.
        let third = get_or_create_daily_note(&conn, &notes_root, "2026-10-06").unwrap();
        assert_ne!(first, third);
    }

    #[test]
    fn daily_note_rejects_a_malformed_date() {
        let app_dir = temp_dir("daily-bad-app");
        let notes_root = temp_dir("daily-bad-root");
        let conn = db::init(&app_dir).unwrap();

        assert!(get_or_create_daily_note(&conn, &notes_root, "../../etc/passwd").is_err());
        assert!(get_or_create_daily_note(&conn, &notes_root, "2026-10-5").is_err());
        assert!(get_or_create_daily_note(&conn, &notes_root, "not-a-date").is_err());
    }
}
