use crate::shell::pty_manager::{write_command, write_raw};
use crate::state::AppState;
use crate::types::errors::ApiError;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalExecuteRequest {
    pub execution_id: String,
    pub session_id: String,
    pub command: String,
    pub source: String,
    pub linked_plan_id: Option<String>,
    pub cwd: Option<String>,
    pub env: Option<std::collections::HashMap<String, String>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionSummary {
    pub id: String,
    pub session_id: String,
    pub command: String,
    pub source: String,
    pub status: String,
    pub started_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalExecuteResponse {
    pub execution: ExecutionSummary,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionStartedEvent {
    pub execution: ExecutionSummary,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalResizeRequest {
    pub session_id: String,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalResizeResponse {
    pub ok: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalWriteRequest {
    pub session_id: String,
    pub data: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalWriteResponse {
    pub ok: bool,
}

#[tauri::command]
pub fn terminal_execute(
    request: TerminalExecuteRequest,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<TerminalExecuteResponse, ApiError> {
    if request.command.is_empty() {
        return Err(ApiError::validation("command cannot be empty"));
    }

    let registry = state
        .sessions
        .lock()
        .map_err(|e| ApiError::execution(e.to_string()))?;

    let record = registry
        .get(&request.session_id)
        .ok_or_else(|| ApiError::session_not_found(&request.session_id))?;

    write_command(&record.writer, &request.command).map_err(ApiError::execution)?;

    let now = chrono::Utc::now().to_rfc3339();
    let summary = ExecutionSummary {
        id: request.execution_id.clone(),
        session_id: request.session_id.clone(),
        command: request.command,
        source: request.source,
        status: "running".to_string(),
        started_at: now,
    };

    let _ = app.emit(
        "terminal:execution_started",
        ExecutionStartedEvent {
            execution: summary.clone(),
        },
    );

    drop(registry);

    // Set pending execution for prompt-marker based completion
    let mut registry = state
        .sessions
        .lock()
        .map_err(|e| ApiError::execution(e.to_string()))?;
    let _ = registry.set_pending_execution(
        &request.session_id,
        Some(request.execution_id),
    );

    Ok(TerminalExecuteResponse { execution: summary })
}

#[tauri::command]
pub fn terminal_resize(
    request: TerminalResizeRequest,
    state: State<'_, AppState>,
) -> Result<TerminalResizeResponse, ApiError> {
    let mut registry = state
        .sessions
        .lock()
        .map_err(|e| ApiError::execution(e.to_string()))?;

    registry
        .resize(&request.session_id, request.cols, request.rows)
        .map_err(ApiError::execution)?;

    Ok(TerminalResizeResponse { ok: true })
}

#[tauri::command]
pub fn terminal_write(
    request: TerminalWriteRequest,
    state: State<'_, AppState>,
) -> Result<TerminalWriteResponse, ApiError> {
    let registry = state
        .sessions
        .lock()
        .map_err(|e| ApiError::execution(e.to_string()))?;

    let record = registry
        .get(&request.session_id)
        .ok_or_else(|| ApiError::session_not_found(&request.session_id))?;

    write_raw(&record.writer, &request.data).map_err(ApiError::execution)?;

    Ok(TerminalWriteResponse { ok: true })
}
