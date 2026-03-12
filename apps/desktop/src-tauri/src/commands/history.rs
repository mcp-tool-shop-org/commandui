use crate::db::sqlite::open_database;
use crate::state::AppState;
use crate::types::errors::ApiError;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub session_id: String,
    pub source: String,
    pub user_input: String,
    pub generated_command: Option<String>,
    pub executed_command: Option<String>,
    pub linked_plan_id: Option<String>,
    pub planner_request_id: Option<String>,
    pub status: String,
    pub exit_code: Option<i32>,
    pub created_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryAppendRequest {
    pub item: HistoryItem,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryAppendResponse {
    pub ok: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryListRequest {
    pub session_id: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryListResponse {
    pub items: Vec<HistoryItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryUpdateRequest {
    pub history_id: String,
    pub status: Option<String>,
    pub exit_code: Option<i32>,
    pub executed_command: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryUpdateResponse {
    pub ok: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStoreRequest {
    pub plan: PlanRow,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRow {
    pub id: String,
    pub session_id: String,
    pub user_intent: String,
    pub command: String,
    pub risk: String,
    pub explanation: String,
    pub generated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStoreResponse {
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
pub fn history_append(
    request: HistoryAppendRequest,
    state: State<'_, AppState>,
) -> Result<HistoryAppendResponse, ApiError> {
    let conn = get_conn(&state)?;
    let item = &request.item;

    conn.execute(
        "INSERT OR REPLACE INTO history_items (id, session_id, source, user_input, generated_command, executed_command, linked_plan_id, planner_request_id, status, exit_code, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            item.id, item.session_id, item.source, item.user_input,
            item.generated_command, item.executed_command, item.linked_plan_id,
            item.planner_request_id, item.status, item.exit_code, item.created_at,
        ],
    ).map_err(|e| ApiError::database(e.to_string()))?;

    Ok(HistoryAppendResponse { ok: true })
}

#[tauri::command]
pub fn history_list(
    request: HistoryListRequest,
    state: State<'_, AppState>,
) -> Result<HistoryListResponse, ApiError> {
    let conn = get_conn(&state)?;
    let limit = request.limit.unwrap_or(100);

    let items = if let Some(ref sid) = request.session_id {
        let mut stmt = conn
            .prepare(
                "SELECT id, session_id, source, user_input, generated_command, executed_command, linked_plan_id, planner_request_id, status, exit_code, created_at FROM history_items WHERE session_id = ?1 ORDER BY created_at DESC LIMIT ?2",
            )
            .map_err(|e| ApiError::database(e.to_string()))?;

        stmt.query_map(rusqlite::params![sid, limit], map_history_row)
            .map_err(|e| ApiError::database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, session_id, source, user_input, generated_command, executed_command, linked_plan_id, planner_request_id, status, exit_code, created_at FROM history_items ORDER BY created_at DESC LIMIT ?1",
            )
            .map_err(|e| ApiError::database(e.to_string()))?;

        stmt.query_map(rusqlite::params![limit], map_history_row)
            .map_err(|e| ApiError::database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect()
    };

    Ok(HistoryListResponse { items })
}

fn map_history_row(row: &rusqlite::Row) -> rusqlite::Result<HistoryItem> {
    Ok(HistoryItem {
        id: row.get(0)?,
        session_id: row.get(1)?,
        source: row.get(2)?,
        user_input: row.get(3)?,
        generated_command: row.get(4)?,
        executed_command: row.get(5)?,
        linked_plan_id: row.get(6)?,
        planner_request_id: row.get(7)?,
        status: row.get(8)?,
        exit_code: row.get(9)?,
        created_at: row.get(10)?,
    })
}

#[tauri::command]
pub fn history_update(
    request: HistoryUpdateRequest,
    state: State<'_, AppState>,
) -> Result<HistoryUpdateResponse, ApiError> {
    let conn = get_conn(&state)?;

    conn.execute(
        "UPDATE history_items SET status = COALESCE(?1, status), exit_code = COALESCE(?2, exit_code), executed_command = COALESCE(?3, executed_command) WHERE id = ?4",
        rusqlite::params![
            request.status,
            request.exit_code,
            request.executed_command,
            request.history_id,
        ],
    )
    .map_err(|e| ApiError::database(e.to_string()))?;

    Ok(HistoryUpdateResponse { ok: true })
}

#[tauri::command]
pub fn plan_store(
    request: PlanStoreRequest,
    state: State<'_, AppState>,
) -> Result<PlanStoreResponse, ApiError> {
    let conn = get_conn(&state)?;
    let p = &request.plan;

    conn.execute(
        "INSERT OR REPLACE INTO plans (id, session_id, user_intent, command, risk, explanation, generated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![p.id, p.session_id, p.user_intent, p.command, p.risk, p.explanation, p.generated_at],
    ).map_err(|e| ApiError::database(e.to_string()))?;

    Ok(PlanStoreResponse { ok: true })
}
