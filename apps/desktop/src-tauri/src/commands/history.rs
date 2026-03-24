use crate::state::AppState;
use crate::types::errors::ApiError;
use commandui_runtime_persistence::db::open_database;
use commandui_runtime_persistence::history::{self, HistoryItem, PlanRow};
use serde::{Deserialize, Serialize};
use tauri::State;

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
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
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
    history::append(&conn, &request.item).map_err(ApiError::database)?;
    Ok(HistoryAppendResponse { ok: true })
}

#[tauri::command]
pub fn history_list(
    request: HistoryListRequest,
    state: State<'_, AppState>,
) -> Result<HistoryListResponse, ApiError> {
    let conn = get_conn(&state)?;
    let limit = request.limit.unwrap_or(100);
    let items = history::list(&conn, request.session_id.as_deref(), limit)
        .map_err(ApiError::database)?;
    Ok(HistoryListResponse { items })
}

#[tauri::command]
pub fn history_update(
    request: HistoryUpdateRequest,
    state: State<'_, AppState>,
) -> Result<HistoryUpdateResponse, ApiError> {
    let conn = get_conn(&state)?;
    history::update(
        &conn,
        &request.history_id,
        request.status.as_deref(),
        request.exit_code,
        request.executed_command.as_deref(),
        request.finished_at.as_deref(),
        request.duration_ms,
    )
    .map_err(ApiError::database)?;
    Ok(HistoryUpdateResponse { ok: true })
}

#[tauri::command]
pub fn plan_store(
    request: PlanStoreRequest,
    state: State<'_, AppState>,
) -> Result<PlanStoreResponse, ApiError> {
    let conn = get_conn(&state)?;
    history::store_plan(&conn, &request.plan).map_err(ApiError::database)?;
    Ok(PlanStoreResponse { ok: true })
}
