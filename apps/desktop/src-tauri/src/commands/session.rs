use crate::state::AppState;
use crate::types::errors::ApiError;
use commandui_runtime_core::services::session_service::{CreateSessionRequest, SessionSummary};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreateRequest {
    pub label: Option<String>,
    pub cwd: Option<String>,
    pub shell: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummaryPayload {
    pub id: String,
    pub label: String,
    pub cwd: String,
    pub shell: String,
    pub status: String,
    pub created_at: String,
    pub last_active_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreateResponse {
    pub session: SessionSummaryPayload,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListResponse {
    pub sessions: Vec<SessionSummaryPayload>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCloseRequest {
    pub session_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCloseResponse {
    pub ok: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCwdUpdateRequest {
    pub session_id: String,
    pub cwd: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCwdUpdateResponse {
    pub ok: bool,
}

fn to_payload(s: SessionSummary) -> SessionSummaryPayload {
    SessionSummaryPayload {
        id: s.id,
        label: s.label,
        cwd: s.cwd,
        shell: s.shell,
        status: s.status,
        created_at: s.created_at,
        last_active_at: s.last_active_at,
    }
}

#[tauri::command]
pub fn session_create(
    request: SessionCreateRequest,
    state: State<'_, AppState>,
) -> Result<SessionCreateResponse, ApiError> {
    let summary = state
        .session_service
        .create(CreateSessionRequest {
            label: request.label,
            cwd: request.cwd,
            shell: request.shell,
        })
        .map_err(ApiError::execution)?;

    Ok(SessionCreateResponse {
        session: to_payload(summary),
    })
}

#[tauri::command]
pub fn session_list(state: State<'_, AppState>) -> Result<SessionListResponse, ApiError> {
    let summaries = state
        .session_service
        .list()
        .map_err(ApiError::execution)?;

    Ok(SessionListResponse {
        sessions: summaries.into_iter().map(to_payload).collect(),
    })
}

#[tauri::command]
pub fn session_close(
    request: SessionCloseRequest,
    state: State<'_, AppState>,
) -> Result<SessionCloseResponse, ApiError> {
    state
        .session_service
        .close(&request.session_id)
        .map_err(ApiError::execution)?;

    Ok(SessionCloseResponse { ok: true })
}

#[tauri::command]
pub fn session_update_cwd(
    request: SessionCwdUpdateRequest,
    state: State<'_, AppState>,
) -> Result<SessionCwdUpdateResponse, ApiError> {
    state
        .session_service
        .update_cwd(&request.session_id, &request.cwd)
        .map_err(|e| ApiError::validation(&e))?;

    Ok(SessionCwdUpdateResponse { ok: true })
}
