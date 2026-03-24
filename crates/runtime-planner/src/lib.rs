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
