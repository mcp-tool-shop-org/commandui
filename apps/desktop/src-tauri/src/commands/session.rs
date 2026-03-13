use crate::shell::pty_manager::{
    bootstrap_prompt, default_shell, spawn_reader_loop, spawn_shell, write_raw, PROMPT_MARKER,
};
use crate::shell::session_registry::{SessionExecState, SessionRecord};
use crate::state::AppState;
use crate::types::errors::ApiError;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

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

// --- Events ---

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalLineEvent {
    pub id: String,
    pub session_id: String,
    pub execution_id: Option<String>,
    pub kind: String,
    pub text: String,
    pub timestamp: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCwdChangedEvent {
    pub session_id: String,
    pub cwd: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionFinishedEvent {
    pub execution_id: String,
    pub session_id: String,
    pub exit_code: i32,
    pub finished_at: String,
    pub status: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionReadyEvent {
    pub session_id: String,
    pub cwd: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionExecStateChangedEvent {
    pub session_id: String,
    pub exec_state: String,
    pub changed_at: String,
}

// --- Helper: emit exec state change ---

fn emit_exec_state_changed(app: &AppHandle, session_id: &str, state: &SessionExecState) {
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
pub fn session_create(
    request: SessionCreateRequest,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<SessionCreateResponse, ApiError> {
    let id = uuid::Uuid::new_v4().to_string();
    let label = request.label.unwrap_or_else(|| "Session".to_string());
    let shell = request.shell.unwrap_or_else(default_shell);
    let cwd = request
        .cwd
        .unwrap_or_else(|| std::env::current_dir().unwrap().to_string_lossy().to_string());
    let now = chrono::Utc::now().to_rfc3339();

    let (pair, writer) =
        spawn_shell(&shell, Some(&cwd)).map_err(|e| ApiError::execution(e))?;

    let session_id_for_reader = id.clone();
    let app_for_reader = app.clone();
    let state_sessions = state.sessions.clone();

    spawn_reader_loop(&pair, move |text| {
        // Line buffer: split chunk into lines, process each one.
        // Lines containing PROMPT_MARKER are consumed for state — NOT forwarded to xterm.
        let mut display_text = String::new();

        // Read current execution_id and exec_state for attribution
        let (current_exec_id, _current_exec_state) = state_sessions
            .lock()
            .ok()
            .and_then(|reg| {
                let record = reg.get(&session_id_for_reader)?;
                Some((
                    record.pending_execution_id.clone(),
                    record.exec_state.clone(),
                ))
            })
            .unwrap_or((None, SessionExecState::Booting));

        for line in text.split_inclusive('\n') {
            if line.contains(PROMPT_MARKER) {
                // Parse marker: __COMMANDUI_PROMPT__|cwd|exitcode
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 2 {
                    let cwd = parts[1].to_string();
                    let exit_code = if parts.len() >= 3 {
                        parts[2].trim().parse::<i32>().unwrap_or(0)
                    } else {
                        0
                    };

                    // Update session cwd + check boot status
                    let (was_booting, pending_exec, was_interrupting) = {
                        if let Ok(mut reg) = state_sessions.lock() {
                            if let Some(record) = reg.get_mut(&session_id_for_reader) {
                                record.cwd = cwd.clone();
                                let was_boot = !record.boot_prompt_received;
                                let was_int =
                                    record.exec_state == SessionExecState::Interrupting;
                                let pending = record.pending_execution_id.clone();

                                if was_boot {
                                    record.boot_prompt_received = true;
                                }

                                // Transition to Ready
                                record.exec_state = SessionExecState::Ready;
                                record.command_sent_at = None;

                                (was_boot, pending, was_int)
                            } else {
                                (false, None, false)
                            }
                        } else {
                            (false, None, false)
                        }
                    };

                    // Emit cwd changed
                    let _ = app_for_reader.emit(
                        "session:cwd_changed",
                        SessionCwdChangedEvent {
                            session_id: session_id_for_reader.clone(),
                            cwd: cwd.clone(),
                        },
                    );

                    // Boot detection — first prompt marker
                    if was_booting {
                        let _ = app_for_reader.emit(
                            "session:ready",
                            SessionReadyEvent {
                                session_id: session_id_for_reader.clone(),
                                cwd: cwd.clone(),
                            },
                        );
                        eprintln!(
                            "[session] {} ready (cwd: {})",
                            session_id_for_reader, cwd
                        );
                    }

                    // Execution completion
                    if let Some(exec_id) = pending_exec {
                        let status = if was_interrupting {
                            "interrupted"
                        } else if exit_code == 0 {
                            "success"
                        } else {
                            "failure"
                        };

                        let _ = app_for_reader.emit(
                            "terminal:execution_finished",
                            ExecutionFinishedEvent {
                                execution_id: exec_id,
                                session_id: session_id_for_reader.clone(),
                                exit_code,
                                finished_at: chrono::Utc::now().to_rfc3339(),
                                status: status.to_string(),
                            },
                        );

                        if let Ok(mut reg) = state_sessions.lock() {
                            let _ =
                                reg.set_pending_execution(&session_id_for_reader, None);
                        }
                    }

                    // Emit state change to Ready
                    emit_exec_state_changed(
                        &app_for_reader,
                        &session_id_for_reader,
                        &SessionExecState::Ready,
                    );
                }
                // Marker line is NOT added to display_text — stripped from output
            } else {
                // Non-marker line — forward to display
                display_text.push_str(line);
            }
        }

        // Emit display text (if any) to xterm
        if !display_text.is_empty() {
            let _ = app_for_reader.emit(
                "terminal:line",
                TerminalLineEvent {
                    id: uuid::Uuid::new_v4().to_string(),
                    session_id: session_id_for_reader.clone(),
                    execution_id: current_exec_id,
                    kind: "stdout".to_string(),
                    text: display_text,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                },
            );
        }
    });

    // Inject prompt marker
    if let Some(prompt_cmd) = bootstrap_prompt(&shell) {
        write_raw(&writer, &prompt_cmd).ok();
    }

    let record = SessionRecord {
        id: id.clone(),
        label: label.clone(),
        cwd: cwd.clone(),
        shell: shell.clone(),
        status: "active".to_string(),
        pty_pair: pair,
        writer,
        pending_execution_id: None,
        exec_state: SessionExecState::Booting,
        boot_prompt_received: false,
        command_sent_at: None,
        created_at: now.clone(),
        last_active_at: now.clone(),
    };

    let summary = SessionSummaryPayload {
        id: id.clone(),
        label,
        cwd,
        shell,
        status: "active".to_string(),
        created_at: now.clone(),
        last_active_at: now,
    };

    let mut registry = state.sessions.lock().map_err(|e| ApiError::execution(e.to_string()))?;
    registry.insert(record);

    Ok(SessionCreateResponse { session: summary })
}

#[tauri::command]
pub fn session_list(state: State<'_, AppState>) -> Result<SessionListResponse, ApiError> {
    let registry = state
        .sessions
        .lock()
        .map_err(|e| ApiError::execution(e.to_string()))?;

    let sessions = registry
        .list()
        .iter()
        .map(|r| SessionSummaryPayload {
            id: r.id.clone(),
            label: r.label.clone(),
            cwd: r.cwd.clone(),
            shell: r.shell.clone(),
            status: r.status.clone(),
            created_at: r.created_at.clone(),
            last_active_at: r.last_active_at.clone(),
        })
        .collect();

    Ok(SessionListResponse { sessions })
}

#[tauri::command]
pub fn session_close(
    request: SessionCloseRequest,
    state: State<'_, AppState>,
) -> Result<SessionCloseResponse, ApiError> {
    let mut registry = state
        .sessions
        .lock()
        .map_err(|e| ApiError::execution(e.to_string()))?;

    registry
        .remove(&request.session_id)
        .ok_or_else(|| ApiError::session_not_found(&request.session_id))?;

    Ok(SessionCloseResponse { ok: true })
}

#[tauri::command]
pub fn session_update_cwd(
    request: SessionCwdUpdateRequest,
    state: State<'_, AppState>,
) -> Result<SessionCwdUpdateResponse, ApiError> {
    if request.cwd.is_empty() {
        return Err(ApiError::validation("cwd cannot be empty"));
    }

    let mut registry = state
        .sessions
        .lock()
        .map_err(|e| ApiError::execution(e.to_string()))?;

    let record = registry
        .get_mut(&request.session_id)
        .ok_or_else(|| ApiError::session_not_found(&request.session_id))?;

    record.cwd = request.cwd;
    Ok(SessionCwdUpdateResponse { ok: true })
}
