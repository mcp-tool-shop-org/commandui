// Pulls in the comctl32 v6 manifest. Bins already get that from tauri-build.
#[cfg(windows)]
#[link(name = "commandui_test_manifest", kind = "static", modifiers = "+whole-archive")]
extern "C" {
    fn commandui_test_manifest_anchor();
}

fn keep_windows_manifest_linked() {
    #[cfg(windows)]
    unsafe {
        commandui_test_manifest_anchor();
    }
}

use crate::state::AppState;
use commandui_runtime_core::events::{NoopSink, RuntimeEventSink};
use commandui_runtime_core::session::SessionExecState;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::{Manager, WebviewWindow, WebviewWindowBuilder};

fn new_state() -> AppState {
    let sink: Arc<dyn RuntimeEventSink> = Arc::new(NoopSink);
    let mut state = AppState::new(sink);
    // Port 1 refuses immediately. The default host is a local Ollama with a 30s timeout.
    state.ollama.endpoint = "http://127.0.0.1:1".into();
    state.ollama.timeout_secs = 1;
    state
}

fn test_app() -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
    keep_windows_manifest_linked();
    let app = mock_builder()
        .manage(new_state())
        .invoke_handler(tauri::generate_handler![
            super::history::history_append,
            super::history::history_list,
            super::history::history_update,
            super::history::plan_store,
            super::memory::memory_list,
            super::memory::memory_add,
            super::memory::memory_accept_suggestion,
            super::memory::memory_dismiss_suggestion,
            super::memory::memory_delete,
            super::memory::memory_store_suggestion,
            super::memory::memory_list_resolved_suggestions,
            super::workflow::workflow_add,
            super::workflow::workflow_list,
            super::workflow::workflow_delete,
            super::settings::settings_get,
            super::settings::settings_update,
            super::session::session_create,
            super::session::session_list,
            super::session::session_close,
            super::session::session_update_cwd,
            super::terminal::terminal_execute,
            super::terminal::terminal_interrupt,
            super::terminal::terminal_resize,
            super::terminal::terminal_resync,
            super::terminal::terminal_write,
            super::planner::planner_generate_plan,
        ])
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock webview");
    (app, webview)
}

fn ipc(webview: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
    match get_ipc_response(
        webview,
        tauri::webview::InvokeRequest {
            cmd: cmd.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    ) {
        Ok(body) => Ok(body.deserialize().expect("ok payload is json")),
        Err(err) => Err(err),
    }
}

fn assert_api_error(err: &Value, code: &str, message_part: &str) {
    assert_eq!(err["code"], code, "{err}");
    let message = err["message"].as_str().unwrap_or("");
    assert!(
        message.contains(message_part),
        "expected {message_part:?} in {err}"
    );
}

fn assert_ok(value: &Value) {
    assert_eq!(value["ok"], true, "{value}");
}

fn set_db(app: &tauri::App<MockRuntime>, path: Option<PathBuf>) {
    let state = app.state::<AppState>();
    *state.db_path.lock().expect("db path") = path;
}

struct TempDb(PathBuf);

impl TempDb {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("commandui-ipc-{}.sqlite", uuid::Uuid::new_v4()));
        let conn = commandui_runtime_persistence::db::open_database(&path).expect("create db");
        commandui_runtime_persistence::schema::init_schema(&conn).expect("schema");
        drop(conn);
        Self(path)
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        remove_file_retry(&self.0);
    }
}

fn remove_file_retry(path: &Path) {
    for _ in 0..20 {
        if std::fs::remove_file(path).is_ok() || !path.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("commandui-cwd-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).expect("temp cwd");
        Self(path)
    }

    fn path_str(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        for _ in 0..20 {
            if std::fs::remove_dir(&self.0).is_ok() || !self.0.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// Closes the shell if the test panics before `session_close`.
struct SessionClose<'a> {
    webview: &'a WebviewWindow<MockRuntime>,
    session_id: String,
    closed: bool,
}

impl Drop for SessionClose<'_> {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        let _ = ipc(
            self.webview,
            "session_close",
            json!({ "request": { "sessionId": self.session_id } }),
        );
    }
}

fn history_item() -> Value {
    json!({
        "id": "h1",
        "sessionId": "s1",
        "source": "ask",
        "userInput": "list files",
        "generatedCommand": null,
        "executedCommand": null,
        "linkedPlanId": null,
        "plannerRequestId": null,
        "status": "planned",
        "exitCode": null,
        "createdAt": "2026-01-01T00:00:00Z",
        "finishedAt": null,
        "durationMs": null,
        "cwd": null,
        "plannerSource": null
    })
}

#[test]
fn database_commands_reject_uninitialized_and_directory() {
    let (app, webview) = test_app();
    for cmd in ["history_list", "memory_list", "workflow_list", "settings_get"] {
        let body = if cmd == "history_list" {
            json!({ "request": { "sessionId": null, "limit": 5 } })
        } else {
            json!({})
        };
        let err = ipc(&webview, cmd, body).expect_err(cmd);
        assert_api_error(&err, "DATABASE_ERROR", "Database not initialized");
    }

    set_db(&app, Some(std::env::temp_dir()));
    for cmd in ["history_list", "memory_list", "workflow_list", "settings_get"] {
        let body = if cmd == "history_list" {
            json!({ "request": { "sessionId": null, "limit": 5 } })
        } else {
            json!({})
        };
        let err = ipc(&webview, cmd, body).expect_err(cmd);
        assert_api_error(&err, "DATABASE_ERROR", "Failed to open database");
    }
}

#[test]
fn database_commands_round_trip() {
    let db = TempDb::new();
    let (app, webview) = test_app();
    set_db(&app, Some(db.0.clone()));

    assert_ok(&ipc(&webview, "history_append", json!({ "request": { "item": history_item() } })).unwrap());
    let listed = ipc(
        &webview,
        "history_list",
        json!({ "request": { "sessionId": "s1", "limit": 10 } }),
    )
    .unwrap();
    assert_eq!(listed["items"][0]["id"], "h1");
    assert_eq!(listed["items"][0]["userInput"], "list files");
    assert_eq!(listed["items"][0]["status"], "planned");

    assert_ok(
        &ipc(
            &webview,
            "history_update",
            json!({
                "request": {
                    "historyId": "h1",
                    "status": "success",
                    "exitCode": 0,
                    "executedCommand": "Get-ChildItem",
                    "finishedAt": "2026-01-01T00:00:01Z",
                    "durationMs": 12
                }
            }),
        )
        .unwrap(),
    );
    // An unknown id changes no row and is reported, not silently accepted.
    let missing = ipc(
        &webview,
        "history_update",
        json!({ "request": { "historyId": "missing-history", "status": "gone" } }),
    )
    .unwrap_err();
    assert_api_error(&missing, "DATABASE_ERROR", "no history item");
    let listed = ipc(
        &webview,
        "history_list",
        json!({ "request": { "sessionId": "s1", "limit": 10 } }),
    )
    .unwrap();
    assert_eq!(listed["items"][0]["status"], "success");
    assert_eq!(listed["items"][0]["exitCode"], 0);
    assert_eq!(listed["items"][0]["executedCommand"], "Get-ChildItem");

    assert_ok(
        &ipc(
            &webview,
            "plan_store",
            json!({
                "request": {
                    "plan": {
                        "id": "p1",
                        "sessionId": "s1",
                        "userIntent": "list files",
                        "command": "Get-ChildItem",
                        "risk": "low",
                        "explanation": "lists the directory",
                        "generatedAt": "2026-01-01T00:00:00Z"
                    }
                }
            }),
        )
        .unwrap(),
    );

    let memory_item = json!({
        "id": "m1",
        "scope": "project",
        "projectRoot": null,
        "kind": "pref",
        "key": "editor",
        "value": "vim",
        "confidence": 0.5,
        "source": "user",
        "createdAt": "2026-01-01T00:00:00Z",
        "updatedAt": "2026-01-01T00:00:00Z"
    });
    assert_ok(&ipc(&webview, "memory_add", json!({ "request": { "item": memory_item } })).unwrap());
    let suggestion = json!({
        "id": "sg1",
        "scope": "project",
        "projectRoot": null,
        "kind": "pref",
        "label": "Editor",
        "proposedKey": "editor",
        "proposedValue": "helix",
        "confidence": 0.8,
        "derivedFromHistoryIds": ["h1"],
        "status": "pending",
        "createdAt": "2026-01-01T00:00:02Z"
    });
    assert_ok(
        &ipc(
            &webview,
            "memory_store_suggestion",
            json!({ "request": { "suggestion": suggestion } }),
        )
        .unwrap(),
    );
    let memory = ipc(&webview, "memory_list", json!({})).unwrap();
    assert_eq!(memory["items"][0]["id"], "m1");
    assert_eq!(memory["suggestions"][0]["id"], "sg1");

    let accepted = ipc(
        &webview,
        "memory_accept_suggestion",
        json!({ "request": { "suggestionId": "sg1" } }),
    )
    .unwrap();
    assert_eq!(accepted["ok"], true);
    assert_eq!(accepted["createdItem"]["key"], "editor");
    assert_eq!(accepted["createdItem"]["value"], "helix");
    let created_id = accepted["createdItem"]["id"].as_str().unwrap().to_string();

    let missing = ipc(
        &webview,
        "memory_accept_suggestion",
        json!({ "request": { "suggestionId": "missing-suggestion" } }),
    )
    .unwrap_err();
    assert_api_error(&missing, "DATABASE_ERROR", "suggestion not found");

    // A second accept of an already accepted suggestion is rejected, not a silent no-op.
    let repeated = ipc(
        &webview,
        "memory_accept_suggestion",
        json!({ "request": { "suggestionId": "sg1" } }),
    )
    .unwrap_err();
    assert_api_error(&repeated, "DATABASE_ERROR", "suggestion not pending");

    let second = json!({
        "id": "sg2",
        "scope": "project",
        "projectRoot": null,
        "kind": "pref",
        "label": "Shell",
        "proposedKey": "shell",
        "proposedValue": "pwsh",
        "confidence": 0.4,
        "derivedFromHistoryIds": [],
        "status": "pending",
        "createdAt": "2026-01-01T00:00:03Z"
    });
    assert_ok(
        &ipc(
            &webview,
            "memory_store_suggestion",
            json!({ "request": { "suggestion": second } }),
        )
        .unwrap(),
    );
    assert_ok(
        &ipc(
            &webview,
            "memory_dismiss_suggestion",
            json!({ "request": { "suggestionId": "sg2" } }),
        )
        .unwrap(),
    );
    let memory = ipc(&webview, "memory_list", json!({})).unwrap();
    let suggestion_ids: Vec<_> = memory["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert!(!suggestion_ids.contains(&"sg2"));
    let missing = ipc(
        &webview,
        "memory_dismiss_suggestion",
        json!({ "request": { "suggestionId": "missing-suggestion" } }),
    )
    .unwrap_err();
    assert_api_error(&missing, "DATABASE_ERROR", "no suggestion");

    assert_ok(
        &ipc(
            &webview,
            "memory_delete",
            json!({ "request": { "memoryId": created_id } }),
        )
        .unwrap(),
    );
    let memory = ipc(&webview, "memory_list", json!({})).unwrap();
    let item_ids: Vec<_> = memory["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert!(!item_ids.contains(&created_id.as_str()));
    assert!(item_ids.contains(&"m1"));
    let missing = ipc(
        &webview,
        "memory_delete",
        json!({ "request": { "memoryId": "missing-memory" } }),
    )
    .unwrap_err();
    assert_api_error(&missing, "DATABASE_ERROR", "no memory item");

    let workflow = json!({
        "id": "w1",
        "label": "status",
        "source": "ask",
        "originalIntent": "show status",
        "command": "git status --short",
        "stepsJson": null,
        "projectRoot": null,
        "createdAt": "2026-01-01T00:00:00Z"
    });
    assert_ok(&ipc(&webview, "workflow_add", json!({ "request": { "workflow": workflow } })).unwrap());
    let workflows = ipc(&webview, "workflow_list", json!({})).unwrap();
    assert_eq!(workflows["workflows"][0]["id"], "w1");
    assert_eq!(workflows["workflows"][0]["command"], "git status --short");
    assert_ok(&ipc(&webview, "workflow_delete", json!({ "request": { "id": "w1" } })).unwrap());
    let missing = ipc(
        &webview,
        "workflow_delete",
        json!({ "request": { "id": "missing-workflow" } }),
    )
    .unwrap_err();
    assert_api_error(&missing, "DATABASE_ERROR", "no workflow");
    let workflows = ipc(&webview, "workflow_list", json!({})).unwrap();
    assert!(workflows["workflows"].as_array().unwrap().is_empty());

    let settings = ipc(&webview, "settings_get", json!({})).unwrap();
    assert_eq!(settings["settings"]["theme"], "dark");
    assert_eq!(settings["settings"]["confirmMediumRisk"], true);
    assert_ok(
        &ipc(
            &webview,
            "settings_update",
            json!({ "request": { "settings": { "theme": "light", "confirmMediumRisk": false } } }),
        )
        .unwrap(),
    );
    let settings = ipc(&webview, "settings_get", json!({})).unwrap();
    assert_eq!(settings["settings"]["theme"], "light");
    assert_eq!(settings["settings"]["confirmMediumRisk"], false);
    assert_eq!(settings["settings"]["fontSize"], "md");

    drop(app);
    drop(webview);
    remove_file_retry(&db.0);
    assert!(!db.0.exists(), "temp database was not deleted");
}

fn force_exec(app: &tauri::App<MockRuntime>, session_id: &str, exec: SessionExecState) {
    let state = app.state::<AppState>();
    let mut sessions = state.sessions.lock().expect("sessions");
    sessions
        .set_exec_state(session_id, exec)
        .expect("session still open");
}

fn terminal_request(session_id: &str) -> Value {
    json!({ "request": { "sessionId": session_id } })
}

#[test]
fn session_and_terminal_commands_cover_lifecycle() {
    let (app, webview) = test_app();
    let cwd = TempDir::new();
    let next = TempDir::new();

    let fish = ipc(
        &webview,
        "session_create",
        json!({ "request": { "label": null, "cwd": null, "shell": "fish" } }),
    )
    .unwrap_err();
    assert_api_error(&fish, "EXECUTION_FAILED", "unsupported shell");

    let created = ipc(
        &webview,
        "session_create",
        json!({ "request": { "label": null, "cwd": cwd.path_str(), "shell": null } }),
    )
    .expect("session_create");
    let session_id = created["session"]["id"].as_str().unwrap().to_string();
    assert!(!session_id.is_empty());
    assert_eq!(created["session"]["cwd"], cwd.path_str());
    assert_eq!(created["session"]["label"], "Session");
    assert_eq!(created["session"]["status"], "active");
    assert!(!created["session"]["shell"].as_str().unwrap().is_empty());

    let mut guard = SessionClose {
        webview: &webview,
        session_id: session_id.clone(),
        closed: false,
    };

    let listed = ipc(&webview, "session_list", json!({})).unwrap();
    let found = listed["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == session_id)
        .expect("created session is listed");
    assert_eq!(found["cwd"], cwd.path_str());
    // The list carries the live exec state so a reloaded webview can seed itself.
    assert!(
        found["execState"].is_string(),
        "session_list must report execState, got {found}"
    );

    assert_ok(
        &ipc(
            &webview,
            "session_update_cwd",
            json!({ "request": { "sessionId": session_id, "cwd": next.path_str() } }),
        )
        .unwrap(),
    );
    let listed = ipc(&webview, "session_list", json!({})).unwrap();
    let found = listed["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == session_id)
        .unwrap();
    assert_eq!(found["cwd"], next.path_str());

    let empty = ipc(
        &webview,
        "session_update_cwd",
        json!({ "request": { "sessionId": session_id, "cwd": "" } }),
    )
    .unwrap_err();
    assert_api_error(&empty, "VALIDATION_FAILED", "cwd cannot be empty");

    assert_ok(
        &ipc(
            &webview,
            "terminal_resize",
            json!({ "request": { "sessionId": session_id, "cols": 80, "rows": 24 } }),
        )
        .unwrap(),
    );
    assert_ok(
        &ipc(
            &webview,
            "terminal_write",
            json!({ "request": { "sessionId": session_id, "data": " " } }),
        )
        .unwrap(),
    );
    assert_ok(&ipc(&webview, "terminal_resync", terminal_request(&session_id)).unwrap());

    let mut execution = None;
    for _ in 0..8 {
        force_exec(&app, &session_id, SessionExecState::Ready);
        match ipc(
            &webview,
            "terminal_execute",
            json!({
                "request": {
                    "executionId": "e1",
                    "sessionId": session_id,
                    "command": "echo cui-ok",
                    "source": "test",
                    "linkedPlanId": null
                }
            }),
        ) {
            Ok(value) => {
                execution = Some(value);
                break;
            }
            Err(err) => {
                let message = err["message"].as_str().unwrap_or("");
                assert!(
                    message.contains("booting")
                        || message.contains("desynced")
                        || message.contains("already running"),
                    "{err}"
                );
                if message.contains("already running") {
                    force_exec(&app, &session_id, SessionExecState::Running);
                    let _ = ipc(&webview, "terminal_interrupt", terminal_request(&session_id));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    let execution = execution.expect("terminal_execute");
    assert_eq!(execution["execution"]["command"], "echo cui-ok");
    assert_eq!(execution["execution"]["sessionId"], session_id);
    assert_eq!(execution["execution"]["id"], "e1");

    if ipc(&webview, "terminal_interrupt", terminal_request(&session_id)).is_err() {
        // The echo can finish before the interrupt. Running is what the command's Ok arm needs.
        force_exec(&app, &session_id, SessionExecState::Running);
        assert_ok(&ipc(&webview, "terminal_interrupt", terminal_request(&session_id)).unwrap());
    }

    assert_ok(&ipc(&webview, "session_close", terminal_request(&session_id)).unwrap());
    guard.closed = true;
    assert!(app.state::<AppState>().sessions.lock().unwrap().list().is_empty());

    let listed = ipc(&webview, "session_list", json!({})).unwrap();
    assert!(listed["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| s["id"] != session_id));

    let closed = ipc(&webview, "session_close", terminal_request(&session_id)).unwrap_err();
    assert_api_error(&closed, "EXECUTION_FAILED", "Session not found");

    for cmd in [
        "terminal_resize",
        "terminal_write",
        "terminal_resync",
        "terminal_execute",
        "terminal_interrupt",
    ] {
        let body = match cmd {
            "terminal_resize" => json!({ "request": { "sessionId": session_id, "cols": 80, "rows": 24 } }),
            "terminal_write" => json!({ "request": { "sessionId": session_id, "data": " " } }),
            "terminal_execute" => json!({
                "request": {
                    "executionId": "e2",
                    "sessionId": session_id,
                    "command": "echo cui-ok",
                    "source": "test",
                    "linkedPlanId": null
                }
            }),
            _ => terminal_request(&session_id),
        };
        let err = ipc(&webview, cmd, body).expect_err(cmd);
        assert_api_error(&err, "EXECUTION_FAILED", "Session not found");
    }
}

#[test]
fn planner_generate_plan_falls_back_to_mock() {
    let (_app, webview) = test_app();
    let empty = ipc(
        &webview,
        "planner_generate_plan",
        json!({
            "request": {
                "sessionId": "s1",
                "userIntent": "",
                "context": {
                    "sessionId": "s1",
                    "cwd": "/work",
                    "projectRoot": null,
                    "os": "windows",
                    "shell": "powershell",
                    "recentCommands": [],
                    "memoryItems": [],
                    "projectFacts": []
                }
            }
        }),
    )
    .unwrap_err();
    assert_api_error(&empty, "VALIDATION_FAILED", "user_intent cannot be empty");

    let proposal = ipc(
        &webview,
        "planner_generate_plan",
        json!({
            "request": {
                "sessionId": "s1",
                "userIntent": "show me changed files",
                "context": {
                    "sessionId": "s1",
                    "cwd": "/work",
                    "projectRoot": "/work",
                    "os": "windows",
                    "shell": "powershell",
                    "recentCommands": ["git status"],
                    "memoryItems": [{
                        "kind": "pref",
                        "key": "editor",
                        "value": "vim",
                        "confidence": 0.5
                    }],
                    "projectFacts": [{
                        "kind": "tool",
                        "label": "cargo",
                        "value": "test"
                    }]
                }
            }
        }),
    )
    .expect("planner_generate_plan");
    assert_eq!(proposal["plan"]["source"], "mock");
    assert_eq!(proposal["plan"]["command"], "git status --short");
    assert_eq!(proposal["plan"]["userIntent"], "show me changed files");
    assert_eq!(proposal["review"]["planId"], proposal["plan"]["id"]);
    let retrieved = proposal["review"]["retrievedContext"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(retrieved.iter().any(|line| line == "tool:cargo"), "{retrieved:?}");
}
