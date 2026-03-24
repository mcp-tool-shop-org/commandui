//! Console planner adapter — thin wrapper over the shared runtime-planner.
//!
//! Console-specific: builds PlanContext from Console's model state.
//! All plan generation, validation, and mock fallback are in the shared crate.

pub use commandui_runtime_planner::OllamaConfig;
use commandui_runtime_planner::{self as planner, CommandProposal, PlanContext};

/// Build a PlanContext from Console's current state.
pub fn build_context(
    session_id: &str,
    cwd: &str,
) -> PlanContext {
    PlanContext {
        session_id: session_id.to_string(),
        cwd: cwd.to_string(),
        os: std::env::consts::OS.to_string(),
        shell: detect_shell(),
        ..Default::default()
    }
}

/// Generate a proposal using the shared planner.
pub async fn generate_proposal(
    config: &OllamaConfig,
    context: &PlanContext,
    user_intent: &str,
) -> CommandProposal {
    planner::generate_proposal(config, context, user_intent).await
}

fn detect_shell() -> String {
    #[cfg(windows)]
    {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
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
}
