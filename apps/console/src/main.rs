mod app;
mod event_sink;
mod input;
mod model;
mod planner;
mod ui;

use commandui_runtime_core::services::session_service::SessionService;
use commandui_runtime_core::services::terminal_service::TerminalService;
use commandui_runtime_core::session::SessionRegistry;
use event_sink::shared_channel_sink;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Channel for runtime events → Console UI
    let (tx, rx) = mpsc::unbounded_channel();
    let sink = shared_channel_sink(tx);

    // Shared session registry
    let sessions = Arc::new(Mutex::new(SessionRegistry::new()));

    // Runtime services — same ones Desktop uses, no Tauri anywhere
    let session_service = SessionService::new(sessions.clone(), sink.clone());
    let terminal_service = TerminalService::new(sessions, sink);

    // Run Console
    let mut app = app::App::new(session_service, terminal_service, rx);
    app.run().await
}
