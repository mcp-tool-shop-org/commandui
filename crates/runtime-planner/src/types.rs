//! Shared planner types — the canonical proposal truth.
//!
//! These types define what a plan IS across all shells.
//! Desktop and Console both consume these; neither defines its own.

use serde::{Deserialize, Serialize};

/// Ollama connection configuration.
#[derive(Clone)]
pub struct OllamaConfig {
    pub endpoint: String,
    pub model: String,
    /// How long one plan request may take. A model that is not loaded yet
    /// has to load first, which can take most of a minute on a modest
    /// machine. The desktop's plan limit (PLAN_TIMEOUT_MS) waits longer.
    pub timeout_secs: u64,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:11434".to_string(),
            model: "qwen2.5:14b".to_string(),
            timeout_secs: 90,
        }
    }
}

/// Context provided to the planner for plan generation.
/// Shells populate this from their own state; the planner consumes it neutrally.
#[derive(Clone, Default)]
pub struct PlanContext {
    pub session_id: String,
    pub cwd: String,
    pub project_root: Option<String>,
    pub os: String,
    pub shell: String,
    pub recent_commands: Vec<String>,
    pub memory_items: Vec<MemoryItemSummary>,
    pub project_facts: Vec<ProjectFact>,
}

/// Summary of a memory item for planner context.
#[derive(Clone, Deserialize, Serialize)]
pub struct MemoryItemSummary {
    pub kind: String,
    pub key: String,
    pub value: String,
    pub confidence: f64,
}

/// A project fact for planner context (e.g., workflows, conventions).
#[derive(Clone, Deserialize, Serialize)]
pub struct ProjectFact {
    pub kind: String,
    pub label: String,
    pub value: String,
}

/// A command proposal — the canonical result of plan generation.
/// This is what both shells render in their review surfaces.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandProposal {
    pub id: String,
    pub session_id: String,
    pub source: String,
    pub user_intent: String,
    pub command: String,
    pub cwd: Option<String>,
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

/// Review metadata for a proposal — safety/ambiguity analysis.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanReview {
    pub plan_id: String,
    pub safety_flags: Vec<String>,
    pub ambiguity_flags: Vec<String>,
    pub memory_used: Vec<String>,
    pub retrieved_context: Vec<String>,
}

/// The result of plan generation: proposal + review metadata.
#[derive(Debug, Clone)]
pub struct PlanResult {
    pub proposal: CommandProposal,
    pub review: PlanReview,
}

/// Raw LLM response schema — what Ollama returns.
/// Internal to the planner crate; shells never see this.
#[derive(Deserialize, Debug)]
pub(crate) struct LlmPlanResponse {
    pub intent_summary: String,
    pub command: String,
    pub risk: String,
    pub explanation: String,
    #[serde(default)]
    pub assumptions: Vec<String>,
    pub requires_approval: bool,
    pub destructive: bool,
    #[serde(default)]
    pub touches_files: bool,
    #[serde(default)]
    pub touches_network: bool,
    pub escalates_privileges: bool,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default)]
    pub expected_output: Option<String>,
}

fn default_confidence() -> f64 {
    0.8
}

#[cfg(test)]
mod default_tests {
    use super::OllamaConfig;

    #[test]
    fn a_plan_request_waits_for_a_cold_model() {
        // Loading qwen2.5:14b from disk took longer than 15 s on a fast machine.
        assert_eq!(OllamaConfig::default().timeout_secs, 90);
    }
}
