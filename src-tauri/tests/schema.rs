use app_lib::db;

#[test]
fn migrations_create_expected_schema_and_fts_works() {
    let tmp = tempfile_dir();
    let conn = db::init(&tmp).expect("db init should succeed");

    // Root folder seeded (id '' = the notes root itself, shown as "Notes").
    let folder_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM folders", [], |r| r.get(0))
        .unwrap();
    assert_eq!(folder_count, 1);

    // Insert a note and confirm FTS5 picks it up via triggers. Note content
    // itself lives in Markdown files on disk (see store.rs); this table
    // only mirrors title/plaintext_content for fast search.
    conn.execute(
        "INSERT INTO notes (id, file_path, title, plaintext_content, folder_id, created_at, updated_at)
         VALUES ('n1', 'Grocery List.md', 'Grocery List', 'milk eggs bread', '', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    )
    .unwrap();

    let matches: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'milk'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(matches, 1);

    // Deleting the note should remove it from the FTS index too.
    conn.execute("DELETE FROM notes WHERE id = 'n1'", [])
        .unwrap();
    let matches_after_delete: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'milk'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(matches_after_delete, 0);

    // Re-running init (as happens on every app start) must be idempotent.
    drop(conn);
    let conn2 = db::init(&tmp).expect("second init should also succeed");
    let folder_count_after: i64 = conn2
        .query_row("SELECT COUNT(*) FROM folders", [], |r| r.get(0))
        .unwrap();
    assert_eq!(folder_count_after, 1);
}

fn tempfile_dir() -> std::path::PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!("notes-schema-test-{}", uuid::Uuid::new_v4()));
    dir
}
