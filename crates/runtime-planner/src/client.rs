//! Ollama HTTP client — shared across all shells.
//!
//! Calls Ollama's /api/generate endpoint, parses the LLM response,
//! validates it, and converts to CommandProposal + PlanReview.

use crate::prompt::build_planner_prompt;
use crate::types::{
    CommandProposal, LlmPlanResponse, OllamaConfig, PlanContext, PlanReview,
};
use crate::validate::{accept_llm_plan, command_floor};
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

    let mut llm: LlmPlanResponse = serde_json::from_str(&envelope.response).map_err(|e| {
        format!("plan parse: {e} | raw: {}", preview_response(&envelope.response))
    })?;

    // Floor then validate — fail closed. Flags the model omitted cannot pass.
    accept_llm_plan(&mut llm)?;

    Ok(llm_to_proposal(&llm, context, user_intent, "ollama"))
}

#[derive(Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    #[serde(default)]
    name: String,
    #[serde(default)]
    model: String,
}

/// Local model list. An error means the endpoint could not be asked.
/// This only calls the configured endpoint. It never calls a cloud host of its own.
pub(crate) async fn list_models(config: &OllamaConfig) -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.timeout_secs.max(1)))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;
    let endpoint = config.endpoint.trim().trim_end_matches('/');
    let url = format!("{endpoint}/api/tags");
    let response = client.get(&url).send().await.map_err(|e| {
        if e.is_timeout() {
            "timeout".to_string()
        } else {
            format!("connection: {e}")
        }
    })?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let tags: TagsResponse = response
        .json()
        .await
        .map_err(|e| format!("tags parse: {e}"))?;
    let mut names = Vec::new();
    for model in tags.models {
        if !model.name.is_empty() {
            names.push(model.name);
        }
        if !model.model.is_empty() {
            names.push(model.model);
        }
    }
    Ok(names)
}

/// Truncate to at most 200 bytes on a char boundary (multibyte-safe).
pub(crate) fn preview_response(response: &str) -> String {
    if response.len() <= 200 {
        return response.to_string();
    }
    let mut end = 200;
    while !response.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &response[..end])
}

/// Convert a validated LLM response to a CommandProposal.
pub(crate) fn llm_to_proposal(
    llm: &LlmPlanResponse,
    context: &PlanContext,
    user_intent: &str,
    source: &str,
) -> CommandProposal {
    let plan_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    CommandProposal {
        id: plan_id,
        session_id: context.session_id.clone(),
        source: source.to_string(),
        user_intent: user_intent.to_string(),
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
    let floor = command_floor(&proposal.command);
    let mut safety_flags = vec![];
    if proposal.destructive || floor.destructive {
        safety_flags.push("DESTRUCTIVE_OPERATION".to_string());
    }
    if proposal.escalates_privileges || floor.escalates_privileges {
        safety_flags.push("PRIVILEGE_ESCALATION".to_string());
    }
    if floor.high_risk {
        safety_flags.push("HIGH_RISK_COMMAND".to_string());
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
        assert_eq!(proposal.user_intent, "list files");
        assert_ne!(proposal.user_intent, llm.intent_summary);
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
        assert_eq!(
            review
                .safety_flags
                .iter()
                .filter(|f| *f == "DESTRUCTIVE_OPERATION")
                .count(),
            1
        );
    }

    #[test]
    fn test_build_review_flags_command_text_when_model_booleans_are_false() {
        let proposal = CommandProposal {
            id: "p1".to_string(),
            session_id: "s1".to_string(),
            source: "ollama".to_string(),
            user_intent: "clean up".to_string(),
            command: "sudo rm -rf /tmp/old".to_string(),
            cwd: Some("/tmp".to_string()),
            explanation: "Cleans a directory".to_string(),
            assumptions: vec![],
            confidence: 0.7,
            risk: "low".to_string(),
            destructive: false,
            requires_confirmation: false,
            touches_files: false,
            touches_network: false,
            escalates_privileges: false,
            expected_output: None,
            generated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let ctx = PlanContext::default();
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
    fn test_deserialize_minimal_response_requires_safety_booleans() {
        let json = r#"{
            "intent_summary": "List files",
            "command": "ls",
            "risk": "low",
            "explanation": "Lists files"
        }"#;
        let err = serde_json::from_str::<LlmPlanResponse>(json).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("requires_approval")
                || msg.contains("destructive")
                || msg.contains("escalates_privileges"),
            "{msg}"
        );
    }

    #[test]
    fn test_deserialize_omitted_touches_still_parses() {
        let json = r#"{
            "intent_summary": "List files",
            "command": "ls",
            "risk": "low",
            "explanation": "Lists files",
            "requires_approval": false,
            "destructive": false,
            "escalates_privileges": false
        }"#;
        let plan: LlmPlanResponse = serde_json::from_str(json).unwrap();
        assert!(!plan.touches_files);
        assert!(!plan.touches_network);
        assert!(plan.expected_output.is_none());
        assert_eq!(plan.confidence, 0.8);
    }

    fn config(endpoint: String, timeout_secs: u64) -> OllamaConfig {
        OllamaConfig {
            endpoint,
            model: "test-model".to_string(),
            timeout_secs,
        }
    }

    fn context() -> PlanContext {
        PlanContext {
            session_id: "sess-from-context".to_string(),
            cwd: "work".to_string(),
            ..PlanContext::default()
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn try_ollama_connection_error_on_closed_port() {
        let closed = try_ollama(
            &config(test_support::closed_endpoint(), 2),
            &context(),
            "list files",
        )
        .await
        .expect_err("closed port must fail");
        // This host drops SYNs to a closed port, so the client timer wins.
        // Port 0 is refused at once and still hits the connection arm.
        let err = if closed.contains("connection") {
            closed
        } else {
            try_ollama(
                &config("http://127.0.0.1:0".to_string(), 2),
                &context(),
                "list files",
            )
            .await
            .expect_err("port 0 must fail")
        };
        assert!(err.contains("connection"), "{err}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn try_ollama_timeout_when_peer_never_responds() {
        let err = try_ollama(&config(test_support::spawn_hang(), 1), &context(), "list files")
            .await
            .expect_err("hanging peer must fail");
        assert!(err == "timeout" || err.contains("timeout"), "{err}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn try_ollama_http_500() {
        let endpoint = test_support::spawn_body("500 Internal Server Error", "nope");
        let err = try_ollama(&config(endpoint, 2), &context(), "list files")
            .await
            .expect_err("HTTP 500 must fail");
        assert!(err.contains("HTTP 500"), "{err}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn try_ollama_envelope_parse_error() {
        let endpoint = test_support::spawn_body("200 OK", "not-an-envelope");
        let err = try_ollama(&config(endpoint, 2), &context(), "list files")
            .await
            .expect_err("non-envelope body must fail");
        assert!(err.contains("envelope parse"), "{err}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn try_ollama_plan_parse_truncates_long_raw() {
        let raw = "a".repeat(250);
        let endpoint = test_support::spawn_body("200 OK", &test_support::envelope(&raw));
        let err = try_ollama(&config(endpoint, 2), &context(), "list files")
            .await
            .expect_err("non-plan response must fail");
        assert!(err.contains("plan parse"), "{err}");
        assert!(err.contains("..."), "{err}");
        assert!(err.contains(&"a".repeat(200)), "{err}");
        assert!(!err.contains(&"a".repeat(201)), "{err}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn try_ollama_plan_parse_multibyte_straddling_byte_200_does_not_panic() {
        // 199 ASCII bytes then 2-byte chars: byte 200 is inside a character.
        let raw = format!("{}{}", "a".repeat(199), "\u{e9}".repeat(40));
        assert!(!raw.is_char_boundary(200));
        let endpoint = test_support::spawn_body("200 OK", &test_support::envelope(&raw));
        let err = try_ollama(&config(endpoint, 2), &context(), "list files")
            .await
            .expect_err("non-plan response must fail, not panic");
        assert!(err.contains("plan parse"), "{err}");
        assert!(err.contains(&format!("{}...", "a".repeat(199))), "{err}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn try_ollama_plan_parse_keeps_short_raw() {
        let endpoint = test_support::spawn_body("200 OK", &test_support::envelope("not-json"));
        let err = try_ollama(&config(endpoint, 2), &context(), "list files")
            .await
            .expect_err("non-plan response must fail");
        assert!(err.contains("plan parse"), "{err}");
        assert!(err.contains("raw: not-json"), "{err}");
        assert!(!err.contains("..."), "{err}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn try_ollama_rejects_empty_command() {
        let plan = test_support::plan_json("", "low", false);
        let endpoint = test_support::spawn_body("200 OK", &test_support::envelope(&plan));
        let err = try_ollama(&config(endpoint, 2), &context(), "list files")
            .await
            .expect_err("empty command must not become a proposal");
        assert!(err.contains("command is empty"), "{err}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn try_ollama_rejects_high_risk_without_approval() {
        let plan = test_support::plan_json("echo hello", "high", false);
        let endpoint = test_support::spawn_body("200 OK", &test_support::envelope(&plan));
        let err = try_ollama(&config(endpoint, 2), &context(), "list files")
            .await
            .expect_err("high risk without approval must not become a proposal");
        assert!(err.contains("must require approval"), "{err}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn try_ollama_accepts_low_risk_plan() {
        let plan = test_support::plan_json("echo hello", "low", false);
        let endpoint = test_support::spawn_body("200 OK", &test_support::envelope(&plan));
        let ctx = context();
        let proposal = try_ollama(&config(endpoint, 2), &ctx, "say hello")
            .await
            .expect("accepted plan");
        assert_eq!(proposal.source, "ollama");
        assert_eq!(proposal.command, "echo hello");
        assert_eq!(proposal.session_id, ctx.session_id);
        assert_eq!(proposal.confidence, 0.5);
    }

    #[test]
    fn test_build_review_project_context_and_network() {
        let proposal = CommandProposal {
            id: "p-net".to_string(),
            session_id: "s1".to_string(),
            source: "ollama".to_string(),
            user_intent: "fetch".to_string(),
            command: "curl https://example.test".to_string(),
            cwd: None,
            explanation: "Fetches a page".to_string(),
            assumptions: vec![],
            confidence: 0.5,
            risk: "low".to_string(),
            destructive: false,
            requires_confirmation: false,
            touches_files: false,
            touches_network: true,
            escalates_privileges: false,
            expected_output: None,
            generated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let ctx = PlanContext {
            cwd: String::new(),
            project_root: Some("repo".to_string()),
            project_facts: vec![crate::types::ProjectFact {
                kind: "workflow".to_string(),
                label: "build".to_string(),
                value: "cargo test".to_string(),
            }],
            ..PlanContext::default()
        };
        let review = build_review(&proposal, &ctx);
        assert!(review
            .safety_flags
            .contains(&"NETWORK_ACCESS".to_string()));
        assert!(!review
            .retrieved_context
            .iter()
            .any(|line| line.starts_with("cwd:")));
        assert!(review
            .retrieved_context
            .contains(&"projectRoot: repo".to_string()));
        assert!(review
            .retrieved_context
            .contains(&"workflow:build".to_string()));
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpListener, TcpStream};
    use std::thread;
    use std::time::Duration;

    pub(crate) fn closed_endpoint() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        drop(listener);
        format!("http://127.0.0.1:{port}")
    }

    pub(crate) fn spawn_hang() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                thread::sleep(Duration::from_secs(4));
                drop(stream);
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    /// Serves each response to the next connection, in order.
    pub(crate) fn spawn_sequence(responses: Vec<(&str, String)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let responses: Vec<(String, String)> = responses
            .into_iter()
            .map(|(status, body)| (status.to_string(), body))
            .collect();
        thread::spawn(move || {
            for (status, body) in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                drain_request(&mut stream);
                let header = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(body.as_bytes());
                let _ = stream.flush();
                let _ = stream.shutdown(Shutdown::Write);
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    pub(crate) fn spawn_body(status: &str, body: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let status = status.to_string();
        let body = body.to_string();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                drain_request(&mut stream);
                let header = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(body.as_bytes());
                let _ = stream.flush();
                let _ = stream.shutdown(Shutdown::Write);
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    pub(crate) fn envelope(response: &str) -> String {
        serde_json::json!({ "response": response }).to_string()
    }

    pub(crate) fn plan_json(command: &str, risk: &str, requires_approval: bool) -> String {
        serde_json::json!({
            "intent_summary": "List files",
            "command": command,
            "risk": risk,
            "explanation": "Prints a short line",
            "requires_approval": requires_approval,
            "destructive": false,
            "escalates_privileges": false,
            "confidence": 0.5
        })
        .to_string()
    }

    fn drain_request(stream: &mut TcpStream) {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        loop {
            match stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    if request_complete(&buf) || buf.len() > 2 * 1024 * 1024 {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }

    fn request_complete(buf: &[u8]) -> bool {
        let Some(header_end) = buf.windows(4).position(|window| window == b"\r\n\r\n") else {
            return false;
        };
        let headers = String::from_utf8_lossy(&buf[..header_end]);
        let mut content_length = 0usize;
        for line in headers.split("\r\n") {
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
        buf.len() >= header_end + 4 + content_length
    }
}
