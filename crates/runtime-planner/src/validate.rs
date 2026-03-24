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

    // Consistency: privilege escalation must require approval
    if plan.escalates_privileges && !plan.requires_approval {
        return Err("privilege escalation must require approval".to_string());
    }

    Ok(())
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
}
