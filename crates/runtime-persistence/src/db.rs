use rusqlite::Connection;
use std::path::Path;

pub fn open_database(path: &Path) -> Result<Connection, String> {
    Connection::open(path).map_err(|e| format!("Failed to open database: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_a_file_and_refuses_a_directory() {
        let path = std::env::temp_dir().join(format!(
            "commandui-db-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let conn = open_database(&path).unwrap();
        conn.execute_batch("CREATE TABLE sample (id INTEGER);")
            .unwrap();
        drop(conn);
        let _ = std::fs::remove_file(&path);

        let err = open_database(std::env::temp_dir().as_path()).unwrap_err();
        assert!(err.contains("Failed to open database"), "{err}");
    }
}
