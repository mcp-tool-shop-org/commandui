//! Proposal validation — fail closed.
//!
//! If proposal fields disagree (e.g., destructive + low risk, privilege escalation
//! without approval), the proposal is rejected before any shell renders it.

use crate::types::LlmPlanResponse;

/// Validate an LLM plan response for consistency.
/// Returns Ok(()) if valid, Err with reason if not.
pub(crate) fn validate_llm_response(plan: &LlmPlanResponse) -> Result<(), String> {
    if plan.command.trim().is_empty() {
        return Err("command is empty".to_string());
    }

    if plan.explanation.trim().is_empty() {
        return Err("explanation is empty".to_string());
    }

    if plan.intent_summary.trim().is_empty() {
        return Err("intent_summary is empty".to_string());
    }

    match plan.risk.as_str() {
        "low" | "medium" | "high" => {}
        other => {
            return Err(format!(
                "invalid risk level: '{other}' (expected low/medium/high)"
            ));
        }
    }

    if !(0.0..=1.0).contains(&plan.confidence) {
        return Err(format!(
            "confidence {} out of range 0.0-1.0",
            plan.confidence
        ));
    }

    // Consistency: destructive command cannot have low risk
    if plan.destructive && plan.risk == "low" {
        return Err("destructive command cannot have low risk".to_string());
    }

    // Privilege escalation is the more specific failure when risk is also high.
    if plan.escalates_privileges && !plan.requires_approval {
        return Err("privilege escalation must require approval".to_string());
    }

    if matches!(plan.risk.as_str(), "medium" | "high") && !plan.requires_approval {
        return Err("medium or high risk must require approval".to_string());
    }

    if plan.destructive && !plan.requires_approval {
        return Err("destructive command must require approval".to_string());
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CommandFloor {
    pub destructive: bool,
    pub escalates_privileges: bool,
}

fn command_tokens(command: &str) -> Vec<String> {
    command
        .split(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    ';' | '|' | '&' | '(' | ')' | '`' | '<' | '>' | '\n' | '\r'
                )
        })
        .filter_map(|raw| {
            let token = raw.trim_matches(|c: char| matches!(c, '"' | '\'' | '\\'));
            if token.is_empty() {
                None
            } else {
                Some(token.to_ascii_lowercase())
            }
        })
        .collect()
}

fn token_base(token: &str) -> &str {
    let base = token.rsplit(['/', '\\']).next().unwrap_or(token);
    base.strip_suffix(".exe").unwrap_or(base)
}

pub(crate) fn command_floor(command: &str) -> CommandFloor {
    let mut floor = CommandFloor {
        destructive: false,
        escalates_privileges: false,
    };
    for token in command_tokens(command) {
        let base = token_base(&token);
        if matches!(base, "rm" | "del" | "remove-item" | "mkfs") || base.starts_with("mkfs.") {
            floor.destructive = true;
        }
        if base == "sudo" {
            floor.escalates_privileges = true;
        }
    }
    floor
}

/// Dangerous command text forces high risk and approval even when the model
/// omitted or lied about the flags.
pub(crate) fn apply_command_safety_floor(plan: &mut LlmPlanResponse) {
    let floor = command_floor(&plan.command);
    if floor.destructive {
        plan.destructive = true;
    }
    if floor.escalates_privileges {
        plan.escalates_privileges = true;
    }
    if floor.destructive || floor.escalates_privileges {
        plan.risk = "high".to_string();
        plan.requires_approval = true;
    }
}

pub(crate) fn accept_llm_plan(plan: &mut LlmPlanResponse) -> Result<(), String> {
    apply_command_safety_floor(plan);
    validate_llm_response(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LlmPlanResponse;

    fn valid_plan() -> LlmPlanResponse {
        LlmPlanResponse {
            intent_summary: "List files".to_string(),
            command: "ls -la".to_string(),
            risk: "low".to_string(),
            explanation: "Lists all files".to_string(),
            assumptions: vec![],
            requires_approval: false,
            destructive: false,
            touches_files: true,
            touches_network: false,
            escalates_privileges: false,
            confidence: 0.95,
            expected_output: None,
        }
    }

    #[test]
    fn test_valid_plan_passes() {
        assert!(validate_llm_response(&valid_plan()).is_ok());
    }

    #[test]
    fn test_empty_command_rejected() {
        let mut p = valid_plan();
        p.command = "".to_string();
        assert!(validate_llm_response(&p).is_err());
    }

    #[test]
    fn test_empty_explanation_rejected() {
        let mut p = valid_plan();
        p.explanation = "".to_string();
        assert!(validate_llm_response(&p).is_err());
    }

    #[test]
    fn test_empty_intent_summary_rejected() {
        let mut p = valid_plan();
        p.intent_summary = "".to_string();
        assert!(validate_llm_response(&p).is_err());
    }

    #[test]
    fn test_invalid_risk_rejected() {
        let mut p = valid_plan();
        p.risk = "extreme".to_string();
        assert!(validate_llm_response(&p).is_err());
    }

    #[test]
    fn test_confidence_out_of_range_rejected() {
        let mut p = valid_plan();
        p.confidence = 1.5;
        assert!(validate_llm_response(&p).is_err());
    }

    #[test]
    fn test_destructive_low_risk_rejected() {
        let mut p = valid_plan();
        p.destructive = true;
        p.risk = "low".to_string();
        assert!(validate_llm_response(&p).is_err());
    }

    #[test]
    fn test_escalation_without_approval_rejected() {
        let mut p = valid_plan();
        p.escalates_privileges = true;
        p.requires_approval = false;
        p.risk = "high".to_string();
        assert!(validate_llm_response(&p).is_err());
    }

    #[test]
    fn test_destructive_high_risk_with_approval_passes() {
        let mut p = valid_plan();
        p.destructive = true;
        p.risk = "high".to_string();
        p.requires_approval = true;
        assert!(validate_llm_response(&p).is_ok());
    }

    #[test]
    fn test_medium_risk_without_approval_rejected() {
        let mut p = valid_plan();
        p.risk = "medium".to_string();
        p.requires_approval = false;
        assert!(validate_llm_response(&p).is_err());
    }

    #[test]
    fn test_high_risk_without_approval_rejected() {
        let mut p = valid_plan();
        p.risk = "high".to_string();
        p.requires_approval = false;
        assert!(validate_llm_response(&p).is_err());
    }

    #[test]
    fn test_destructive_without_approval_rejected() {
        let mut p = valid_plan();
        p.destructive = true;
        p.risk = "high".to_string();
        p.requires_approval = false;
        assert!(validate_llm_response(&p).is_err());
    }

    #[test]
    fn test_medium_destructive_with_approval_passes() {
        let mut p = valid_plan();
        p.destructive = true;
        p.risk = "medium".to_string();
        p.requires_approval = true;
        assert!(validate_llm_response(&p).is_ok());
    }

    #[test]
    fn command_text_floor_forces_high_risk_and_approval() {
        let mut rm = valid_plan();
        rm.command = "rm -rf /tmp/old".to_string();
        rm.risk = "low".to_string();
        rm.destructive = false;
        rm.requires_approval = false;
        accept_llm_plan(&mut rm).unwrap();
        assert_eq!(rm.risk, "high");
        assert!(rm.destructive);
        assert!(rm.requires_approval);

        let mut sudo = valid_plan();
        sudo.command = "/usr/bin/sudo ls".to_string();
        sudo.risk = "low".to_string();
        sudo.escalates_privileges = false;
        sudo.requires_approval = false;
        accept_llm_plan(&mut sudo).unwrap();
        assert_eq!(sudo.risk, "high");
        assert!(sudo.escalates_privileges);
        assert!(sudo.requires_approval);
        assert!(!sudo.destructive);

        let mut mkfs = valid_plan();
        mkfs.command = "mkfs.ext4 /dev/sdb".to_string();
        mkfs.requires_approval = false;
        mkfs.destructive = false;
        mkfs.risk = "low".to_string();
        accept_llm_plan(&mut mkfs).unwrap();
        assert!(mkfs.destructive);
        assert!(mkfs.requires_approval);

        let mut remove = valid_plan();
        remove.command = "Remove-Item -Recurse C:\\work\\old".to_string();
        remove.requires_approval = false;
        remove.destructive = false;
        remove.risk = "low".to_string();
        accept_llm_plan(&mut remove).unwrap();
        assert!(remove.destructive);
        assert_eq!(remove.risk, "high");

        let mut del = valid_plan();
        del.command = "del /q notes.txt".to_string();
        del.requires_approval = false;
        del.destructive = false;
        del.risk = "low".to_string();
        accept_llm_plan(&mut del).unwrap();
        assert!(del.destructive);
    }

    #[test]
    fn command_text_floor_ignores_embedded_tokens() {
        let mut firmware = valid_plan();
        firmware.command = "echo firmware".to_string();
        accept_llm_plan(&mut firmware).unwrap();
        assert_eq!(firmware.risk, "low");
        assert!(!firmware.destructive);
        assert!(!firmware.requires_approval);

        let mut delta = valid_plan();
        delta.command = "echo delta".to_string();
        accept_llm_plan(&mut delta).unwrap();
        assert!(!delta.destructive);
        assert!(!delta.requires_approval);
    }
}
