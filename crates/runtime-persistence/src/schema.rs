use rusqlite::Connection;

pub fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS history_items (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            source TEXT NOT NULL,
            user_input TEXT NOT NULL,
            generated_command TEXT,
            executed_command TEXT,
            linked_plan_id TEXT,
            planner_request_id TEXT,
            status TEXT NOT NULL,
            exit_code INTEGER,
            created_at TEXT NOT NULL,
            finished_at TEXT,
            duration_ms INTEGER,
            cwd TEXT,
            planner_source TEXT
        );

        CREATE TABLE IF NOT EXISTS workflows (
            id TEXT PRIMARY KEY,
            label TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'raw',
            original_intent TEXT,
            command TEXT NOT NULL,
            steps_json TEXT,
            project_root TEXT,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS plans (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            user_intent TEXT NOT NULL,
            command TEXT NOT NULL,
            risk TEXT NOT NULL,
            explanation TEXT NOT NULL,
            generated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS memory_items (
            id TEXT PRIMARY KEY,
            scope TEXT NOT NULL,
            project_root TEXT,
            kind TEXT NOT NULL,
            key TEXT NOT NULL,
            value TEXT NOT NULL,
            confidence REAL NOT NULL,
            source TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS memory_suggestions (
            id TEXT PRIMARY KEY,
            scope TEXT NOT NULL,
            project_root TEXT,
            kind TEXT NOT NULL,
            label TEXT NOT NULL,
            proposed_key TEXT NOT NULL,
            proposed_value TEXT NOT NULL,
            confidence REAL NOT NULL,
            derived_from_history_ids_json TEXT NOT NULL DEFAULT '[]',
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT NOT NULL
        );
        ",
    )
    .map_err(|e| format!("Schema init failed: {e}"))?;

    // Old databases lack columns that CREATE TABLE now includes.
    // Duplicate-column is the only error that means "already migrated".
    let migrations = [
        "ALTER TABLE history_items ADD COLUMN finished_at TEXT",
        "ALTER TABLE history_items ADD COLUMN duration_ms INTEGER",
        "ALTER TABLE history_items ADD COLUMN cwd TEXT",
        "ALTER TABLE history_items ADD COLUMN planner_source TEXT",
        "ALTER TABLE workflows ADD COLUMN steps_json TEXT",
    ];
    for sql in migrations {
        exec_migration(conn, sql)?;
    }

    for (table, column) in REQUIRED_COLUMNS {
        require_column(conn, table, column)?;
    }

    Ok(())
}

const REQUIRED_COLUMNS: &[(&str, &str)] = &[
    ("history_items", "finished_at"),
    ("history_items", "duration_ms"),
    ("history_items", "cwd"),
    ("history_items", "planner_source"),
    ("workflows", "steps_json"),
];

fn is_duplicate_column(err: &rusqlite::Error) -> bool {
    match err {
        rusqlite::Error::SqliteFailure(_, Some(msg)) => {
            msg.to_ascii_lowercase().contains("duplicate column")
        }
        _ => false,
    }
}

fn exec_migration(conn: &Connection, sql: &str) -> Result<(), String> {
    match conn.execute(sql, []) {
        Ok(_) => Ok(()),
        Err(e) if is_duplicate_column(&e) => Ok(()),
        Err(e) => Err(format!("Schema migration failed: {e}")),
    }
}

fn require_column(conn: &Connection, table: &str, column: &str) -> Result<(), String> {
    let pragma = format!("PRAGMA table_info({table})");
    let mut stmt = conn
        .prepare(&pragma)
        .map_err(|e| format!("Schema init failed: {e}"))?;
    let mut rows = stmt
        .query([])
        .map_err(|e| format!("Schema init failed: {e}"))?;
    while let Some(row) = rows.next().map_err(|e| format!("Schema init failed: {e}"))? {
        let name: String = row
            .get(1)
            .map_err(|e| format!("Schema init failed: {e}"))?;
        if name == column {
            return Ok(());
        }
    }
    Err(format!("Schema init failed: missing column {table}.{column}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_init_schema_creates_tables() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='history_items'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_migration_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        // Running again should not fail
        init_schema(&conn).unwrap();
    }

    #[test]
    fn fresh_database_creates_steps_json() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name = 'workflows'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(sql.contains("steps_json"), "{sql}");
        require_column(&conn, "workflows", "steps_json").unwrap();
        require_column(&conn, "history_items", "finished_at").unwrap();
    }

    #[test]
    fn old_database_gains_missing_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE history_items (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                source TEXT NOT NULL,
                user_input TEXT NOT NULL,
                generated_command TEXT,
                executed_command TEXT,
                linked_plan_id TEXT,
                planner_request_id TEXT,
                status TEXT NOT NULL,
                exit_code INTEGER,
                created_at TEXT NOT NULL
            );
            CREATE TABLE workflows (
                id TEXT PRIMARY KEY,
                label TEXT NOT NULL,
                source TEXT NOT NULL DEFAULT 'raw',
                original_intent TEXT,
                command TEXT NOT NULL,
                project_root TEXT,
                created_at TEXT NOT NULL
            );
            ",
        )
        .unwrap();
        init_schema(&conn).unwrap();
        require_column(&conn, "workflows", "steps_json").unwrap();
        require_column(&conn, "history_items", "finished_at").unwrap();
        require_column(&conn, "history_items", "duration_ms").unwrap();
        require_column(&conn, "history_items", "cwd").unwrap();
        require_column(&conn, "history_items", "planner_source").unwrap();
    }

    #[test]
    fn migration_returns_non_duplicate_errors() {
        let conn = Connection::open_in_memory().unwrap();
        let err = exec_migration(&conn, "ALTER TABLE missing_table ADD COLUMN steps_json TEXT")
            .unwrap_err();
        assert!(err.contains("Schema migration failed"), "{err}");
        assert!(err.to_lowercase().contains("no such table"), "{err}");
    }

    #[test]
    fn duplicate_column_migration_is_ok() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE sample (id TEXT);")
            .unwrap();
        exec_migration(&conn, "ALTER TABLE sample ADD COLUMN extra TEXT").unwrap();
        exec_migration(&conn, "ALTER TABLE sample ADD COLUMN extra TEXT").unwrap();
    }

    #[test]
    fn missing_required_column_fails_check() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE workflows (id TEXT);")
            .unwrap();
        let err = require_column(&conn, "workflows", "steps_json").unwrap_err();
        assert!(err.contains("workflows.steps_json"), "{err}");
    }

    #[test]
    fn init_fails_when_a_table_name_is_already_a_view() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE VIEW history_items AS SELECT 1 AS id;")
            .unwrap();
        let err = init_schema(&conn).unwrap_err();
        assert!(
            err.contains("Schema init failed") || err.contains("Schema migration failed"),
            "{err}"
        );
    }

    #[test]
    fn missing_table_fails_the_column_check() {
        let conn = Connection::open_in_memory().unwrap();
        let err = require_column(&conn, "missing_table", "steps_json").unwrap_err();
        assert!(err.contains("missing_table.steps_json"), "{err}");
    }
}
