use std::fs;
use std::path::{Path, PathBuf};

use chrono::{TimeZone, Utc};
use serde::Serialize;

use crate::note_file;
use crate::store::StoreResult;
use crate::vault::NoteCodec;

/// Snapshots live under notes_root/.versions/<note-id>/<epoch-millis>.md,
/// a hidden sibling of the notes themselves - like .attachments, this
/// keeps them out of the main full_rescan walk (which skips dot-prefixed
/// entries) while still traveling with the notes folder across a sync.
pub(crate) const VERSIONS_DIR: &str = ".versions";

/// Snapshots are throttled to roughly this often per note while actively
/// editing, so autosave (which fires on every ~500ms pause in typing)
/// doesn't produce a snapshot per pause. This is "recover from a bad edit
/// from a while back", not a full undo history.
const SNAPSHOT_MIN_INTERVAL_SECS: i64 = 300;

/// Oldest snapshots beyond this count are pruned so a long-lived note's
/// version folder doesn't grow forever.
const MAX_SNAPSHOTS_PER_NOTE: usize = 50;

fn dir_for(notes_root: &Path, note_id: &str) -> PathBuf {
    notes_root.join(VERSIONS_DIR).join(note_id)
}

fn filename_for(millis: i64) -> String {
    format!("{millis}.md")
}

fn millis_from_filename(name: &str) -> Option<i64> {
    name.strip_suffix(".md")?.parse().ok()
}

fn list_millis(dir: &Path) -> Vec<i64> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut millis: Vec<i64> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| millis_from_filename(&e.file_name().to_string_lossy()))
        .collect();
    millis.sort_unstable();
    millis
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub timestamp: String,
    pub preview: String,
}

/// Snapshots `body` for `note_id` if enough time has passed since the last
/// snapshot, or unconditionally when `force` (used right before a restore
/// overwrites the current content, so the restore itself is always
/// undoable too). A no-op for a blank body - there's nothing worth
/// recovering in an empty note.
pub fn maybe_snapshot(
    notes_root: &Path,
    note_id: &str,
    body: &str,
    force: bool,
    codec: &NoteCodec,
) -> StoreResult<()> {
    if body.trim().is_empty() {
        return Ok(());
    }

    let dir = dir_for(notes_root, note_id);
    let mut millis = list_millis(&dir);

    if !force
        && let Some(&last) = millis.last()
        && Utc::now().timestamp_millis() - last < SNAPSHOT_MIN_INTERVAL_SECS * 1000
    {
        return Ok(());
    }

    // Two snapshots landing in the same millisecond (e.g. a forced
    // snapshot right after a throttled one, as `restore_version` can
    // trigger) must not overwrite each other - bump forward until free.
    let mut now = Utc::now().timestamp_millis();
    while millis.contains(&now) {
        now += 1;
    }

    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    codec.write(&dir.join(filename_for(now)), body)?;

    millis.push(now);
    if millis.len() > MAX_SNAPSHOTS_PER_NOTE {
        for old in &millis[..millis.len() - MAX_SNAPSHOTS_PER_NOTE] {
            fs::remove_file(dir.join(filename_for(*old))).ok();
        }
    }
    Ok(())
}

/// Lists a note's snapshots, newest first.
pub fn list(notes_root: &Path, note_id: &str, codec: &NoteCodec) -> StoreResult<Vec<VersionInfo>> {
    let dir = dir_for(notes_root, note_id);
    let mut millis = list_millis(&dir);
    millis.reverse();

    millis
        .into_iter()
        .map(|m| {
            let body = codec.read(&dir.join(filename_for(m)))?;
            let timestamp = Utc
                .timestamp_millis_opt(m)
                .single()
                .map(|t| t.to_rfc3339())
                .unwrap_or_default();
            Ok(VersionInfo {
                timestamp,
                preview: note_file::extract_title(&body),
            })
        })
        .collect()
}

fn path_for_timestamp(notes_root: &Path, note_id: &str, timestamp: &str) -> StoreResult<PathBuf> {
    let at = chrono::DateTime::parse_from_rfc3339(timestamp).map_err(|e| e.to_string())?;
    Ok(dir_for(notes_root, note_id).join(filename_for(at.timestamp_millis())))
}

/// Reads one snapshot's body back out.
pub fn get(
    notes_root: &Path,
    note_id: &str,
    timestamp: &str,
    codec: &NoteCodec,
) -> StoreResult<String> {
    let path = path_for_timestamp(notes_root, note_id, timestamp)?;
    codec.read(&path)
}

/// Deletes every snapshot for a note, called when the note itself is
/// erased for good so orphaned version files don't pile up forever.
pub fn delete_all(notes_root: &Path, note_id: &str) {
    fs::remove_dir_all(dir_for(notes_root, note_id)).ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temp_dir(label: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!("notes-versions-test-{label}-{}", Uuid::new_v4()));
        dir
    }

    #[test]
    fn snapshots_throttle_but_force_bypasses() {
        let root = temp_dir("throttle");

        maybe_snapshot(&root, "n1", "first", false, &NoteCodec::Plain).unwrap();
        maybe_snapshot(&root, "n1", "second", false, &NoteCodec::Plain).unwrap();
        let versions = list(&root, "n1", &NoteCodec::Plain).unwrap();
        assert_eq!(versions.len(), 1, "second snapshot should be throttled");

        maybe_snapshot(&root, "n1", "third", true, &NoteCodec::Plain).unwrap();
        let versions = list(&root, "n1", &NoteCodec::Plain).unwrap();
        assert_eq!(versions.len(), 2, "forced snapshot bypasses the throttle");
        assert_eq!(versions[0].preview, "third", "newest listed first");
    }

    #[test]
    fn blank_body_is_never_snapshotted() {
        let root = temp_dir("blank");
        maybe_snapshot(&root, "n1", "   \n  ", false, &NoteCodec::Plain).unwrap();
        assert!(list(&root, "n1", &NoteCodec::Plain).unwrap().is_empty());
    }

    #[test]
    fn get_round_trips_a_listed_snapshot() {
        let root = temp_dir("roundtrip");
        maybe_snapshot(&root, "n1", "hello world", false, &NoteCodec::Plain).unwrap();
        let versions = list(&root, "n1", &NoteCodec::Plain).unwrap();
        let body = get(&root, "n1", &versions[0].timestamp, &NoteCodec::Plain).unwrap();
        assert_eq!(body, "hello world");
    }

    #[test]
    fn delete_all_removes_the_note_versions_dir() {
        let root = temp_dir("delete-all");
        maybe_snapshot(&root, "n1", "hello", false, &NoteCodec::Plain).unwrap();
        delete_all(&root, "n1");
        assert!(list(&root, "n1", &NoteCodec::Plain).unwrap().is_empty());
    }

    #[test]
    fn snapshots_are_encrypted_on_disk_for_an_encrypted_vault() {
        let root = temp_dir("encrypted");
        let codec = NoteCodec::Encrypted(crate::vault_crypto::random_vault_key());

        maybe_snapshot(&root, "n1", "Secret Title\nsecret body", false, &codec).unwrap();
        let versions = list(&root, "n1", &codec).unwrap();
        assert_eq!(versions[0].preview, "Secret Title");
        assert_eq!(
            get(&root, "n1", &versions[0].timestamp, &codec).unwrap(),
            "Secret Title\nsecret body"
        );

        // The wrong key must not be able to read it, and the raw bytes on
        // disk must not contain the plaintext title/body.
        let wrong_codec = NoteCodec::Encrypted(crate::vault_crypto::random_vault_key());
        assert!(get(&root, "n1", &versions[0].timestamp, &wrong_codec).is_err());
        let raw = fs::read(
            dir_for(&root, "n1").join(filename_for(
                chrono::DateTime::parse_from_rfc3339(&versions[0].timestamp)
                    .unwrap()
                    .timestamp_millis(),
            )),
        )
        .unwrap();
        assert!(!raw.windows(6).any(|w| w == b"secret"));
    }
}
