#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use commandui_desktop::state::AppState;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| {
            let app_data = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            std::fs::create_dir_all(&app_data).ok();

            let db_path = app_data.join("commandui.sqlite");
            let conn =
                commandui_desktop::db::sqlite::open_database(&db_path).expect("failed to open SQLite database");
            commandui_desktop::db::schema::init_schema(&conn).expect("failed to initialize database schema");

            let state = app.state::<AppState>();
            let mut path = state.db_path.lock().unwrap();
            *path = Some(db_path);

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running CommandUI");
}
