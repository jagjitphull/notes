//! Full vault backup: zips up the whole notes_root folder exactly as it
//! sits on disk - every note, subfolder, attachment, and version-history
//! file. Unlike `store::import_markdown_folder`'s selective .md-only
//! copy (meant for porting content *in* from another app), this is a
//! byte-for-byte mirror meant for disaster recovery: restoring it is
//! just unzipping it back into a folder and pointing the app at it.

use std::fs;
use std::io::Write;
use std::path::Path;

use walkdir::WalkDir;
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
}
