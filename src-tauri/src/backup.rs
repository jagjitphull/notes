//! Full vault backup: zips up the whole notes_root folder exactly as it
//! sits on disk - every note, subfolder, attachment, and version-history
//! file. Unlike `store::import_markdown_folder`'s selective .md-only
//! copy (meant for porting content *in* from another app), this is a
//! byte-for-byte mirror meant for disaster recovery - `restore_vault`
//! below is the other half, unzipping one such backup back out.

use std::fs;
use std::io::Write;
use std::path::Path;

use walkdir::WalkDir;
use zip::ZipArchive;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::store::StoreResult;

pub fn export_vault(notes_root: &Path, dest_path: &Path) -> StoreResult<()> {
    let file = fs::File::create(dest_path).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);
    let options =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for entry in WalkDir::new(notes_root).min_depth(1) {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let rel = path.strip_prefix(notes_root).map_err(|e| e.to_string())?;
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        if entry.file_type().is_dir() {
            zip.add_directory(format!("{rel_str}/"), options)
                .map_err(|e| e.to_string())?;
        } else if entry.file_type().is_file() {
            zip.start_file(&rel_str, options).map_err(|e| e.to_string())?;
            let bytes = fs::read(path).map_err(|e| e.to_string())?;
            zip.write_all(&bytes).map_err(|e| e.to_string())?;
        }
    }

    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// Extracts a vault backup (see `export_vault`) into `dest_dir`, returning
/// the number of files written. Refuses to touch a destination that
/// already has anything in it - restoring is a "point me at a fresh
/// folder" operation, never a silent merge/overwrite of whatever's
/// already there. Each entry's path is resolved via `enclosed_name`,
/// which rejects anything that would escape `dest_dir` (an absolute path
/// or a `..` component) - a zip is as untrusted as any other file picked
/// up from outside the app.
pub fn restore_vault(zip_path: &Path, dest_dir: &Path) -> StoreResult<usize> {
    if dest_dir.exists() && fs::read_dir(dest_dir).map_err(|e| e.to_string())?.next().is_some() {
        return Err("destination folder is not empty".to_string());
    }
    fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;

    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;

    let mut count = 0usize;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel_path) = entry.enclosed_name() else {
            continue;
        };
        let out_path = dest_dir.join(rel_path);

        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out_file = fs::File::create(&out_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out_file).map_err(|e| e.to_string())?;
            count += 1;
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use uuid::Uuid;

    fn temp_path(label: &str) -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!("notes-backup-test-{label}-{}", Uuid::new_v4()));
        dir
    }

    #[test]
    fn zips_every_file_preserving_relative_paths_and_content() {
        let notes_root = temp_path("root");
        fs::create_dir_all(notes_root.join("Work")).unwrap();
        fs::create_dir_all(notes_root.join(".attachments").join("note-1")).unwrap();
        fs::write(notes_root.join("Top.md"), "---\nid: a\n---\nTop").unwrap();
        fs::write(notes_root.join("Work").join("Nested.md"), "Nested body").unwrap();
        fs::write(
            notes_root.join(".attachments").join("note-1").join("file.pdf"),
            b"%PDF-fake",
        )
        .unwrap();

        let dest = temp_path("dest").with_extension("zip");
        export_vault(&notes_root, &dest).unwrap();

        let file = fs::File::open(&dest).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        let mut names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        names.sort();
        assert!(names.contains(&"Top.md".to_string()));
        assert!(names.contains(&"Work/".to_string()));
        assert!(names.contains(&"Work/Nested.md".to_string()));
        assert!(names.contains(&".attachments/note-1/file.pdf".to_string()));

        let mut top = archive.by_name("Top.md").unwrap();
        let mut content = String::new();
        top.read_to_string(&mut content).unwrap();
        assert_eq!(content, "---\nid: a\n---\nTop");
    }

    #[test]
    fn errors_cleanly_when_notes_root_does_not_exist() {
        let missing = temp_path("missing");
        let dest = temp_path("dest2").with_extension("zip");
        assert!(export_vault(&missing, &dest).is_err());
    }

    #[test]
    fn restore_round_trips_a_backup_into_a_fresh_folder() {
        let notes_root = temp_path("restore-src");
        fs::create_dir_all(notes_root.join("Work")).unwrap();
        fs::write(notes_root.join("Top.md"), "---\nid: a\n---\nTop").unwrap();
        fs::write(notes_root.join("Work").join("Nested.md"), "Nested body").unwrap();

        let zip_path = temp_path("restore-zip").with_extension("zip");
        export_vault(&notes_root, &zip_path).unwrap();

        let dest = temp_path("restore-dest");
        let count = restore_vault(&zip_path, &dest).unwrap();

        assert_eq!(count, 2, "Top.md and Work/Nested.md");
        assert_eq!(fs::read_to_string(dest.join("Top.md")).unwrap(), "---\nid: a\n---\nTop");
        assert_eq!(
            fs::read_to_string(dest.join("Work").join("Nested.md")).unwrap(),
            "Nested body"
        );
    }

    #[test]
    fn restore_refuses_a_non_empty_destination() {
        let notes_root = temp_path("refuse-src");
        fs::create_dir_all(&notes_root).unwrap();
        fs::write(notes_root.join("Top.md"), "---\nid: a\n---\nTop").unwrap();
        let zip_path = temp_path("refuse-zip").with_extension("zip");
        export_vault(&notes_root, &zip_path).unwrap();

        let dest = temp_path("refuse-dest");
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("already-here.txt"), "don't clobber me").unwrap();

        assert!(restore_vault(&zip_path, &dest).is_err());
        // The pre-existing file must survive the refused restore untouched.
        assert_eq!(
            fs::read_to_string(dest.join("already-here.txt")).unwrap(),
            "don't clobber me"
        );
    }

    #[test]
    fn restore_ignores_a_zip_slip_entry_outside_the_destination() {
        // A hand-crafted zip (not one export_vault would ever produce)
        // with a path-traversal entry name, standing in for a malicious
        // or corrupted backup file.
        let zip_path = temp_path("slip-zip").with_extension("zip");
        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        zip.start_file("safe.txt", options).unwrap();
        zip.write_all(b"fine").unwrap();
        zip.start_file("../escape.txt", options).unwrap();
        zip.write_all(b"should never land outside dest").unwrap();
        zip.finish().unwrap();

        let dest = temp_path("slip-dest");
        let count = restore_vault(&zip_path, &dest).unwrap();

        assert_eq!(count, 1, "only the safe entry should be written");
        assert!(dest.join("safe.txt").exists());
        assert!(
            !dest.parent().unwrap().join("escape.txt").exists(),
            "the traversal entry must not have escaped dest_dir"
        );
    }
}
