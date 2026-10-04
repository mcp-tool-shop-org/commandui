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
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub probe_only: bool,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<CommandProposal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<planner::PlanReview>,
    pub status: planner::PlannerStatus,
}

// --- Tauri command: thin adapter ---

#[tauri::command]
pub async fn planner_generate_plan(
    request: PlannerGeneratePlanRequest,
    state: tauri::State<'_, AppState>,
) -> Result<PlannerGeneratePlanResponse, ApiError> {
    if request.user_intent.is_empty() && !request.probe_only {
        return Err(ApiError::validation("user_intent cannot be empty"));
    }

    let mut config = state.ollama.clone();
    if let Some(model) = request.model.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        config.model = model.to_string();
    }
    if let Some(endpoint) = request
        .endpoint
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        config.endpoint = endpoint.trim_end_matches('/').to_string();
    }

    if request.probe_only {
        let status = planner::probe_status(&config).await;
        return Ok(PlannerGeneratePlanResponse {
            plan: None,
            review: None,
            status,
        });
    }

    let context = to_plan_context(&request.context);
    // Debug builds and `cargo test` may return a labeled stand-in. A release
    // build passes false inside answer_for via generate_proposal's cfg, but
    // this call states it: tests stay on the debug path, release does not.
    let answer = planner::answer_for(
        &config,
        &context,
        &request.user_intent,
        cfg!(debug_assertions),
    )
    .await;

    Ok(match answer {
        planner::PlannerAnswer::Proposal {
            proposal,
            review,
            status,
        } => PlannerGeneratePlanResponse {
            plan: Some(proposal),
            review: Some(review),
            status,
        },
        planner::PlannerAnswer::Unavailable { status } => PlannerGeneratePlanResponse {
            plan: None,
            review: None,
            status,
        },
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
            model: None,
            endpoint: None,
            probe_only: false,
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
