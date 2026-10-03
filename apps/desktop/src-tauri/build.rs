fn main() {
    // App ACL manifest for the commands the desktop UI invokes.
    // session_update_cwd is registered in main.rs but has no tauriInvoke
    // caller, so it stays deny-by-default and is omitted here.
    // Underscores become hyphens in the generated allow-<command> identifiers.
    let commands = &[
        "session_create",
        "session_list",
        "session_close",
        "terminal_execute",
        "terminal_interrupt",
        "terminal_resize",
        "terminal_resync",
        "terminal_write",
        "planner_generate_plan",
        "history_append",
        "history_list",
        "history_update",
        "plan_store",
        "settings_get",
        "settings_update",
        "workflow_add",
        "workflow_delete",
        "workflow_list",
        "memory_list",
        "memory_add",
        "memory_accept_suggestion",
        "memory_dismiss_suggestion",
        "memory_delete",
        "memory_store_suggestion",
    ];

    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(commands)),
    )
    .expect("failed to run tauri-build");
}
