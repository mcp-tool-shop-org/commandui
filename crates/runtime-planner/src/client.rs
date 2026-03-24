//! Ollama HTTP client — shared across all shells.
//!
//! Calls Ollama's /api/generate endpoint, parses the LLM response,
//! validates it, and converts to CommandProposal + PlanReview.

use crate::prompt::build_planner_prompt;
use crate::types::{
    CommandProposal, LlmPlanResponse, OllamaConfig, PlanContext, PlanReview,
};
use crate::validate::validate_llm_response;
use serde::{Deserialize, Serialize};
use std::time::Duration;

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

/// Try to generate a proposal via Ollama.
/// Returns the full PlanResult on success, or an error string on failure.
pub(crate) async fn try_ollama(
    config: &OllamaConfig,
    context: &PlanContext,
    user_intent: &str,
) -> Result<CommandProposal, String> {
    let prompt = build_planner_prompt(context, user_intent);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.timeout_secs))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

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
                "timeout".to_string()
            } else {
                format!("connection: {e}")
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("HTTP {status}: {body}"));
    }

    let envelope: OllamaGenerateResponse = response
        .json()
        .await
        .map_err(|e| format!("envelope parse: {e}"))?;

    let llm: LlmPlanResponse = serde_json::from_str(&envelope.response).map_err(|e| {
        let preview = if envelope.response.len() > 200 {
            format!("{}...", &envelope.response[..200])
        } else {
            envelope.response.clone()
        };
        format!("plan parse: {e} | raw: {preview}")
    })?;

    // Validate before converting — fail closed
    validate_llm_response(&llm)?;

    Ok(llm_to_proposal(&llm, context, user_intent, "ollama"))
}

/// Convert a validated LLM response to a CommandProposal.
pub(crate) fn llm_to_proposal(
    llm: &LlmPlanResponse,
    context: &PlanContext,
    _user_intent: &str,
    source: &str,
) -> CommandProposal {
    let plan_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    CommandProposal {
        id: plan_id,
        session_id: context.session_id.clone(),
        source: source.to_string(),
        user_intent: llm.intent_summary.clone(),
        command: llm.command.clone(),
        cwd: Some(context.cwd.clone()),
        explanation: llm.explanation.clone(),
        assumptions: llm.assumptions.clone(),
        confidence: llm.confidence,
        risk: llm.risk.clone(),
        destructive: llm.destructive,
        requires_confirmation: llm.requires_approval,
        touches_files: llm.touches_files,
        touches_network: llm.touches_network,
        escalates_privileges: llm.escalates_privileges,
        expected_output: llm.expected_output.clone(),
        generated_at: now,
    }
}

/// Build a PlanReview from a proposal and context.
pub fn build_review(proposal: &CommandProposal, context: &PlanContext) -> PlanReview {
    let mut safety_flags = vec![];
    if proposal.destructive {
        safety_flags.push("DESTRUCTIVE_OPERATION".to_string());
    }
    if proposal.escalates_privileges {
        safety_flags.push("PRIVILEGE_ESCALATION".to_string());
    }
    if proposal.touches_network {
        safety_flags.push("NETWORK_ACCESS".to_string());
    }

    let mut retrieved_context = vec![];
    if !context.cwd.is_empty() {
        retrieved_context.push(format!("cwd: {}", context.cwd));
    }
    if let Some(ref root) = context.project_root {
        retrieved_context.push(format!("projectRoot: {root}"));
    }
    for fact in &context.project_facts {
        retrieved_context.push(format!("{}:{}", fact.kind, fact.label));
    }

    PlanReview {
        plan_id: proposal.id.clone(),
        safety_flags,
        ambiguity_flags: vec![],
        memory_used: context
            .memory_items
            .iter()
            .map(|m| format!("{}:{}", m.kind, m.key))
            .collect(),
        retrieved_context,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_to_proposal() {
        let llm = LlmPlanResponse {
            intent_summary: "List files".to_string(),
            command: "ls -la".to_string(),
            risk: "low".to_string(),
            explanation: "Lists all files".to_string(),
            assumptions: vec!["current directory".to_string()],
            requires_approval: false,
            destructive: false,
            touches_files: true,
            touches_network: false,
            escalates_privileges: false,
            confidence: 0.95,
            expected_output: Some("file listing".to_string()),
        };

        let ctx = PlanContext {
            session_id: "s1".to_string(),
            cwd: "/tmp".to_string(),
            ..Default::default()
        };

        let proposal = llm_to_proposal(&llm, &ctx, "list files", "ollama");
        assert_eq!(proposal.command, "ls -la");
        assert_eq!(proposal.source, "ollama");
        assert_eq!(proposal.risk, "low");
        assert_eq!(proposal.confidence, 0.95);
        assert!(proposal.touches_files);
        assert!(!proposal.requires_confirmation);
        assert_eq!(proposal.session_id, "s1");
    }

    #[test]
    fn test_build_review_safety_flags() {
        let proposal = CommandProposal {
            id: "p1".to_string(),
            session_id: "s1".to_string(),
            source: "ollama".to_string(),
            user_intent: "delete".to_string(),
            command: "rm -rf ./*".to_string(),
            cwd: Some("/tmp".to_string()),
            explanation: "Deletes files".to_string(),
            assumptions: vec![],
            confidence: 0.7,
            risk: "high".to_string(),
            destructive: true,
            requires_confirmation: true,
            touches_files: true,
            touches_network: false,
            escalates_privileges: true,
            expected_output: None,
            generated_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let ctx = PlanContext {
            cwd: "/tmp".to_string(),
            ..Default::default()
        };

        let review = build_review(&proposal, &ctx);
        assert!(review.safety_flags.contains(&"DESTRUCTIVE_OPERATION".to_string()));
        assert!(review.safety_flags.contains(&"PRIVILEGE_ESCALATION".to_string()));
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
        assert_eq!(plan.confidence, 0.95);
    }

    #[test]
    fn test_deserialize_minimal_response() {
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
    }
}
