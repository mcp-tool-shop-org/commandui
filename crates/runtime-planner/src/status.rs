//! Why Ask can or cannot draft a command.
//!
//! A release build returns this instead of a runnable stand-in. The words are
//! the ones the desktop shows, so a test can lock them without a window.

use crate::types::{CommandProposal, PlanReview};
use serde::Serialize;

pub const DOWNLOAD_PAGE: &str = "https://ollama.com/download";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannerStatus {
    pub state: String,
    pub model: String,
    pub endpoint: String,
    pub headline: String,
    pub fix: String,
    pub link: String,
    pub link_label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlannerAnswer {
    Proposal {
        proposal: CommandProposal,
        review: PlanReview,
        status: PlannerStatus,
    },
    Unavailable {
        status: PlannerStatus,
    },
}

/// A refused connection means Ollama is not running when its program is on
/// PATH, and not installed when it is not.
pub fn classify_connection(ollama_on_path: bool) -> &'static str {
    if ollama_on_path {
        "notRunning"
    } else {
        "notInstalled"
    }
}

pub fn model_is_present(names: &[String], wanted: &str) -> bool {
    let wanted = wanted.trim();
    if wanted.is_empty() {
        return false;
    }
    names.iter().any(|name| {
        let name = name.trim();
        name == wanted
            || name.strip_suffix(":latest") == Some(wanted)
            || name.starts_with(&format!("{wanted}:"))
    })
}

pub fn ollama_on_path(path_var: &str, windows: bool) -> bool {
    let file = if windows { "ollama.exe" } else { "ollama" };
    let sep = if windows { ';' } else { ':' };
    path_var.split(sep).any(|dir| {
        let dir = dir.trim().trim_matches('"');
        !dir.is_empty() && std::path::Path::new(dir).join(file).is_file()
    })
}

pub fn executable_on_path() -> bool {
    let path = std::env::var("PATH").unwrap_or_default();
    ollama_on_path(&path, cfg!(windows))
}

pub fn status_for(state: &str, model: &str, endpoint: &str) -> PlannerStatus {
    let shown = if model.trim().is_empty() {
        "the model".to_string()
    } else {
        model.trim().to_string()
    };
    let (headline, fix, link, link_label) = match state {
        "ready" => (
            "Ready.".to_string(),
            format!("Ask can draft a command with {shown}."),
            "https://ollama.com/library".to_string(),
            "Model library".to_string(),
        ),
        "notInstalled" => (
            "Ollama is not installed.".to_string(),
            format!("Install Ollama, then download {shown}."),
            DOWNLOAD_PAGE.to_string(),
            "Download Ollama".to_string(),
        ),
        "notRunning" => (
            "Ollama is not running.".to_string(),
            "Start Ollama, then choose Check again.".to_string(),
            DOWNLOAD_PAGE.to_string(),
            "Download Ollama".to_string(),
        ),
        "modelMissing" => (
            format!("The model {shown} is not downloaded."),
            format!("Download it with: ollama pull {shown}"),
            library_link(&shown),
            "Model page".to_string(),
        ),
        _ => (
            "Ollama did not return a plan.".to_string(),
            format!("Check that {shown} is available, then choose Check again."),
            DOWNLOAD_PAGE.to_string(),
            "Download Ollama".to_string(),
        ),
    };
    PlannerStatus {
        state: state.to_string(),
        model: model.to_string(),
        endpoint: endpoint.to_string(),
        headline,
        fix,
        link,
        link_label,
    }
}

fn library_link(model: &str) -> String {
    let base = model.split(':').next().unwrap_or(model);
    if base.is_empty() || base == "the model" {
        "https://ollama.com/library".to_string()
    } else {
        format!("https://ollama.com/library/{base}")
    }
}

/// `allow_mock` is true only for tests, browser preview, and a debug build.
/// A release caller passes false and gets no command.
pub fn plan_for_user(
    allow_mock: bool,
    status: PlannerStatus,
    proposal: CommandProposal,
    review: PlanReview,
) -> PlannerAnswer {
    if allow_mock {
        PlannerAnswer::Proposal {
            proposal,
            review,
            status,
        }
    } else {
        PlannerAnswer::Unavailable { status }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::build_review;
    use crate::mock;
    use crate::types::PlanContext;

    #[test]
    fn each_state_names_itself_and_a_fix() {
        let cases = ["notInstalled", "notRunning", "modelMissing", "ready"];
        for state in cases {
            let status = status_for(state, "qwen2.5:14b", "http://localhost:11434");
            assert_eq!(status.state, state);
            assert!(!status.headline.is_empty(), "{state}");
            assert!(status.fix.contains("qwen2.5:14b") || state == "notRunning", "{state}: {}", status.fix);
            assert!(status.link.starts_with("https://ollama.com/"), "{state}");
            assert!(!status.link_label.is_empty(), "{state}");
        }
        let missing = status_for("modelMissing", "qwen2.5:14b", "http://localhost:11434");
        assert_eq!(missing.headline, "The model qwen2.5:14b is not downloaded.");
        assert!(missing.fix.contains("ollama pull qwen2.5:14b"));
        assert_eq!(missing.link, "https://ollama.com/library/qwen2.5");
    }

    #[test]
    fn model_names_match_the_tag_or_a_longer_tag() {
        let names = vec!["qwen2.5:14b".to_string(), "llama3.2:latest".to_string()];
        assert!(model_is_present(&names, "qwen2.5:14b"));
        assert!(model_is_present(&names, "llama3.2"));
        assert!(!model_is_present(&names, "missing:7b"));
        assert!(!model_is_present(&names, "  "));
    }

    #[test]
    fn connection_class_follows_the_program_on_path() {
        assert_eq!(classify_connection(true), "notRunning");
        assert_eq!(classify_connection(false), "notInstalled");
    }

    #[test]
    fn path_scan_finds_the_program_and_ignores_the_other_name() {
        let dir = std::env::temp_dir().join(format!("cui-planner-path-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe_name = if cfg!(windows) { "ollama.exe" } else { "ollama" };
        std::fs::write(dir.join(exe_name), b"").unwrap();
        let path = dir.to_string_lossy().to_string();
        assert!(ollama_on_path(&path, cfg!(windows)));
        assert!(!ollama_on_path(&path, !cfg!(windows)));
        assert!(!ollama_on_path("", cfg!(windows)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn release_without_a_model_has_no_runnable_command() {
        let status = status_for("notInstalled", "qwen2.5:14b", "http://localhost:11434");
        let context = PlanContext {
            session_id: "s1".to_string(),
            cwd: "notes".to_string(),
            ..PlanContext::default()
        };
        let proposal = mock::generate(&context, "list the files");
        assert!(!proposal.command.is_empty());
        let review = build_review(&proposal, &context);
        let answer = plan_for_user(false, status, proposal, review);
        let value = serde_json::to_value(&answer).unwrap();
        assert_eq!(value["kind"], "unavailable");
        assert!(value.get("proposal").is_none(), "{value}");
        let dumped = value.to_string();
        assert!(!dumped.contains("\"command\""), "{dumped}");
        assert_eq!(value["status"]["headline"], "Ollama is not installed.");
        assert!(value["status"]["fix"].as_str().unwrap().contains("qwen2.5:14b"));
    }

    #[test]
    fn a_labeled_stand_in_is_only_returned_when_mock_is_allowed() {
        let status = status_for("notRunning", "qwen2.5:14b", "http://localhost:11434");
        let context = PlanContext::default();
        let proposal = mock::generate(&context, "show changed files");
        let review = build_review(&proposal, &context);
        let answer = plan_for_user(true, status, proposal, review);
        match answer {
            PlannerAnswer::Proposal { proposal, .. } => {
                assert_eq!(proposal.source, "mock");
                assert_eq!(proposal.command, "git status --short");
            }
            PlannerAnswer::Unavailable { .. } => panic!("debug stand-in was dropped"),
        }
    }
}
