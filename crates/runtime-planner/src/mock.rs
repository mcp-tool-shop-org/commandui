//! Mock planner fallback — shared across all shells.
//!
//! Used when Ollama is unavailable. Provides reasonable defaults
//! based on intent keywords so the product is usable without a model.

use crate::types::{CommandProposal, PlanContext};

/// Fixed shell command. User text is never interpolated into it.
const MOCK_PLACEHOLDER_COMMAND: &str = "echo \"mock: placeholder\"";

fn intent_tokens(intent_lower: &str) -> Vec<String> {
    intent_lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_string())
        .collect()
}

fn has_token(tokens: &[String], word: &str) -> bool {
    tokens.iter().any(|token| token == word)
}

fn has_phrase(tokens: &[String], phrase: &[&str]) -> bool {
    tokens
        .windows(phrase.len())
        .any(|window| window.iter().zip(phrase).all(|(token, expected)| token == expected))
}

fn uses_windows_shell(context: &PlanContext) -> bool {
    let os = context.os.to_ascii_lowercase();
    let shell = context.shell.to_ascii_lowercase();
    os.contains("windows")
        || shell.contains("powershell")
        || shell.contains("pwsh")
        || shell.contains("cmd")
}

fn list_command(context: &PlanContext) -> String {
    if uses_windows_shell(context) {
        "Get-ChildItem".to_string()
    } else {
        "ls -la".to_string()
    }
}

fn disk_command(context: &PlanContext) -> String {
    if uses_windows_shell(context) {
        "Get-PSDrive".to_string()
    } else {
        "df -h".to_string()
    }
}

/// Generate a mock proposal based on intent keyword matching.
pub fn generate(context: &PlanContext, user_intent: &str) -> CommandProposal {
    let intent_lower = user_intent.to_lowercase();
    let plan_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let tokens = intent_tokens(&intent_lower);
    let (command, explanation, risk, destructive) =
        if has_token(&tokens, "delete") || has_token(&tokens, "remove") {
            (
                MOCK_PLACEHOLDER_COMMAND.to_string(),
                format!("Destructive intent detected — review carefully: {user_intent}"),
                "high".to_string(),
                true,
            )
        } else if has_phrase(&tokens, &["changed", "files"]) || has_phrase(&tokens, &["git", "status"])
        {
            (
                "git status --short".to_string(),
                "Shows modified and untracked files in the working tree.".to_string(),
                "low".to_string(),
                false,
            )
        } else if has_token(&tokens, "disk") || has_token(&tokens, "space") {
            (
                disk_command(context),
                "Shows disk usage in human-readable format.".to_string(),
                "low".to_string(),
                false,
            )
        } else if has_token(&tokens, "list") || has_token(&tokens, "show") || has_token(&tokens, "files")
        {
            (
                list_command(context),
                "Lists files in the current directory with details.".to_string(),
                "low".to_string(),
                false,
            )
        } else {
            (
                MOCK_PLACEHOLDER_COMMAND.to_string(),
                format!("Mock response for: {user_intent}"),
                "low".to_string(),
                false,
            )
        };

    let requires_confirmation = risk != "low";

    CommandProposal {
        id: plan_id,
        session_id: context.session_id.clone(),
        source: "mock".to_string(),
        user_intent: user_intent.to_string(),
        command,
        cwd: Some(context.cwd.clone()),
        explanation,
        assumptions: vec![],
        confidence: 0.95,
        risk,
        destructive,
        requires_confirmation,
        touches_files: false,
        touches_network: false,
        escalates_privileges: false,
        expected_output: None,
        generated_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> PlanContext {
        PlanContext {
            session_id: "test".to_string(),
            cwd: "/tmp".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_mock_git_status() {
        let p = generate(&test_context(), "show changed files");
        assert_eq!(p.command, "git status --short");
        assert_eq!(p.risk, "low");
        assert_eq!(p.source, "mock");
    }

    #[test]
    fn test_mock_destructive() {
        let p = generate(&test_context(), "delete old logs");
        assert_eq!(p.command, "echo \"mock: placeholder\"");
        assert!(!p.command.contains("delete"));
        assert!(!p.command.contains("old logs"));
        assert_eq!(p.user_intent, "delete old logs");
        assert!(p.explanation.contains("delete old logs"));
        assert_eq!(p.risk, "high");
        assert!(p.destructive);
        assert!(p.requires_confirmation);
    }

    #[test]
    fn test_mock_list_files() {
        let p = generate(&test_context(), "list files");
        assert_eq!(p.command, "ls -la");
        assert_eq!(p.risk, "low");
    }

    #[test]
    fn test_mock_disk_space() {
        let p = generate(&test_context(), "check disk space");
        assert_eq!(p.command, "df -h");
        assert_eq!(p.risk, "low");
    }

    #[test]
    fn test_mock_generic() {
        let p = generate(&test_context(), "something unknown");
        assert_eq!(p.command, "echo \"mock: placeholder\"");
        assert!(!p.command.contains("something unknown"));
        assert_eq!(p.user_intent, "something unknown");
        assert!(p.explanation.contains("something unknown"));
        assert_eq!(p.risk, "low");
        assert!(!p.requires_confirmation);
    }

    #[test]
    fn test_mock_does_not_interpolate_shell_metacharacters() {
        let nasty = "x\"; whoami; echo \"$(touch /tmp/owned)\n`id`";
        let generic = generate(&test_context(), nasty);
        assert_eq!(generic.command, "echo \"mock: placeholder\"");
        assert!(!generic.command.contains("whoami"));
        assert!(!generic.command.contains('\n'));
        assert_eq!(generic.risk, "low");
        assert_eq!(generic.user_intent, nasty);

        let destructive = generate(&test_context(), "remove $(whoami)");
        assert_eq!(destructive.command, "echo \"mock: placeholder\"");
        assert!(!destructive.command.contains("whoami"));
        assert_eq!(destructive.risk, "high");
        assert!(destructive.destructive);
        assert!(destructive.requires_confirmation);
        assert_eq!(destructive.user_intent, "remove $(whoami)");
    }

    #[test]
    fn test_mock_always_sets_session_id() {
        let ctx = PlanContext {
            session_id: "my-session".to_string(),
            ..Default::default()
        };
        let p = generate(&ctx, "test");
        assert_eq!(p.session_id, "my-session");
    }

    #[test]
    fn destructive_token_wins_over_earlier_keywords() {
        for intent in [
            "delete changed files",
            "git status and then delete everything",
            "list and remove files",
        ] {
            let p = generate(&test_context(), intent);
            assert_eq!(p.command, "echo \"mock: placeholder\"", "{intent}");
            assert!(!p.command.contains(intent), "{intent}");
            assert_eq!(p.risk, "high", "{intent}");
            assert!(p.destructive, "{intent}");
            assert!(p.requires_confirmation, "{intent}");
        }
    }

    #[test]
    fn token_boundary_does_not_match_substrings() {
        let undelete = generate(&test_context(), "undelete the branch");
        assert!(!undelete.destructive);
        assert_eq!(undelete.risk, "low");
        assert_eq!(undelete.command, "echo \"mock: placeholder\"");

        let showing = generate(&test_context(), "showing the directory");
        assert_eq!(showing.command, "echo \"mock: placeholder\"");
        assert_ne!(showing.command, "ls -la");
    }

    #[test]
    fn disk_tokens_win_over_list_and_show() {
        let show = generate(&test_context(), "show disk space");
        assert_eq!(show.command, "df -h");
        let list = generate(&test_context(), "list disk usage");
        assert_eq!(list.command, "df -h");
        let git = generate(&test_context(), "git status");
        assert_eq!(git.command, "git status --short");
        assert!(!git.destructive);
    }

    #[test]
    fn windows_context_uses_powershell_placeholders() {
        let ctx = PlanContext {
            os: "windows".to_string(),
            shell: "powershell".to_string(),
            ..test_context()
        };
        assert_eq!(generate(&ctx, "list files").command, "Get-ChildItem");
        assert_eq!(generate(&ctx, "show disk space").command, "Get-PSDrive");
        let removed = generate(&ctx, "remove old logs");
        assert_eq!(removed.command, "echo \"mock: placeholder\"");
        assert!(removed.destructive);
    }
}
