use crate::db::sqlite::open_database;
use crate::state::AppState;
use crate::types::errors::ApiError;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    pub id: String,
    pub label: String,
    pub source: String,
    pub original_intent: Option<String>,
    pub command: String,
    pub project_root: Option<String>,
    pub created_at: String,
}

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
    let wf = &request.workflow;

    conn.execute(
        "INSERT OR REPLACE INTO workflows (id, label, source, original_intent, command, project_root, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![wf.id, wf.label, wf.source, wf.original_intent, wf.command, wf.project_root, wf.created_at],
    ).map_err(|e| ApiError::database(e.to_string()))?;

    Ok(WorkflowAddResponse { ok: true })
}

#[tauri::command]
pub fn workflow_list(state: State<'_, AppState>) -> Result<WorkflowListResponse, ApiError> {
    let conn = get_conn(&state)?;

    let mut stmt = conn
        .prepare("SELECT id, label, source, original_intent, command, project_root, created_at FROM workflows ORDER BY created_at DESC")
        .map_err(|e| ApiError::database(e.to_string()))?;

    let workflows = stmt
        .query_map([], |row| {
            Ok(Workflow {
                id: row.get(0)?,
                label: row.get(1)?,
                source: row.get(2)?,
                original_intent: row.get(3)?,
                command: row.get(4)?,
                project_root: row.get(5)?,
                created_at: row.get(6)?,
            })
        })
        .map_err(|e| ApiError::database(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(WorkflowListResponse { workflows })
}
