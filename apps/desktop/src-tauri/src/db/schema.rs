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
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS workflows (
            id TEXT PRIMARY KEY,
            label TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'raw',
            original_intent TEXT,
            command TEXT NOT NULL,
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

    Ok(())
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
}
