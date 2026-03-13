use crate::ollama::{self, LlmPlanResponse};
use crate::state::AppState;
use crate::types::errors::ApiError;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannerGeneratePlanRequest {
    pub session_id: String,
    pub user_intent: String,
    pub context: PlannerContext,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannerContext {
    pub session_id: String,
    pub cwd: String,
    pub project_root: Option<String>,
    pub os: String,
    pub shell: String,
    pub recent_commands: Vec<String>,
    pub memory_items: Vec<MemoryItemSummary>,
    pub project_facts: Vec<ProjectFact>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItemSummary {
    pub kind: String,
    pub key: String,
    pub value: String,
    pub confidence: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFact {
    pub kind: String,
    pub label: String,
    pub value: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannerGeneratePlanResponse {
    pub plan: CommandPlanPayload,
    pub review: PlanReviewPayload,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandPlanPayload {
    pub id: String,
    pub session_id: String,
    pub source: String,
    pub user_intent: String,
    pub command: String,
    pub cwd: Option<String>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub explanation: String,
    pub assumptions: Vec<String>,
    pub confidence: f64,
    pub risk: String,
    pub destructive: bool,
    pub requires_confirmation: bool,
    pub touches_files: bool,
    pub touches_network: bool,
    pub escalates_privileges: bool,
    pub expected_output: Option<String>,
    pub generated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanReviewPayload {
    pub plan_id: String,
    pub ambiguity_flags: Vec<String>,
    pub safety_flags: Vec<String>,
    pub memory_used: Vec<String>,
    pub retrieved_context: Vec<String>,
}

// --- Main command: try Ollama, fallback to mock ---

#[tauri::command]
pub async fn planner_generate_plan(
    request: PlannerGeneratePlanRequest,
    state: tauri::State<'_, AppState>,
) -> Result<PlannerGeneratePlanResponse, ApiError> {
    if request.user_intent.is_empty() {
        return Err(ApiError::validation("user_intent cannot be empty"));
    }

    // Try Ollama first
    match ollama::generate_plan(&state.ollama, &request.context, &request.user_intent).await {
        Ok(llm_plan) => {
            eprintln!("[planner] Ollama returned plan for: {}", request.user_intent);
            Ok(build_response_from_llm(&request, llm_plan))
        }
        Err(e) => {
            eprintln!("[planner] Ollama failed, using mock fallback: {e}");
            Ok(mock_plan(&request))
        }
    }
}

// --- Convert LLM response to our contract ---

fn build_response_from_llm(
    request: &PlannerGeneratePlanRequest,
    llm: LlmPlanResponse,
) -> PlannerGeneratePlanResponse {
    let plan_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let mut safety_flags = vec![];
    if llm.destructive {
        safety_flags.push("DESTRUCTIVE_OPERATION".to_string());
    }
    if llm.escalates_privileges {
        safety_flags.push("PRIVILEGE_ESCALATION".to_string());
    }
    if llm.touches_network {
        safety_flags.push("NETWORK_ACCESS".to_string());
    }

    let mut retrieved_context = vec![];
    if !request.context.cwd.is_empty() {
        retrieved_context.push(format!("cwd: {}", request.context.cwd));
    }
    if let Some(ref root) = request.context.project_root {
        retrieved_context.push(format!("projectRoot: {root}"));
    }

    let plan = CommandPlanPayload {
        id: plan_id.clone(),
        session_id: request.session_id.clone(),
        source: "ollama".to_string(),
        user_intent: llm.intent_summary,
        command: llm.command,
        cwd: Some(request.context.cwd.clone()),
        env: None,
        explanation: llm.explanation,
        assumptions: llm.assumptions,
        confidence: llm.confidence,
        risk: llm.risk,
        destructive: llm.destructive,
        requires_confirmation: llm.requires_approval,
        touches_files: llm.touches_files,
        touches_network: llm.touches_network,
        escalates_privileges: llm.escalates_privileges,
        expected_output: llm.expected_output,
        generated_at: now,
    };

    let review = PlanReviewPayload {
        plan_id,
        ambiguity_flags: vec![],
        safety_flags,
        memory_used: request
            .context
            .memory_items
            .iter()
            .map(|m| format!("{}:{}", m.kind, m.key))
            .collect(),
        retrieved_context,
    };

    PlannerGeneratePlanResponse { plan, review }
}

// --- Mock fallback ---

pub fn mock_plan(request: &PlannerGeneratePlanRequest) -> PlannerGeneratePlanResponse {
    let plan_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let intent_lower = request.user_intent.to_lowercase();

    let (command, explanation, risk) = if intent_lower.contains("changed files")
        || intent_lower.contains("git status")
    {
        (
            "git status --short".to_string(),
            "Shows modified and untracked files in the working tree using short format.".to_string(),
            "low".to_string(),
        )
    } else if intent_lower.contains("delete") || intent_lower.contains("remove") {
        (
            format!("echo \"planner stub: {}\"", request.user_intent),
            "Stub: destructive intent detected, requires review.".to_string(),
            "high".to_string(),
        )
    } else {
        (
            format!("echo \"planner stub: {}\"", request.user_intent),
            format!(
                "Stub planner response for intent: {}",
                request.user_intent
            ),
            "low".to_string(),
        )
    };

    let plan = CommandPlanPayload {
        id: plan_id.clone(),
        session_id: request.session_id.clone(),
        source: "mock".to_string(),
        user_intent: request.user_intent.clone(),
        command,
        cwd: Some(request.context.cwd.clone()),
        env: None,
        explanation,
        assumptions: vec![],
        confidence: 0.96,
        risk,
        destructive: false,
        requires_confirmation: false,
        touches_files: false,
        touches_network: false,
        escalates_privileges: false,
        expected_output: None,
        generated_at: now,
    };

    let mut retrieved_context = vec![];
    if !request.context.cwd.is_empty() {
        retrieved_context.push(format!("cwd: {}", request.context.cwd));
    }
    if let Some(ref root) = request.context.project_root {
        retrieved_context.push(format!("projectRoot: {root}"));
    }

    let review = PlanReviewPayload {
        plan_id,
        ambiguity_flags: vec![],
        safety_flags: vec![],
        memory_used: request
            .context
            .memory_items
            .iter()
            .map(|m| format!("{}:{}", m.kind, m.key))
            .collect(),
        retrieved_context,
    };

    PlannerGeneratePlanResponse { plan, review }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(intent: &str) -> PlannerGeneratePlanRequest {
        PlannerGeneratePlanRequest {
            session_id: "test".to_string(),
            user_intent: intent.to_string(),
            context: PlannerContext {
                session_id: "test".to_string(),
                cwd: "/tmp".to_string(),
                project_root: None,
                os: "linux".to_string(),
                shell: "bash".to_string(),
                recent_commands: vec![],
                memory_items: vec![],
                project_facts: vec![],
            },
        }
    }

    #[test]
    fn test_mock_planner_git_status() {
        let request = make_request("show me changed files");
        let res = mock_plan(&request);
        assert_eq!(res.plan.command, "git status --short");
        assert_eq!(res.plan.source, "mock");
    }

    #[test]
    fn test_mock_planner_destructive() {
        let request = make_request("delete the old logs");
        let res = mock_plan(&request);
        assert_eq!(res.plan.risk, "high");
    }

    #[test]
    fn test_mock_planner_generic() {
        let request = make_request("check disk space");
        let res = mock_plan(&request);
        assert!(res.plan.command.contains("planner stub"));
        assert_eq!(res.plan.risk, "low");
    }

    #[test]
    fn test_llm_response_conversion() {
        let request = make_request("list files");
        let llm = LlmPlanResponse {
            intent_summary: "List all files in current directory".to_string(),
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
        };

        let res = build_response_from_llm(&request, llm);
        assert_eq!(res.plan.command, "ls -la");
        assert_eq!(res.plan.source, "ollama");
        assert_eq!(res.plan.risk, "low");
        assert_eq!(res.plan.confidence, 0.95);
        assert!(res.plan.touches_files);
        assert!(!res.plan.requires_confirmation);
        assert_eq!(res.plan.assumptions.len(), 1);
        assert!(res.review.safety_flags.is_empty());
    }

    #[test]
    fn test_llm_response_safety_flags() {
        let request = make_request("delete everything");
        let llm = LlmPlanResponse {
            intent_summary: "Delete all files".to_string(),
            command: "rm -rf ./*".to_string(),
            risk: "high".to_string(),
            explanation: "Deletes all files in current directory".to_string(),
            assumptions: vec![],
            requires_approval: true,
            destructive: true,
            touches_files: true,
            touches_network: false,
            escalates_privileges: false,
            confidence: 0.7,
            expected_output: None,
        };

        let res = build_response_from_llm(&request, llm);
        assert!(res.review.safety_flags.contains(&"DESTRUCTIVE_OPERATION".to_string()));
        assert!(res.plan.requires_confirmation);
        assert!(res.plan.destructive);
    }
}
