use app_lib::db;

#[test]
fn migrations_create_expected_schema_and_fts_works() {
    let tmp = tempfile_dir();
    let conn = db::init(&tmp).expect("db init should succeed");

    // Default folder seeded.
    let folder_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM folders", [], |r| r.get(0))
        .unwrap();
    assert_eq!(folder_count, 1);

    // Insert a note and confirm FTS5 picks it up via triggers.
    conn.execute(
        "INSERT INTO notes (id, title, content, plaintext_content, folder_id)
         VALUES ('n1', 'Grocery List', '<p>milk eggs bread</p>', 'milk eggs bread', 'default')",
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
