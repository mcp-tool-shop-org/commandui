use crate::state::AppState;
use crate::types::errors::ApiError;
use commandui_runtime_planner::{self as planner, CommandProposal, PlanContext};
use serde::{Deserialize, Serialize};

// --- Desktop request/response envelopes (Tauri frontend API) ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannerGeneratePlanRequest {
    pub session_id: String,
    pub user_intent: String,
    pub context: PlannerContextPayload,
}

/// Desktop-specific context payload from the frontend.
/// Converted to shared PlanContext for the planner.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannerContextPayload {
    pub session_id: String,
    pub cwd: String,
    pub project_root: Option<String>,
    pub os: String,
    pub shell: String,
    pub recent_commands: Vec<String>,
    pub memory_items: Vec<MemoryItemPayload>,
    pub project_facts: Vec<ProjectFactPayload>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItemPayload {
    pub kind: String,
    pub key: String,
    pub value: String,
    pub confidence: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFactPayload {
    pub kind: String,
    pub label: String,
    pub value: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannerGeneratePlanResponse {
    pub plan: CommandProposal,
    pub review: planner::PlanReview,
}

// --- Tauri command: thin adapter ---

#[tauri::command]
pub async fn planner_generate_plan(
    request: PlannerGeneratePlanRequest,
    state: tauri::State<'_, AppState>,
) -> Result<PlannerGeneratePlanResponse, ApiError> {
    if request.user_intent.is_empty() {
        return Err(ApiError::validation("user_intent cannot be empty"));
    }

    // Convert desktop payload to shared PlanContext
    let context = to_plan_context(&request.context);

    // Use shared planner — same one Console uses
    let proposal = planner::generate_proposal(
        &state.ollama,
        &context,
        &request.user_intent,
    )
    .await;

    let review = planner::client::build_review(&proposal, &context);

    Ok(PlannerGeneratePlanResponse {
        plan: proposal,
        review,
    })
}

fn to_plan_context(payload: &PlannerContextPayload) -> PlanContext {
    PlanContext {
        session_id: payload.session_id.clone(),
        cwd: payload.cwd.clone(),
        project_root: payload.project_root.clone(),
        os: payload.os.clone(),
        shell: payload.shell.clone(),
        recent_commands: payload.recent_commands.clone(),
        memory_items: payload
            .memory_items
            .iter()
            .map(|m| planner::types::MemoryItemSummary {
                kind: m.kind.clone(),
                key: m.key.clone(),
                value: m.value.clone(),
                confidence: m.confidence,
            })
            .collect(),
        project_facts: payload
            .project_facts
            .iter()
            .map(|f| planner::types::ProjectFact {
                kind: f.kind.clone(),
                label: f.label.clone(),
                value: f.value.clone(),
            })
            .collect(),
    }
}

// --- Tests use the shared planner's mock directly ---

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(intent: &str) -> PlannerGeneratePlanRequest {
        PlannerGeneratePlanRequest {
            session_id: "test".to_string(),
            user_intent: intent.to_string(),
            context: PlannerContextPayload {
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
        let ctx = to_plan_context(&request.context);
        let proposal = planner::mock::generate(&ctx, &request.user_intent);
        assert_eq!(proposal.command, "git status --short");
        assert_eq!(proposal.source, "mock");
    }

    #[test]
    fn test_mock_planner_destructive() {
        let request = make_request("delete the old logs");
        let ctx = to_plan_context(&request.context);
        let proposal = planner::mock::generate(&ctx, &request.user_intent);
        assert_eq!(proposal.risk, "high");
    }

    #[test]
    fn test_mock_planner_generic() {
        let request = make_request("check disk space");
        let ctx = to_plan_context(&request.context);
        let proposal = planner::mock::generate(&ctx, &request.user_intent);
        assert_eq!(proposal.command, "df -h");
        assert_eq!(proposal.risk, "low");
    }

    #[test]
    fn test_context_conversion() {
        let request = make_request("test");
        let ctx = to_plan_context(&request.context);
        assert_eq!(ctx.session_id, "test");
        assert_eq!(ctx.cwd, "/tmp");
        assert_eq!(ctx.os, "linux");
    }
}
