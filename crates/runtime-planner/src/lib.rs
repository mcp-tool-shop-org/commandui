pub mod client;
pub mod mock;
#[cfg(test)]
mod parity;
pub mod prompt;
pub mod status;
pub mod types;
pub mod validate;

pub use status::{plan_for_user, status_for, PlannerAnswer, PlannerStatus};
pub use types::{CommandProposal, OllamaConfig, PlanContext, PlanReview};

/// Ask the configured model, then decide what the user is allowed to see.
///
/// `allow_mock` keeps a labeled stand-in for tests and debug builds. A release
/// caller passes false and receives [`PlannerAnswer::Unavailable`] with no command.
pub async fn answer_for(
    config: &OllamaConfig,
    context: &PlanContext,
    user_intent: &str,
    allow_mock: bool,
) -> PlannerAnswer {
    match client::list_models(config).await {
        Err(_) => {
            let state = status::classify_connection(status::executable_on_path());
            let unavailable = status_for(state, &config.model, &config.endpoint);
            return stand_in(allow_mock, unavailable, context, user_intent);
        }
        Ok(names) => {
            if !status::model_is_present(&names, &config.model) {
                let unavailable = status_for("modelMissing", &config.model, &config.endpoint);
                return stand_in(allow_mock, unavailable, context, user_intent);
            }
        }
    }

    match client::try_ollama(config, context, user_intent).await {
        Ok(proposal) => {
            let review = client::build_review(&proposal, context);
            PlannerAnswer::Proposal {
                status: status_for("ready", &config.model, &config.endpoint),
                proposal,
                review,
            }
        }
        Err(_) => {
            let unavailable = status_for("unavailable", &config.model, &config.endpoint);
            stand_in(allow_mock, unavailable, context, user_intent)
        }
    }
}

fn stand_in(
    allow_mock: bool,
    unavailable: PlannerStatus,
    context: &PlanContext,
    user_intent: &str,
) -> PlannerAnswer {
    let proposal = mock::generate(context, user_intent);
    let review = client::build_review(&proposal, context);
    plan_for_user(allow_mock, unavailable, proposal, review)
}

/// Same decision as [`answer_for`], with the mock allowed only in a debug build.
pub async fn generate_proposal(
    config: &OllamaConfig,
    context: &PlanContext,
    user_intent: &str,
) -> PlannerAnswer {
    answer_for(config, context, user_intent, cfg!(debug_assertions)).await
}

/// Which of the four planner states is true, without drafting a command.
pub async fn probe_status(config: &OllamaConfig) -> PlannerStatus {
    match client::list_models(config).await {
        Err(_) => {
            let state = status::classify_connection(status::executable_on_path());
            status_for(state, &config.model, &config.endpoint)
        }
        Ok(names) => {
            if status::model_is_present(&names, &config.model) {
                status_for("ready", &config.model, &config.endpoint)
            } else {
                status_for("modelMissing", &config.model, &config.endpoint)
            }
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
    async fn closed_port_with_mock_allowed_is_a_labeled_stand_in() {
        let context = context();
        let intent = "list files";
        let answer = answer_for(&config(test_support::closed_endpoint(), 2), &context, intent, true).await;
        match answer {
            PlannerAnswer::Proposal { proposal, status, .. } => {
                assert_eq!(proposal.source, "mock");
                assert_eq!(proposal.source, crate::mock::generate(&context, intent).source);
                assert!(status.state == "notInstalled" || status.state == "notRunning");
                assert!(!status.headline.is_empty());
                assert!(!status.fix.is_empty());
            }
            PlannerAnswer::Unavailable { status } => {
                panic!("stand-in was hidden: {}", status.headline)
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn closed_port_without_mock_never_returns_a_command() {
        let context = context();
        let answer = answer_for(
            &config(test_support::closed_endpoint(), 2),
            &context,
            "list files",
            false,
        )
        .await;
        let dumped = serde_json::to_string(&answer).unwrap();
        match answer {
            PlannerAnswer::Unavailable { status } => {
                assert!(status.state == "notInstalled" || status.state == "notRunning");
                assert!(!status.fix.is_empty());
                assert!(!dumped.contains("\"command\""), "{dumped}");
            }
            PlannerAnswer::Proposal { proposal, .. } => {
                panic!("release returned {}", proposal.command)
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn missing_model_without_mock_names_the_model() {
        let tags = serde_json::json!({ "models": [{ "name": "other:7b" }] }).to_string();
        let endpoint = test_support::spawn_sequence(vec![("200 OK", tags)]);
        let answer = answer_for(&config(endpoint, 2), &context(), "say hello", false).await;
        let dumped = serde_json::to_string(&answer).unwrap();
        match answer {
            PlannerAnswer::Unavailable { status } => {
                assert_eq!(status.state, "modelMissing");
                assert!(status.headline.contains("test-model"), "{}", status.headline);
                assert!(status.fix.contains("ollama pull test-model"), "{}", status.fix);
                assert!(!dumped.contains("\"command\""), "{dumped}");
            }
            PlannerAnswer::Proposal { proposal, .. } => panic!("release returned {}", proposal.command),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn generate_proposal_uses_ollama_when_local_plan_is_accepted() {
        let tags = serde_json::json!({ "models": [{ "name": "test-model" }] }).to_string();
        let body = test_support::envelope(&test_support::plan_json("echo hello", "low", false));
        let endpoint = test_support::spawn_sequence(vec![("200 OK", tags), ("200 OK", body)]);
        let context = context();
        let answer = generate_proposal(&config(endpoint, 2), &context, "say hello").await;
        match answer {
            PlannerAnswer::Proposal { proposal, status, .. } => {
                assert_eq!(proposal.source, "ollama");
                assert_eq!(proposal.command, "echo hello");
                assert_eq!(proposal.session_id, context.session_id);
                assert_eq!(status.state, "ready");
            }
            PlannerAnswer::Unavailable { status } => panic!("{}", status.headline),
        }
    }
}
