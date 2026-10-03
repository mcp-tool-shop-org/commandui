pub mod client;
pub mod mock;
#[cfg(test)]
mod parity;
pub mod prompt;
pub mod types;
pub mod validate;

pub use types::{CommandProposal, OllamaConfig, PlanContext, PlanReview};

/// Generate a command proposal for the given intent.
/// Tries Ollama first, falls back to mock on any failure.
pub async fn generate_proposal(
    config: &OllamaConfig,
    context: &PlanContext,
    user_intent: &str,
) -> CommandProposal {
    match client::try_ollama(config, context, user_intent).await {
        Ok(proposal) => proposal,
        Err(e) => {
            eprintln!("[planner] Ollama failed, using mock: {e}");
            mock::generate(context, user_intent)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::test_support;

    fn context() -> PlanContext {
        PlanContext {
            session_id: "sess-from-context".to_string(),
            cwd: "work".to_string(),
            ..PlanContext::default()
        }
    }

    fn config(endpoint: String, timeout_secs: u64) -> OllamaConfig {
        OllamaConfig {
            endpoint,
            model: "test-model".to_string(),
            timeout_secs,
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn generate_proposal_falls_back_to_mock_on_closed_port() {
        let context = context();
        let intent = "list files";
        let proposal = generate_proposal(&config(test_support::closed_endpoint(), 2), &context, intent).await;
        assert_eq!(
            proposal.source,
            crate::mock::generate(&context, intent).source
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn generate_proposal_uses_ollama_when_local_plan_is_accepted() {
        let body = test_support::envelope(&test_support::plan_json("echo hello", "low", false));
        let endpoint = test_support::spawn_body("200 OK", &body);
        let context = context();
        let proposal = generate_proposal(&config(endpoint, 2), &context, "say hello").await;
        assert_eq!(proposal.source, "ollama");
        assert_eq!(proposal.command, "echo hello");
        assert_eq!(proposal.session_id, context.session_id);
    }
}
