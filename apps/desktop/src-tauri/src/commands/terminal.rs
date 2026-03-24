use crate::state::AppState;
use crate::types::errors::ApiError;
use commandui_runtime_core::events::ExecutionSummary;
use commandui_runtime_core::services::terminal_service::ExecuteRequest;
use serde::{Deserialize, Serialize};
use tauri::State;

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalExecuteResponse {
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalInterruptRequest {
    pub session_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalInterruptResponse {
    pub ok: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalResyncRequest {
    pub session_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalResyncResponse {
    pub ok: bool,
}

#[tauri::command]
pub fn terminal_execute(
    request: TerminalExecuteRequest,
    state: State<'_, AppState>,
) -> Result<TerminalExecuteResponse, ApiError> {
    let summary = state
        .terminal_service
        .execute(ExecuteRequest {
            execution_id: request.execution_id,
            session_id: request.session_id,
            command: request.command,
            source: request.source,
            linked_plan_id: request.linked_plan_id,
        })
        .map_err(ApiError::execution)?;

    Ok(TerminalExecuteResponse { execution: summary })
}

#[tauri::command]
pub fn terminal_interrupt(
    request: TerminalInterruptRequest,
    state: State<'_, AppState>,
) -> Result<TerminalInterruptResponse, ApiError> {
    state
        .terminal_service
        .interrupt(&request.session_id)
        .map_err(ApiError::execution)?;

    Ok(TerminalInterruptResponse { ok: true })
}

#[tauri::command]
pub fn terminal_resync(
    request: TerminalResyncRequest,
    state: State<'_, AppState>,
) -> Result<TerminalResyncResponse, ApiError> {
    state
        .terminal_service
        .resync(&request.session_id)
        .map_err(ApiError::execution)?;

    Ok(TerminalResyncResponse { ok: true })
}

#[tauri::command]
pub fn terminal_resize(
    request: TerminalResizeRequest,
    state: State<'_, AppState>,
) -> Result<TerminalResizeResponse, ApiError> {
    state
        .terminal_service
        .resize(&request.session_id, request.cols, request.rows)
        .map_err(ApiError::execution)?;

    Ok(TerminalResizeResponse { ok: true })
}

#[tauri::command]
pub fn terminal_write(
    request: TerminalWriteRequest,
    state: State<'_, AppState>,
) -> Result<TerminalWriteResponse, ApiError> {
    state
        .terminal_service
        .write(&request.session_id, &request.data)
        .map_err(ApiError::execution)?;

    Ok(TerminalWriteResponse { ok: true })
}
