use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

/// Embedded schema migrations, applied in order on every startup. Each is
/// tracked in `schema_migrations` and runs at most once per database, so a
/// migration doesn't need to be idempotent on its own (e.g. a later one may
/// safely `DROP TABLE` and redefine what an earlier one created).
const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", include_str!("../migrations/0001_init.sql")),
    (
        "0002_file_backed_notes",
        include_str!("../migrations/0002_file_backed_notes.sql"),
    ),
];

pub struct DbState(pub Mutex<Connection>);

pub fn init(app_data_dir: &Path) -> rusqlite::Result<Connection> {
    std::fs::create_dir_all(app_data_dir).expect("failed to create app data directory");
    let db_path = app_data_dir.join("notes.sqlite3");

    let conn = Connection::open(db_path)?;
    // `journal_mode` returns the resulting mode as a row, so it can't go
    // through `pragma_update` (which expects no result set).
    conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get::<_, String>(0))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;

    run_migrations(&conn)?;

    Ok(conn)
}

fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            name       TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );",
    )?;

    for (name, sql) in MIGRATIONS {
        let already_applied: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE name = ?1)",
                [name],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if already_applied {
            continue;
        }

        conn.execute_batch(sql)?;
        conn.execute(
            "INSERT INTO schema_migrations (name) VALUES (?1)",
            [name],
        )?;
    }

    Ok(())
}
