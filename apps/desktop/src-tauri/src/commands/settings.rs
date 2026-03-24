use crate::state::AppState;
use crate::types::errors::ApiError;
use commandui_runtime_persistence::db::open_database;
use commandui_runtime_persistence::settings::{self, SettingsSnapshot};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsGetResponse {
    pub settings: SettingsSnapshot,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdateRequest {
    pub settings: SettingsSnapshot,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdateResponse {
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
pub fn settings_get(state: State<'_, AppState>) -> Result<SettingsGetResponse, ApiError> {
    let conn = get_conn(&state)?;
    let s = settings::get(&conn).map_err(ApiError::database)?;
    Ok(SettingsGetResponse { settings: s })
}

#[tauri::command]
pub fn settings_update(
    request: SettingsUpdateRequest,
    state: State<'_, AppState>,
) -> Result<SettingsUpdateResponse, ApiError> {
    let conn = get_conn(&state)?;
    settings::update(&conn, &request.settings).map_err(ApiError::database)?;
    Ok(SettingsUpdateResponse { ok: true })
}
