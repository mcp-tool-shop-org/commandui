use crate::shell::pty_manager::{write_command, write_raw};
use crate::shell::session_registry::SessionExecState;
use crate::state::AppState;
use crate::types::errors::ApiError;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use super::session::SessionExecStateChangedEvent;

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
    pub linked_plan_id: Option<String>,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub exit_code: Option<i32>,
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

// --- Helper ---

fn emit_exec_state(app: &AppHandle, session_id: &str, state: &SessionExecState) {
    let _ = app.emit(
        "session:exec_state_changed",
        SessionExecStateChangedEvent {
            session_id: session_id.to_string(),
            exec_state: state.to_string(),
            changed_at: chrono::Utc::now().to_rfc3339(),
        },
    );
}

// --- Commands ---

#[tauri::command]
pub fn terminal_execute(
    request: TerminalExecuteRequest,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<TerminalExecuteResponse, ApiError> {
    if request.command.is_empty() {
        return Err(ApiError::validation("command cannot be empty"));
    }

    // Double-submit guard: check exec state before proceeding
    {
        let registry = state
            .sessions
            .lock()
            .map_err(|e| ApiError::execution(e.to_string()))?;

        let record = registry
            .get(&request.session_id)
            .ok_or_else(|| ApiError::session_not_found(&request.session_id))?;

        match record.exec_state {
            SessionExecState::Running | SessionExecState::Interrupting => {
                return Err(ApiError::validation(
                    "A command is already running in this session",
                ));
            }
            SessionExecState::Booting => {
                return Err(ApiError::validation("Session is still booting"));
            }
            SessionExecState::Desynced => {
                return Err(ApiError::validation(
                    "Session is desynced — resync first",
                ));
            }
            SessionExecState::Ready => {} // proceed
        }

        // Write command while we hold the lock
        write_command(&record.writer, &request.command).map_err(ApiError::execution)?;
    }

    let now = chrono::Utc::now().to_rfc3339();
    let summary = ExecutionSummary {
        id: request.execution_id.clone(),
        session_id: request.session_id.clone(),
        command: request.command,
        source: request.source,
        linked_plan_id: request.linked_plan_id,
        status: "running".to_string(),
        started_at: now.clone(),
        finished_at: None,
        exit_code: None,
    };

    let _ = app.emit(
        "terminal:execution_started",
        ExecutionStartedEvent {
            execution: summary.clone(),
        },
    );

    // Update state: Running + pending execution
    {
        let mut registry = state
            .sessions
            .lock()
            .map_err(|e| ApiError::execution(e.to_string()))?;

        if let Some(record) = registry.get_mut(&request.session_id) {
            record.exec_state = SessionExecState::Running;
            record.command_sent_at = Some(now);
            record.pending_execution_id = Some(request.execution_id);
        }
    }

    emit_exec_state(&app, &request.session_id, &SessionExecState::Running);

    Ok(TerminalExecuteResponse { execution: summary })
}

#[tauri::command]
pub fn terminal_interrupt(
    request: TerminalInterruptRequest,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<TerminalInterruptResponse, ApiError> {
    let mut registry = state
        .sessions
        .lock()
        .map_err(|e| ApiError::execution(e.to_string()))?;

    let record = registry
        .get_mut(&request.session_id)
        .ok_or_else(|| ApiError::session_not_found(&request.session_id))?;

    if record.exec_state != SessionExecState::Running {
        return Err(ApiError::validation("No command is currently running"));
    }

    // Send Ctrl+C (ETX byte)
    write_raw(&record.writer, "\x03").map_err(ApiError::execution)?;

    record.exec_state = SessionExecState::Interrupting;

    drop(registry);

    emit_exec_state(&app, &request.session_id, &SessionExecState::Interrupting);
    eprintln!("[terminal] interrupt sent to session {}", request.session_id);

    Ok(TerminalInterruptResponse { ok: true })
}

#[tauri::command]
pub fn terminal_resync(
    request: TerminalResyncRequest,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<TerminalResyncResponse, ApiError> {
    let mut registry = state
        .sessions
        .lock()
        .map_err(|e| ApiError::execution(e.to_string()))?;

    let record = registry
        .get_mut(&request.session_id)
        .ok_or_else(|| ApiError::session_not_found(&request.session_id))?;

    // Send newline to provoke a new prompt
    write_raw(&record.writer, "\n").map_err(ApiError::execution)?;

    // Reset state — wait for prompt marker to transition back to Ready
    record.exec_state = SessionExecState::Booting;
    record.pending_execution_id = None;
    record.command_sent_at = None;

    drop(registry);

    emit_exec_state(&app, &request.session_id, &SessionExecState::Booting);
    eprintln!("[terminal] resync initiated for session {}", request.session_id);

    Ok(TerminalResyncResponse { ok: true })
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
