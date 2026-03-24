//! Golden-path parity tests — drift gate for shared planner law.
//!
//! These fixtures lock the shared behavior that both Desktop and Console
//! rely on. If these tests fail, shared law has changed and both shells
//! must be checked.
//!
//! Convention: test names start with `parity_` to distinguish them from
//! unit tests. Fixture data uses deterministic inputs.

#[cfg(test)]
mod tests {
    use crate::mock;
    use crate::types::PlanContext;
    use crate::validate::validate_llm_response;
    use crate::types::LlmPlanResponse;
    use crate::client::llm_to_proposal;

    fn fixed_context() -> PlanContext {
        PlanContext {
            session_id: "parity-test".to_string(),
            cwd: "/home/user/project".to_string(),
            project_root: Some("/home/user/project".to_string()),
            os: "linux".to_string(),
            shell: "bash".to_string(),
            recent_commands: vec!["git status".to_string()],
            memory_items: vec![],
            project_facts: vec![],
        }
    }

    // ---- Golden-path: same intent → same proposal shape ----

    #[test]
    fn parity_mock_git_status_proposal() {
        let ctx = fixed_context();
        let p = mock::generate(&ctx, "show changed files");

        assert_eq!(p.command, "git status --short");
        assert_eq!(p.risk, "low");
        assert_eq!(p.source, "mock");
        assert_eq!(p.session_id, "parity-test");
        assert_eq!(p.cwd.as_deref(), Some("/home/user/project"));
        assert!(!p.destructive);
        assert!(!p.requires_confirmation);
        assert_eq!(p.confidence, 0.95);
    }

    #[test]
    fn parity_mock_destructive_proposal() {
        let ctx = fixed_context();
        let p = mock::generate(&ctx, "delete old logs");

        assert_eq!(p.risk, "high");
        assert!(p.destructive);
        assert!(p.requires_confirmation);
        assert_eq!(p.source, "mock");
        assert_eq!(p.confidence, 0.95);
    }

    #[test]
    fn parity_mock_list_files_proposal() {
        let ctx = fixed_context();
        let p = mock::generate(&ctx, "list all files");

        assert_eq!(p.command, "ls -la");
        assert_eq!(p.risk, "low");
        assert!(!p.destructive);
    }

    #[test]
    fn parity_mock_disk_space_proposal() {
        let ctx = fixed_context();
        let p = mock::generate(&ctx, "check disk space");

        assert_eq!(p.command, "df -h");
        assert_eq!(p.risk, "low");
    }

    // ---- Golden-path: same LLM output → same proposal conversion ----

    fn fixed_llm_response() -> LlmPlanResponse {
        LlmPlanResponse {
            intent_summary: "List files in current directory".to_string(),
            command: "ls -la".to_string(),
            risk: "low".to_string(),
            explanation: "Lists all files including hidden ones with details".to_string(),
            assumptions: vec!["User wants the current directory".to_string()],
            requires_approval: false,
            destructive: false,
            touches_files: true,
            touches_network: false,
            escalates_privileges: false,
            confidence: 0.95,
            expected_output: Some("file listing with permissions".to_string()),
        }
    }

    #[test]
    fn parity_llm_to_proposal_preserves_command() {
        let ctx = fixed_context();
        let llm = fixed_llm_response();
        let p = llm_to_proposal(&llm, &ctx, "list files", "ollama");

        assert_eq!(p.command, "ls -la");
        assert_eq!(p.source, "ollama");
        assert_eq!(p.risk, "low");
        assert_eq!(p.confidence, 0.95);
        assert_eq!(p.user_intent, "List files in current directory");
        assert!(p.touches_files);
        assert!(!p.destructive);
        assert!(!p.requires_confirmation);
        assert_eq!(p.session_id, "parity-test");
        assert_eq!(p.assumptions, vec!["User wants the current directory"]);
    }

    #[test]
    fn parity_llm_destructive_proposal() {
        let ctx = fixed_context();
        let llm = LlmPlanResponse {
            intent_summary: "Delete all temporary files".to_string(),
            command: "rm -rf /tmp/*".to_string(),
            risk: "high".to_string(),
            explanation: "Removes all files in /tmp".to_string(),
            assumptions: vec![],
            requires_approval: true,
            destructive: true,
            touches_files: true,
            touches_network: false,
            escalates_privileges: false,
            confidence: 0.8,
            expected_output: None,
        };
        let p = llm_to_proposal(&llm, &ctx, "delete temp files", "ollama");

        assert!(p.destructive);
        assert!(p.requires_confirmation);
        assert_eq!(p.risk, "high");
        assert!(p.touches_files);
    }

    // ---- Golden-path: same malformed input → same rejection ----

    #[test]
    fn parity_reject_empty_command() {
        let llm = LlmPlanResponse {
            intent_summary: "test".to_string(),
            command: "".to_string(),
            risk: "low".to_string(),
            explanation: "test".to_string(),
            assumptions: vec![],
            requires_approval: false,
            destructive: false,
            touches_files: false,
            touches_network: false,
            escalates_privileges: false,
            confidence: 0.5,
            expected_output: None,
        };
        let result = validate_llm_response(&llm);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("command"));
    }

    #[test]
    fn parity_reject_destructive_low_risk() {
        let llm = LlmPlanResponse {
            intent_summary: "test".to_string(),
            command: "rm -rf /".to_string(),
            risk: "low".to_string(),
            explanation: "test".to_string(),
            assumptions: vec![],
            requires_approval: false,
            destructive: true,
            touches_files: true,
            touches_network: false,
            escalates_privileges: false,
            confidence: 0.9,
            expected_output: None,
        };
        let result = validate_llm_response(&llm);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("destructive"));
    }

    #[test]
    fn parity_reject_escalation_without_approval() {
        let llm = LlmPlanResponse {
            intent_summary: "update system".to_string(),
            command: "sudo apt update".to_string(),
            risk: "high".to_string(),
            explanation: "Updates packages".to_string(),
            assumptions: vec![],
            requires_approval: false,
            destructive: false,
            touches_files: false,
            touches_network: true,
            escalates_privileges: true,
            confidence: 0.9,
            expected_output: None,
        };
        let result = validate_llm_response(&llm);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("privilege"));
    }

    #[test]
    fn parity_reject_invalid_risk_level() {
        let llm = LlmPlanResponse {
            intent_summary: "test".to_string(),
            command: "ls".to_string(),
            risk: "extreme".to_string(),
            explanation: "test".to_string(),
            assumptions: vec![],
            requires_approval: false,
            destructive: false,
            touches_files: false,
            touches_network: false,
            escalates_privileges: false,
            confidence: 0.5,
            expected_output: None,
        };
        let result = validate_llm_response(&llm);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("risk"));
    }

    #[test]
    fn parity_reject_confidence_out_of_range() {
        let llm = LlmPlanResponse {
            intent_summary: "test".to_string(),
            command: "ls".to_string(),
            risk: "low".to_string(),
            explanation: "test".to_string(),
            assumptions: vec![],
            requires_approval: false,
            destructive: false,
            touches_files: false,
            touches_network: false,
            escalates_privileges: false,
            confidence: 1.5,
            expected_output: None,
        };
        let result = validate_llm_response(&llm);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("confidence"));
    }

    // ---- Golden-path: review metadata derivation ----

    #[test]
    fn parity_review_safety_flags_from_destructive() {
        let ctx = fixed_context();
        let p = mock::generate(&ctx, "delete everything");
        let review = crate::client::build_review(&p, &ctx);

        assert!(review.safety_flags.contains(&"DESTRUCTIVE_OPERATION".to_string()));
    }

    #[test]
    fn parity_review_context_includes_cwd() {
        let ctx = fixed_context();
        let p = mock::generate(&ctx, "list files");
        let review = crate::client::build_review(&p, &ctx);

        assert!(review.retrieved_context.iter().any(|c| c.contains("/home/user/project")));
    }

    #[test]
    fn parity_review_context_includes_project_root() {
        let ctx = fixed_context();
        let p = mock::generate(&ctx, "list files");
        let review = crate::client::build_review(&p, &ctx);

        assert!(review.retrieved_context.iter().any(|c| c.contains("projectRoot")));
    }
}
