use crate::db::sqlite::open_database;
use crate::state::AppState;
use crate::types::errors::ApiError;
use serde::{Deserialize, Serialize};
use tauri::State;

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryListResponse {
    pub items: Vec<MemoryItem>,
    pub suggestions: Vec<MemorySuggestion>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryAddRequest {
    pub item: MemoryItem,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryAddResponse {
    pub ok: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryAcceptSuggestionRequest {
    pub suggestion_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryAcceptSuggestionResponse {
    pub ok: bool,
    pub created_item: Option<MemoryItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDismissSuggestionRequest {
    pub suggestion_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDismissSuggestionResponse {
    pub ok: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDeleteRequest {
    pub memory_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDeleteResponse {
    pub ok: bool,
}

fn get_conn(state: &State<'_, AppState>) -> Result<rusqlite::Connection, ApiError> {
    let path_guard = state.db_path.lock().map_err(|e| ApiError::database(e.to_string()))?;
    let path = path_guard
        .as_ref()
        .ok_or_else(|| ApiError::database("Database not initialized"))?;
    open_database(path).map_err(ApiError::database)
}

#[tauri::command]
pub fn memory_list(state: State<'_, AppState>) -> Result<MemoryListResponse, ApiError> {
    let conn = get_conn(&state)?;

    let mut stmt = conn
        .prepare("SELECT id, scope, project_root, kind, key, value, confidence, source, created_at, updated_at FROM memory_items ORDER BY updated_at DESC")
        .map_err(|e| ApiError::database(e.to_string()))?;

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
        .map_err(|e| ApiError::database(e.to_string()))?;
    let items: Vec<MemoryItem> = rows.filter_map(|r| r.ok()).collect();

    let mut stmt2 = conn
        .prepare("SELECT id, scope, project_root, kind, label, proposed_key, proposed_value, confidence, derived_from_history_ids_json, status, created_at FROM memory_suggestions WHERE status = 'pending' ORDER BY created_at DESC")
        .map_err(|e| ApiError::database(e.to_string()))?;

    let rows2 = stmt2
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
        .map_err(|e| ApiError::database(e.to_string()))?;
    let suggestions: Vec<MemorySuggestion> = rows2.filter_map(|r| r.ok()).collect();

    Ok(MemoryListResponse { items, suggestions })
}

#[tauri::command]
pub fn memory_add(
    request: MemoryAddRequest,
    state: State<'_, AppState>,
) -> Result<MemoryAddResponse, ApiError> {
    let conn = get_conn(&state)?;
    let m = &request.item;

    conn.execute(
        "INSERT OR REPLACE INTO memory_items (id, scope, project_root, kind, key, value, confidence, source, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![m.id, m.scope, m.project_root, m.kind, m.key, m.value, m.confidence, m.source, m.created_at, m.updated_at],
    ).map_err(|e| ApiError::database(e.to_string()))?;

    Ok(MemoryAddResponse { ok: true })
}

#[tauri::command]
pub fn memory_accept_suggestion(
    request: MemoryAcceptSuggestionRequest,
    state: State<'_, AppState>,
) -> Result<MemoryAcceptSuggestionResponse, ApiError> {
    let conn = get_conn(&state)?;

    // Read suggestion
    let suggestion: MemorySuggestion = conn
        .query_row(
            "SELECT id, scope, project_root, kind, label, proposed_key, proposed_value, confidence, derived_from_history_ids_json, status, created_at FROM memory_suggestions WHERE id = ?1",
            rusqlite::params![request.suggestion_id],
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
        .map_err(|e| ApiError::database(format!("Suggestion not found: {e}")))?;

    let now = chrono::Utc::now().to_rfc3339();
    let mem_id = uuid::Uuid::new_v4().to_string();

    let created_item = MemoryItem {
        id: mem_id.clone(),
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
    conn.execute(
        "INSERT OR REPLACE INTO memory_items (id, scope, project_root, kind, key, value, confidence, source, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            created_item.id, created_item.scope, created_item.project_root,
            created_item.kind, created_item.key, created_item.value,
            created_item.confidence, created_item.source,
            created_item.created_at, created_item.updated_at,
        ],
    ).map_err(|e| ApiError::database(e.to_string()))?;

    // Update suggestion status
    conn.execute(
        "UPDATE memory_suggestions SET status = 'accepted' WHERE id = ?1",
        rusqlite::params![request.suggestion_id],
    )
    .map_err(|e| ApiError::database(e.to_string()))?;

    Ok(MemoryAcceptSuggestionResponse {
        ok: true,
        created_item: Some(created_item),
    })
}

#[tauri::command]
pub fn memory_dismiss_suggestion(
    request: MemoryDismissSuggestionRequest,
    state: State<'_, AppState>,
) -> Result<MemoryDismissSuggestionResponse, ApiError> {
    let conn = get_conn(&state)?;

    conn.execute(
        "UPDATE memory_suggestions SET status = 'dismissed' WHERE id = ?1",
        rusqlite::params![request.suggestion_id],
    )
    .map_err(|e| ApiError::database(e.to_string()))?;

    Ok(MemoryDismissSuggestionResponse { ok: true })
}

#[tauri::command]
pub fn memory_delete(
    request: MemoryDeleteRequest,
    state: State<'_, AppState>,
) -> Result<MemoryDeleteResponse, ApiError> {
    let conn = get_conn(&state)?;

    conn.execute(
        "DELETE FROM memory_items WHERE id = ?1",
        rusqlite::params![request.memory_id],
    )
    .map_err(|e| ApiError::database(e.to_string()))?;

    Ok(MemoryDeleteResponse { ok: true })
}
