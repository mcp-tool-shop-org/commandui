use rusqlite::Connection;
use std::path::Path;

pub fn open_database(path: &Path) -> Result<Connection, String> {
    Connection::open(path).map_err(|e| format!("Failed to open database: {e}"))
}
