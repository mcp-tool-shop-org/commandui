use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItem {
    pub id: String,
    pub scope: String,
    pub project_root: Option<String>,
    pub kind: String,
    pub key: String,
    pub value: String,
    pub confidence: f64,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MemorySuggestion {
    pub id: String,
    pub scope: String,
    pub project_root: Option<String>,
    pub kind: String,
    pub label: String,
    pub proposed_key: String,
    pub proposed_value: String,
    pub confidence: f64,
    pub derived_from_history_ids: Vec<String>,
    pub status: String,
    pub created_at: String,
}

pub fn list_items(conn: &Connection) -> Result<Vec<MemoryItem>, String> {
    let mut stmt = conn
        .prepare("SELECT id, scope, project_root, kind, key, value, confidence, source, created_at, updated_at FROM memory_items ORDER BY updated_at DESC")
        .map_err(|e| format!("memory list: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(MemoryItem {
                id: row.get(0)?,
                scope: row.get(1)?,
                project_root: row.get(2)?,
                kind: row.get(3)?,
                key: row.get(4)?,
                value: row.get(5)?,
                confidence: row.get(6)?,
                source: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(|e| format!("memory list: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn list_pending_suggestions(conn: &Connection) -> Result<Vec<MemorySuggestion>, String> {
    let mut stmt = conn
        .prepare("SELECT id, scope, project_root, kind, label, proposed_key, proposed_value, confidence, derived_from_history_ids_json, status, created_at FROM memory_suggestions WHERE status = 'pending' ORDER BY created_at DESC")
        .map_err(|e| format!("memory suggestions: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            let ids_json: String = row.get(8)?;
            let derived: Vec<String> =
                serde_json::from_str(&ids_json).unwrap_or_default();
            Ok(MemorySuggestion {
                id: row.get(0)?,
                scope: row.get(1)?,
                project_root: row.get(2)?,
                kind: row.get(3)?,
                label: row.get(4)?,
                proposed_key: row.get(5)?,
                proposed_value: row.get(6)?,
                confidence: row.get(7)?,
                derived_from_history_ids: derived,
                status: row.get(9)?,
                created_at: row.get(10)?,
            })
        })
        .map_err(|e| format!("memory suggestions: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn add_item(conn: &Connection, item: &MemoryItem) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO memory_items (id, scope, project_root, kind, key, value, confidence, source, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![item.id, item.scope, item.project_root, item.kind, item.key, item.value, item.confidence, item.source, item.created_at, item.updated_at],
    ).map_err(|e| format!("memory add: {e}"))?;
    Ok(())
}

pub fn accept_suggestion(conn: &Connection, suggestion_id: &str) -> Result<MemoryItem, String> {
    // Read suggestion
    let suggestion: MemorySuggestion = conn
        .query_row(
            "SELECT id, scope, project_root, kind, label, proposed_key, proposed_value, confidence, derived_from_history_ids_json, status, created_at FROM memory_suggestions WHERE id = ?1",
            rusqlite::params![suggestion_id],
            |row| {
                let ids_json: String = row.get(8)?;
                let derived: Vec<String> = serde_json::from_str(&ids_json).unwrap_or_default();
                Ok(MemorySuggestion {
                    id: row.get(0)?,
                    scope: row.get(1)?,
                    project_root: row.get(2)?,
                    kind: row.get(3)?,
                    label: row.get(4)?,
                    proposed_key: row.get(5)?,
                    proposed_value: row.get(6)?,
                    confidence: row.get(7)?,
                    derived_from_history_ids: derived,
                    status: row.get(9)?,
                    created_at: row.get(10)?,
                })
            },
        )
        .map_err(|e| format!("suggestion not found: {e}"))?;

    let now = chrono::Utc::now().to_rfc3339();
    let mem_id = uuid::Uuid::new_v4().to_string();

    let created_item = MemoryItem {
        id: mem_id,
        scope: suggestion.scope,
        project_root: suggestion.project_root,
        kind: suggestion.kind,
        key: suggestion.proposed_key,
        value: suggestion.proposed_value,
        confidence: suggestion.confidence,
        source: "accepted".to_string(),
        created_at: now.clone(),
        updated_at: now,
    };

    // Insert memory item
    add_item(conn, &created_item)?;

    // Update suggestion status
    conn.execute(
        "UPDATE memory_suggestions SET status = 'accepted' WHERE id = ?1",
        rusqlite::params![suggestion_id],
    )
    .map_err(|e| format!("accept suggestion: {e}"))?;

    Ok(created_item)
}

pub fn dismiss_suggestion(conn: &Connection, suggestion_id: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE memory_suggestions SET status = 'dismissed' WHERE id = ?1",
        rusqlite::params![suggestion_id],
    )
    .map_err(|e| format!("dismiss suggestion: {e}"))?;
    Ok(())
}

pub fn delete_item(conn: &Connection, memory_id: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM memory_items WHERE id = ?1",
        rusqlite::params![memory_id],
    )
    .map_err(|e| format!("memory delete: {e}"))?;
    Ok(())
}

pub fn store_suggestion(conn: &Connection, suggestion: &MemorySuggestion) -> Result<(), String> {
    let ids_json =
        serde_json::to_string(&suggestion.derived_from_history_ids).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT OR IGNORE INTO memory_suggestions (id, scope, project_root, kind, label, proposed_key, proposed_value, confidence, derived_from_history_ids_json, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            suggestion.id, suggestion.scope, suggestion.project_root, suggestion.kind, suggestion.label,
            suggestion.proposed_key, suggestion.proposed_value, suggestion.confidence,
            ids_json, suggestion.status, suggestion.created_at,
        ],
    ).map_err(|e| format!("store suggestion: {e}"))?;
    Ok(())
}
