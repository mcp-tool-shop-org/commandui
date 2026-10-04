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

// These commands are async so Tauri runs them on its async runtime instead of the main
// thread: a PTY write that blocks on a slow-draining shell must not freeze the webview.

/// Largest piece of a paste handed to the PTY in one write.
const WRITE_CHUNK_BYTES: usize = 4096;

/// Split at char boundaries so no chunk cuts a UTF-8 sequence.
fn write_chunks(data: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut rest = data;
    while !rest.is_empty() {
        let mut end = rest.len().min(WRITE_CHUNK_BYTES);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        let (head, tail) = rest.split_at(end);
        chunks.push(head);
        rest = tail;
    }
    chunks
}

#[tauri::command]
pub async fn terminal_execute(
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
        .map_err(ApiError::from_execution)?;

    Ok(TerminalExecuteResponse { execution: summary })
}

#[tauri::command]
pub async fn terminal_interrupt(
    request: TerminalInterruptRequest,
    state: State<'_, AppState>,
) -> Result<TerminalInterruptResponse, ApiError> {
    state
        .terminal_service
        .interrupt(&request.session_id)
        .map_err(ApiError::from_execution)?;

    Ok(TerminalInterruptResponse { ok: true })
}

#[tauri::command]
pub async fn terminal_resync(
    request: TerminalResyncRequest,
    state: State<'_, AppState>,
) -> Result<TerminalResyncResponse, ApiError> {
    state
        .terminal_service
        .resync(&request.session_id)
        .map_err(ApiError::from_execution)?;

    Ok(TerminalResyncResponse { ok: true })
}

#[tauri::command]
pub async fn terminal_resize(
    request: TerminalResizeRequest,
    state: State<'_, AppState>,
) -> Result<TerminalResizeResponse, ApiError> {
    state
        .terminal_service
        .resize(&request.session_id, request.cols, request.rows)
        .map_err(ApiError::from_execution)?;

    Ok(TerminalResizeResponse { ok: true })
}

#[tauri::command]
pub async fn terminal_write(
    request: TerminalWriteRequest,
    state: State<'_, AppState>,
) -> Result<TerminalWriteResponse, ApiError> {
    for chunk in write_chunks(&request.data) {
        state
            .terminal_service
            .write(&request.session_id, chunk)
            .map_err(ApiError::from_execution)?;
    }

    Ok(TerminalWriteResponse { ok: true })
}

#[cfg(test)]
mod chunk_tests {
    use super::*;

    #[test]
    fn chunks_rejoin_and_respect_char_boundaries() {
        let data = "é".repeat(WRITE_CHUNK_BYTES);
        let chunks = write_chunks(&data);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| c.len() <= WRITE_CHUNK_BYTES));
        assert_eq!(chunks.concat(), data);
        assert!(write_chunks("").is_empty());
    }
}
