use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    pub id: String,
    pub label: String,
    pub source: String,
    pub original_intent: Option<String>,
    pub command: String,
    pub steps_json: Option<String>,
    pub project_root: Option<String>,
    pub created_at: String,
}

pub fn add(conn: &Connection, wf: &Workflow) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO workflows (id, label, source, original_intent, command, steps_json, project_root, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![wf.id, wf.label, wf.source, wf.original_intent, wf.command, wf.steps_json, wf.project_root, wf.created_at],
    ).map_err(|e| format!("workflow add: {e}"))?;
    Ok(())
}

pub fn list(conn: &Connection) -> Result<Vec<Workflow>, String> {
    let mut stmt = conn
        .prepare("SELECT id, label, source, original_intent, command, steps_json, project_root, created_at FROM workflows ORDER BY created_at DESC")
        .map_err(|e| format!("workflow list: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Workflow {
                id: row.get(0)?,
                label: row.get(1)?,
                source: row.get(2)?,
                original_intent: row.get(3)?,
                command: row.get(4)?,
                steps_json: row.get(5)?,
                project_root: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| format!("workflow list: {e}"))?;
    let workflows = rows.filter_map(|r| r.ok()).collect();

    Ok(workflows)
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM workflows WHERE id = ?1",
        rusqlite::params![id],
    )
    .map_err(|e| format!("workflow delete: {e}"))?;
    Ok(())
}
