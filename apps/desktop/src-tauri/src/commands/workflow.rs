use crate::state::AppState;
use crate::types::errors::ApiError;
use commandui_runtime_persistence::db::open_database;
use commandui_runtime_persistence::workflow::{self, Workflow};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAddRequest {
    pub workflow: Workflow,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAddResponse {
    pub ok: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowListResponse {
    pub workflows: Vec<Workflow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDeleteRequest {
    pub id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDeleteResponse {
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
pub fn workflow_add(
    request: WorkflowAddRequest,
    state: State<'_, AppState>,
) -> Result<WorkflowAddResponse, ApiError> {
    let conn = get_conn(&state)?;
    workflow::add(&conn, &request.workflow).map_err(ApiError::database)?;
    Ok(WorkflowAddResponse { ok: true })
}

#[tauri::command]
pub fn workflow_list(state: State<'_, AppState>) -> Result<WorkflowListResponse, ApiError> {
    let conn = get_conn(&state)?;
    let workflows = workflow::list(&conn).map_err(ApiError::database)?;
    Ok(WorkflowListResponse { workflows })
}

#[tauri::command]
pub fn workflow_delete(
    request: WorkflowDeleteRequest,
    state: State<'_, AppState>,
) -> Result<WorkflowDeleteResponse, ApiError> {
    let conn = get_conn(&state)?;
    workflow::delete(&conn, &request.id).map_err(ApiError::database)?;
    Ok(WorkflowDeleteResponse { ok: true })
}
