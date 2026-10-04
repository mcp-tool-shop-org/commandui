#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use commandui_desktop::events::TauriEventSink;
use commandui_desktop::navigation::app_navigation_allowed;
use commandui_desktop::state::AppState;
use std::sync::Arc;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(
            tauri::plugin::Builder::<tauri::Wry, ()>::new("navigation-guard")
                .on_navigation(|webview, url| {
                    let dev_url = webview.app_handle().config().build.dev_url.as_ref();
                    app_navigation_allowed(url, dev_url)
                })
                .build(),
        )
        .setup(|app| {
            // Create the event sink with the real AppHandle
            let sink = Arc::new(TauriEventSink::new(app.handle().clone()));

            // Create AppState with the sink
            let state = AppState::new(sink);

            // Initialize database
            let app_data = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("Failed to resolve app data directory: {e}"))?;
            std::fs::create_dir_all(&app_data).map_err(|e| {
                format!("Failed to create app data directory at {}: {e}", app_data.display())
            })?;

            let db_path = app_data.join("commandui.sqlite");
            let conn = commandui_desktop::db::sqlite::open_database(&db_path).map_err(|e| {
                format!("Failed to open database at {}: {e}", db_path.display())
            })?;
            commandui_desktop::db::schema::init_schema(&conn)
                .map_err(|e| format!("Failed to initialize database schema: {e}"))?;

            {
                let mut path = state.db_path.lock().unwrap();
                *path = Some(db_path);
            }

            // Register state with Tauri
            app.manage(state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commandui_desktop::commands::session::session_create,
            commandui_desktop::commands::session::session_list,
            commandui_desktop::commands::session::session_close,
            commandui_desktop::commands::session::session_update_cwd,
            commandui_desktop::commands::terminal::terminal_execute,
            commandui_desktop::commands::terminal::terminal_interrupt,
            commandui_desktop::commands::terminal::terminal_resize,
            commandui_desktop::commands::terminal::terminal_resync,
            commandui_desktop::commands::terminal::terminal_write,
            commandui_desktop::commands::planner::planner_generate_plan,
            commandui_desktop::commands::history::history_append,
            commandui_desktop::commands::history::history_list,
            commandui_desktop::commands::history::history_update,
            commandui_desktop::commands::history::plan_store,
            commandui_desktop::commands::settings::settings_get,
            commandui_desktop::commands::settings::settings_update,
            commandui_desktop::commands::workflow::workflow_add,
            commandui_desktop::commands::workflow::workflow_delete,
            commandui_desktop::commands::workflow::workflow_list,
            commandui_desktop::commands::memory::memory_list,
            commandui_desktop::commands::memory::memory_add,
            commandui_desktop::commands::memory::memory_accept_suggestion,
            commandui_desktop::commands::memory::memory_dismiss_suggestion,
            commandui_desktop::commands::memory::memory_delete,
            commandui_desktop::commands::memory::memory_store_suggestion,
            commandui_desktop::commands::memory::memory_list_resolved_suggestions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CommandUI");
}
