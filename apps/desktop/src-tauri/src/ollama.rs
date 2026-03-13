use crate::commands::planner::PlannerContext;
use crate::state::OllamaConfig;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

// --- Ollama API types ---

#[derive(Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    format: String,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f64,
    num_predict: i32,
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

// --- LLM response schema ---

#[derive(Deserialize, Debug)]
pub struct LlmPlanResponse {
    pub intent_summary: String,
    pub command: String,
    pub risk: String,
    pub explanation: String,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub requires_approval: bool,
    #[serde(default)]
    pub destructive: bool,
    #[serde(default)]
    pub touches_files: bool,
    #[serde(default)]
    pub touches_network: bool,
    #[serde(default)]
    pub escalates_privileges: bool,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    pub expected_output: Option<String>,
}

fn default_confidence() -> f64 {
    0.8
}

// --- Error type ---

#[derive(Debug)]
pub enum OllamaError {
    ConnectionFailed(String),
    Timeout,
    MalformedJson(String),
    ValidationFailed(String),
}

impl fmt::Display for OllamaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionFailed(msg) => write!(f, "Ollama connection failed: {msg}"),
            Self::Timeout => write!(f, "Ollama request timed out"),
            Self::MalformedJson(msg) => write!(f, "Ollama returned invalid JSON: {msg}"),
            Self::ValidationFailed(msg) => write!(f, "Plan validation failed: {msg}"),
        }
    }
}

// --- Prompt builder ---

pub fn build_planner_prompt(context: &PlannerContext, user_intent: &str) -> String {
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

    format!(
        r#"You are a shell command planner for a desktop terminal application.

## Environment
- OS: {os}
- Shell: {shell}
- Working directory: {cwd}
- Recent commands: {recent}{memory_context}

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
        intent = user_intent,
    )
}

// --- Ollama caller ---

pub async fn generate_plan(
    config: &OllamaConfig,
    context: &PlannerContext,
    user_intent: &str,
) -> Result<LlmPlanResponse, OllamaError> {
    let prompt = build_planner_prompt(context, user_intent);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.timeout_secs))
        .build()
        .map_err(|e| OllamaError::ConnectionFailed(e.to_string()))?;

    let url = format!("{}/api/generate", config.endpoint);

    let request_body = OllamaGenerateRequest {
        model: config.model.clone(),
        prompt,
        format: "json".to_string(),
        stream: false,
        options: OllamaOptions {
            temperature: 0.0,
            num_predict: 2048,
        },
    };

    let response = client
        .post(&url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                OllamaError::Timeout
            } else {
                OllamaError::ConnectionFailed(e.to_string())
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(OllamaError::ConnectionFailed(format!(
            "HTTP {status}: {body}"
        )));
    }

    let envelope: OllamaGenerateResponse = response
        .json()
        .await
        .map_err(|e| OllamaError::MalformedJson(format!("envelope parse: {e}")))?;

    let plan: LlmPlanResponse = serde_json::from_str(&envelope.response).map_err(|e| {
        let preview = if envelope.response.len() > 200 {
            format!("{}...", &envelope.response[..200])
        } else {
            envelope.response.clone()
        };
        OllamaError::MalformedJson(format!("plan parse: {e} | raw: {preview}"))
    })?;

    validate_plan(&plan)?;

    Ok(plan)
}

// --- Validation ---

pub fn validate_plan(plan: &LlmPlanResponse) -> Result<(), OllamaError> {
    if plan.command.trim().is_empty() {
        return Err(OllamaError::ValidationFailed(
            "command is empty".to_string(),
        ));
    }

    if plan.explanation.trim().is_empty() {
        return Err(OllamaError::ValidationFailed(
            "explanation is empty".to_string(),
        ));
    }

    if plan.intent_summary.trim().is_empty() {
        return Err(OllamaError::ValidationFailed(
            "intent_summary is empty".to_string(),
        ));
    }

    match plan.risk.as_str() {
        "low" | "medium" | "high" => {}
        other => {
            return Err(OllamaError::ValidationFailed(format!(
                "invalid risk level: '{other}' (expected low/medium/high)"
            )));
        }
    }

    if !(0.0..=1.0).contains(&plan.confidence) {
        return Err(OllamaError::ValidationFailed(format!(
            "confidence {} out of range 0.0-1.0",
            plan.confidence
        )));
    }

    // Consistency checks
    if plan.destructive && plan.risk == "low" {
        return Err(OllamaError::ValidationFailed(
            "destructive command cannot have low risk".to_string(),
        ));
    }

    if plan.escalates_privileges && !plan.requires_approval {
        return Err(OllamaError::ValidationFailed(
            "privilege escalation must require approval".to_string(),
        ));
    }

    Ok(())
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context() -> PlannerContext {
        PlannerContext {
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
        let ctx = make_context();
        let prompt = build_planner_prompt(&ctx, "show files");
        assert!(prompt.contains("linux"));
        assert!(prompt.contains("bash"));
        assert!(prompt.contains("/home/user/project"));
        assert!(prompt.contains("show files"));
        assert!(prompt.contains("git status, ls"));
    }

    #[test]
    fn test_prompt_no_recent_commands() {
        let mut ctx = make_context();
        ctx.recent_commands = vec![];
        let prompt = build_planner_prompt(&ctx, "test");
        assert!(prompt.contains("Recent commands: none"));
    }

    #[test]
    fn test_validate_valid_plan() {
        let plan = LlmPlanResponse {
            intent_summary: "List files".to_string(),
            command: "ls -la".to_string(),
            risk: "low".to_string(),
            explanation: "Lists all files including hidden ones".to_string(),
            assumptions: vec![],
            requires_approval: false,
            destructive: false,
            touches_files: true,
            touches_network: false,
            escalates_privileges: false,
            confidence: 0.95,
            expected_output: None,
        };
        assert!(validate_plan(&plan).is_ok());
    }

    #[test]
    fn test_validate_empty_command() {
        let plan = LlmPlanResponse {
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
        assert!(validate_plan(&plan).is_err());
    }

    #[test]
    fn test_validate_invalid_risk() {
        let plan = LlmPlanResponse {
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
        assert!(validate_plan(&plan).is_err());
    }

    #[test]
    fn test_validate_confidence_out_of_range() {
        let plan = LlmPlanResponse {
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
        assert!(validate_plan(&plan).is_err());
    }

    #[test]
    fn test_validate_destructive_low_risk_rejected() {
        let plan = LlmPlanResponse {
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
        assert!(validate_plan(&plan).is_err());
    }

    #[test]
    fn test_validate_escalation_requires_approval() {
        let plan = LlmPlanResponse {
            intent_summary: "test".to_string(),
            command: "sudo apt update".to_string(),
            risk: "high".to_string(),
            explanation: "test".to_string(),
            assumptions: vec![],
            requires_approval: false,
            destructive: false,
            touches_files: false,
            touches_network: true,
            escalates_privileges: true,
            confidence: 0.9,
            expected_output: None,
        };
        assert!(validate_plan(&plan).is_err());
    }

    #[test]
    fn test_deserialize_llm_response() {
        let json = r#"{
            "intent_summary": "List files",
            "command": "ls -la",
            "risk": "low",
            "explanation": "Shows all files",
            "assumptions": ["current directory"],
            "requires_approval": false,
            "destructive": false,
            "touches_files": true,
            "touches_network": false,
            "escalates_privileges": false,
            "confidence": 0.95,
            "expected_output": "file listing"
        }"#;
        let plan: LlmPlanResponse = serde_json::from_str(json).unwrap();
        assert_eq!(plan.command, "ls -la");
        assert_eq!(plan.risk, "low");
        assert_eq!(plan.confidence, 0.95);
    }

    #[test]
    fn test_deserialize_minimal_response() {
        // LLM might omit optional/defaulted fields
        let json = r#"{
            "intent_summary": "List files",
            "command": "ls",
            "risk": "low",
            "explanation": "Lists files"
        }"#;
        let plan: LlmPlanResponse = serde_json::from_str(json).unwrap();
        assert_eq!(plan.command, "ls");
        assert!(!plan.destructive);
        assert_eq!(plan.confidence, 0.8); // default
        assert!(plan.assumptions.is_empty());
    }
}
