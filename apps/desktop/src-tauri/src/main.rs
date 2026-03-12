#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use commandui_desktop::commands::{
    history_append, history_list, history_update, memory_accept_suggestion, memory_add,
    memory_delete, memory_dismiss_suggestion, memory_list, plan_store, planner_generate_plan,
    session_close, session_create, session_list, session_update_cwd, settings_get, settings_update,
    terminal_execute, terminal_resize, terminal_write, workflow_add, workflow_list,
};
use commandui_desktop::state::AppState;

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
            session_create,
            session_list,
            session_close,
            session_update_cwd,
            terminal_execute,
            terminal_resize,
            terminal_write,
            planner_generate_plan,
            history_append,
            history_list,
            history_update,
            plan_store,
            settings_get,
            settings_update,
            workflow_add,
            workflow_list,
            memory_list,
            memory_add,
            memory_accept_suggestion,
            memory_dismiss_suggestion,
            memory_delete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CommandUI");
}
