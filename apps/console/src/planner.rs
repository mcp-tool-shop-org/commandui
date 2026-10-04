//! Console planner adapter — thin wrapper over the shared runtime-planner.
//!
//! Console-specific: builds PlanContext from Console's model state.
//! All plan generation, validation, and mock fallback are in the shared crate.

pub use commandui_runtime_planner::OllamaConfig;
use commandui_runtime_planner::{self as planner, PlanContext, PlannerAnswer};

/// Build a PlanContext from Console's current state, with the default shell.
#[allow(dead_code)]
pub fn build_context(
    session_id: &str,
    cwd: &str,
) -> PlanContext {
    build_context_for_shell(session_id, cwd, None)
}

/// Build a PlanContext for a session whose shell is known. The planner writes
/// commands for the shell the session actually runs, not for COMSPEC.
pub fn build_context_for_shell(
    session_id: &str,
    cwd: &str,
    shell: Option<&str>,
) -> PlanContext {
    PlanContext {
        session_id: session_id.to_string(),
        cwd: cwd.to_string(),
        os: std::env::consts::OS.to_string(),
        shell: detect_shell(shell),
        ..Default::default()
    }
}

/// Generate a proposal using the shared planner.
/// A debug build may return a labeled stand-in. A release build returns no command.
pub async fn generate_proposal(
    config: &OllamaConfig,
    context: &PlanContext,
    user_intent: &str,
) -> PlannerAnswer {
    planner::generate_proposal(config, context, user_intent).await
}

/// The session's own shell when known. Otherwise the shell a session with no
/// shell set resolves to (pwsh or powershell on Windows, $SHELL elsewhere).
fn detect_shell(session_shell: Option<&str>) -> String {
    match session_shell.filter(|s| !s.is_empty()) {
        Some(shell) => shell.to_string(),
        None => commandui_runtime_core::pty::default_shell(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_context() {
        let ctx = build_context("s1", "/home/user");
        assert_eq!(ctx.session_id, "s1");
        assert_eq!(ctx.cwd, "/home/user");
        assert!(!ctx.os.is_empty());
        assert!(!ctx.shell.is_empty());
    }

    #[test]
    fn the_planner_is_told_the_sessions_real_shell_not_comspec() {
        let ctx = build_context_for_shell("s1", "C:/work", Some("pwsh.exe"));
        assert_eq!(ctx.shell, "pwsh.exe");
        // No shell recorded: the default a session would resolve to, never COMSPEC.
        let ctx = build_context_for_shell("s1", "C:/work", None);
        assert_eq!(ctx.shell, commandui_runtime_core::pty::default_shell());
        #[cfg(windows)]
        assert!(!ctx.shell.to_lowercase().ends_with("cmd.exe"));
    }

    #[test]
    fn test_shared_mock_git_status() {
        let ctx = build_context("test", "/tmp");
        let p = commandui_runtime_planner::mock::generate(&ctx, "show changed files");
        assert_eq!(p.command, "git status --short");
        assert_eq!(p.source, "mock");
    }

    #[test]
    fn test_shared_mock_destructive() {
        let ctx = build_context("test", "/tmp");
        let p = commandui_runtime_planner::mock::generate(&ctx, "delete old logs");
        assert_eq!(p.risk, "high");
        assert!(p.destructive);
    }

    #[tokio::test]
    async fn generate_proposal_falls_back_when_the_endpoint_is_closed() {
        let config = OllamaConfig {
            endpoint: "http://127.0.0.1:9".into(),
            model: "unused".into(),
            timeout_secs: 1,
        };
        let ctx = build_context("s1", "/work");
        let answer = planner::answer_for(&config, &ctx, "show changed files", false).await;
        let dumped = serde_json::to_string(&answer).unwrap();
        match answer {
            PlannerAnswer::Unavailable { status } => {
                assert!(status.state == "notInstalled" || status.state == "notRunning");
                assert!(!dumped.contains("\"command\""), "{dumped}");
            }
            PlannerAnswer::Proposal { proposal, .. } => {
                panic!("release returned {}", proposal.command)
            }
        }
    }
}
