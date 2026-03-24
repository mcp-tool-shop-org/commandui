use crate::state::AppState;
use crate::types::errors::ApiError;
use commandui_runtime_persistence::db::open_database;
use commandui_runtime_persistence::memory::{self, MemoryItem, MemorySuggestion};
use serde::{Deserialize, Serialize};
use tauri::State;

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
pub struct MemoryStoreSuggestionRequest {
    pub suggestion: MemorySuggestion,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStoreSuggestionResponse {
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
    let items = memory::list_items(&conn).map_err(ApiError::database)?;
    let suggestions = memory::list_pending_suggestions(&conn).map_err(ApiError::database)?;
    Ok(MemoryListResponse { items, suggestions })
}

#[tauri::command]
pub fn memory_add(
    request: MemoryAddRequest,
    state: State<'_, AppState>,
) -> Result<MemoryAddResponse, ApiError> {
    let conn = get_conn(&state)?;
    memory::add_item(&conn, &request.item).map_err(ApiError::database)?;
    Ok(MemoryAddResponse { ok: true })
}

#[tauri::command]
pub fn memory_accept_suggestion(
    request: MemoryAcceptSuggestionRequest,
    state: State<'_, AppState>,
) -> Result<MemoryAcceptSuggestionResponse, ApiError> {
    let conn = get_conn(&state)?;
    let created = memory::accept_suggestion(&conn, &request.suggestion_id)
        .map_err(ApiError::database)?;
    Ok(MemoryAcceptSuggestionResponse {
        ok: true,
        created_item: Some(created),
    })
}

#[tauri::command]
pub fn memory_dismiss_suggestion(
    request: MemoryDismissSuggestionRequest,
    state: State<'_, AppState>,
) -> Result<MemoryDismissSuggestionResponse, ApiError> {
    let conn = get_conn(&state)?;
    memory::dismiss_suggestion(&conn, &request.suggestion_id).map_err(ApiError::database)?;
    Ok(MemoryDismissSuggestionResponse { ok: true })
}

#[tauri::command]
pub fn memory_delete(
    request: MemoryDeleteRequest,
    state: State<'_, AppState>,
) -> Result<MemoryDeleteResponse, ApiError> {
    let conn = get_conn(&state)?;
    memory::delete_item(&conn, &request.memory_id).map_err(ApiError::database)?;
    Ok(MemoryDeleteResponse { ok: true })
}

#[tauri::command]
pub fn memory_store_suggestion(
    request: MemoryStoreSuggestionRequest,
    state: State<'_, AppState>,
) -> Result<MemoryStoreSuggestionResponse, ApiError> {
    let conn = get_conn(&state)?;
    memory::store_suggestion(&conn, &request.suggestion).map_err(ApiError::database)?;
    Ok(MemoryStoreSuggestionResponse { ok: true })
}
