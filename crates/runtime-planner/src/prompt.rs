//! Prompt builder — shared template for Ollama plan generation.
//!
//! Both shells use the same prompt shape so Ollama produces
//! consistent proposals regardless of which shell asked.

use crate::types::PlanContext;

pub(crate) fn build_planner_prompt(context: &PlanContext, user_intent: &str) -> String {
    let recent = if context.recent_commands.is_empty() {
        "none".to_string()
    } else {
        context.recent_commands.join(", ")
    };

    let memory_context = if context.memory_items.is_empty() {
        String::new()
    } else {
        let items: Vec<String> = context
            .memory_items
            .iter()
            .map(|m| format!("- {}: {} = {}", m.kind, m.key, m.value))
            .collect();
        format!("\n## Known preferences\n{}", items.join("\n"))
    };

    let workflow_context = {
        let wf_facts: Vec<&crate::types::ProjectFact> = context
            .project_facts
            .iter()
            .filter(|f| f.kind == "workflow")
            .collect();
        if wf_facts.is_empty() {
            String::new()
        } else {
            let items: Vec<String> = wf_facts
                .iter()
                .map(|f| format!("- \"{}\": {}", f.label, f.value))
                .collect();
            format!(
                "\n## Known workflows\n{}\nIf the user's intent matches a known workflow, suggest using it by name and output its exact commands.",
                items.join("\n")
            )
        }
    };

    format!(
        r#"You are a shell command planner for a terminal application.

## Environment
- OS: {os}
- Shell: {shell}
- Working directory: {cwd}
- Recent commands: {recent}{memory_context}{workflow_context}

## Task
The user wants: "{intent}"

## Rules
1. Produce exactly ONE shell command that accomplishes the user's intent.
2. The command must be valid for the specified OS and shell.
3. Classify risk accurately:
   - "low": read-only operations (ls, cat, git status, echo, pwd, which, where)
   - "medium": file modifications, installs, config changes, git commit/push
   - "high": deletions (rm, del), system changes, privilege escalation, format/wipe ops
4. Set destructive=true if the command deletes files/data or is irreversible.
5. Set requires_approval=true if risk is "medium" or "high".
6. Set touches_files=true if the command reads or writes files.
7. Set touches_network=true if the command makes network requests.
8. Set escalates_privileges=true if the command uses sudo, runas, or similar.
9. Confidence should reflect how certain you are the command is correct (0.0-1.0).
10. If the user's intent closely matches a known workflow, prefer that workflow's commands over inventing new ones.

## Response format
Return ONLY a JSON object with these exact fields:
{{
  "intent_summary": "brief restatement of what the user wants",
  "command": "the shell command to execute",
  "risk": "low or medium or high",
  "explanation": "what this command does and why",
  "assumptions": ["things you assumed about the intent"],
  "requires_approval": true or false,
  "destructive": true or false,
  "touches_files": true or false,
  "touches_network": true or false,
  "escalates_privileges": true or false,
  "confidence": 0.0 to 1.0,
  "expected_output": "what the output should look like, or null"
}}"#,
        os = context.os,
        shell = context.shell,
        cwd = context.cwd,
        recent = recent,
        memory_context = memory_context,
        workflow_context = workflow_context,
        intent = user_intent,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MemoryItemSummary, PlanContext, ProjectFact};

    fn test_context() -> PlanContext {
        PlanContext {
            session_id: "test".to_string(),
            cwd: "/home/user/project".to_string(),
            project_root: None,
            os: "linux".to_string(),
            shell: "bash".to_string(),
            recent_commands: vec!["git status".to_string(), "ls".to_string()],
            memory_items: vec![],
            project_facts: vec![],
        }
    }

    #[test]
    fn test_prompt_contains_context() {
        let ctx = test_context();
        let prompt = build_planner_prompt(&ctx, "show files");
        assert!(prompt.contains("linux"));
        assert!(prompt.contains("bash"));
        assert!(prompt.contains("/home/user/project"));
        assert!(prompt.contains("show files"));
        assert!(prompt.contains("git status, ls"));
    }

    #[test]
    fn test_prompt_no_recent_commands() {
        let mut ctx = test_context();
        ctx.recent_commands = vec![];
        let prompt = build_planner_prompt(&ctx, "test");
        assert!(prompt.contains("Recent commands: none"));
    }

    #[test]
    fn test_prompt_with_memory_items() {
        let mut ctx = test_context();
        ctx.memory_items = vec![MemoryItemSummary {
            kind: "preference".to_string(),
            key: "editor".to_string(),
            value: "vim".to_string(),
            confidence: 0.9,
        }];
        let prompt = build_planner_prompt(&ctx, "test");
        assert!(prompt.contains("Known preferences"));
        assert!(prompt.contains("preference: editor = vim"));
    }

    #[test]
    fn test_prompt_with_workflow_facts() {
        let mut ctx = test_context();
        ctx.project_facts = vec![ProjectFact {
            kind: "workflow".to_string(),
            label: "build".to_string(),
            value: "cargo build --release".to_string(),
        }];
        let prompt = build_planner_prompt(&ctx, "build the project");
        assert!(prompt.contains("Known workflows"));
        assert!(prompt.contains("cargo build --release"));
    }
}
