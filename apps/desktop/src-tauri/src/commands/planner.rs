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

#[tauri::command]
pub fn planner_generate_plan(
    request: PlannerGeneratePlanRequest,
) -> Result<PlannerGeneratePlanResponse, ApiError> {
    if request.user_intent.is_empty() {
        return Err(ApiError::validation("user_intent cannot be empty"));
    }

    let plan_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let intent_lower = request.user_intent.to_lowercase();

    // Mock planner: infer command from intent
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
        session_id: request.session_id,
        source: "semantic".to_string(),
        user_intent: request.user_intent,
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

    Ok(PlannerGeneratePlanResponse { plan, review })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_planner_git_status() {
        let request = PlannerGeneratePlanRequest {
            session_id: "test".to_string(),
            user_intent: "show me changed files".to_string(),
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
        };
        let res = planner_generate_plan(request).unwrap();
        assert_eq!(res.plan.command, "git status --short");
    }
}
