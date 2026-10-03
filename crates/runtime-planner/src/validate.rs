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
    /// Output redirection (`>`, not `>>`) that truncates a file.
    pub truncates_file: bool,
}

/// True when the command redirects output into a file with a single `>`
/// (which truncates), ignoring quoted text, `>>`, fd duplication (`2>&1`)
/// and the null devices.
fn has_truncating_redirect(command: &str) -> bool {
    let chars: Vec<char> = command.chars().collect();
    let mut quote: Option<char> = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                } else if c == '>' {
                    if chars.get(i + 1) == Some(&'>') {
                        i += 2;
                        continue;
                    }
                    let mut j = i + 1;
                    if chars.get(j) == Some(&'&') {
                        i = j + 1;
                        continue;
                    }
                    if chars.get(j) == Some(&'|') {
                        j += 1;
                    }
                    while chars.get(j).is_some_and(|ch| ch.is_whitespace()) {
                        j += 1;
                    }
                    let target: String = chars[j..]
                        .iter()
                        .take_while(|ch| !ch.is_whitespace() && !matches!(ch, ';' | '|' | '&' | ')'))
                        .collect();
                    let target = target.trim_matches(|ch: char| matches!(ch, '"' | '\'')).to_ascii_lowercase();
                    if !target.is_empty() && !matches!(target.as_str(), "/dev/null" | "nul" | "$null") {
                        return true;
                    }
                }
            }
        }
        i += 1;
    }
    false
}

fn command_tokens(command: &str) -> Vec<String> {
    command
        .split(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    ';' | '|' | '&' | '(' | ')' | '`' | '<' | '>' | '\n' | '\r' | '{' | '}' | '$'
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

/// Commands that overwrite a file like `>` does: Set-Content / Out-File /
/// Clear-Content and `tee` without an append flag. cp/mv are not listed: the
/// floor cannot see whether the destination already exists.
fn overwrites_file_by_command(tokens: &[String]) -> bool {
    tokens.iter().enumerate().any(|(i, token)| {
        let base = token_base(token);
        match base {
            "set-content" | "out-file" | "clear-content" | "sc" => {
                !tokens[i + 1..].iter().any(|t| t == "-append")
            }
            "tee" => !tokens[i + 1..]
                .iter()
                .any(|t| t == "-a" || t == "--append" || t == "-append"),
            _ => false,
        }
    })
}

pub(crate) fn command_floor(command: &str) -> CommandFloor {
    let mut floor = CommandFloor {
        destructive: false,
        escalates_privileges: false,
        truncates_file: has_truncating_redirect(command),
    };
    let tokens = command_tokens(command);
    if overwrites_file_by_command(&tokens) {
        floor.truncates_file = true;
    }
    let has_after = |start: usize, wanted: &str| tokens[start + 1..].iter().any(|t| t == wanted);
    for (i, token) in tokens.iter().enumerate() {
        let base = token_base(token);
        if matches!(
            base,
            "rm" | "del"
                | "erase"
                | "remove-item"
                | "mkfs"
                | "format"
                | "format-volume"
                | "clear-disk"
                | "dd"
                | "diskpart"
                | "shred"
                | "rmdir"
                | "rd"
                | "ri"
                | "unlink"
                | "truncate"
                | "wipefs"
                | "fdisk"
                | "parted"
                | "sfdisk"
                | "sgdisk"
                | "shutdown"
                | "reboot"
                | "poweroff"
                | "halt"
                | "stop-computer"
                | "restart-computer"
        ) || base.starts_with("mkfs.")
        {
            floor.destructive = true;
        }
        if matches!(base, "chmod" | "chown" | "chgrp")
            && tokens[i + 1..].iter().any(|t| {
                t == "--recursive" || (t.starts_with('-') && !t.starts_with("--") && t.contains('r'))
            })
        {
            floor.destructive = true;
        }
        if base == "git"
            && tokens[i + 1..].iter().any(|t| {
                t == "--force"
                    || t == "-f"
                    || t.starts_with("--force-with-lease")
                    || t.starts_with("--force-if-includes")
            })
            && has_after(i, "push")
        {
            floor.destructive = true;
        }
        if base == "find" && has_after(i, "-delete") {
            floor.destructive = true;
        }
        if base == "git" && (has_after(i, "clean") || (has_after(i, "reset") && has_after(i, "--hard")))
        {
            floor.destructive = true;
        }
        if matches!(base, "sudo" | "doas" | "pkexec" | "gsudo" | "runas" | "su") {
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
    } else if floor.truncates_file {
        // Additive: never lowers a risk the model already set higher.
        if plan.risk == "low" {
            plan.risk = "medium".to_string();
        }
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
    fn command_text_floor_covers_wipe_and_privilege_commands() {
        for command in [
            "format E:",
            "Format-Volume -DriveLetter E",
            "dd if=/dev/zero of=/dev/sda",
            "diskpart",
            "shred -u secrets.txt",
            "rmdir /s /q build",
            "rd /s build",
            "Clear-Disk -Number 1",
            "find . -delete",
            "git clean -fdx",
            "git reset --hard HEAD~1",
            "echo ok
format E:",
        ] {
            let mut p = valid_plan();
            p.command = command.to_string();
            accept_llm_plan(&mut p).unwrap();
            assert!(p.destructive, "{command}");
            assert_eq!(p.risk, "high", "{command}");
            assert!(p.requires_approval, "{command}");
        }
        for command in ["doas ls", "pkexec ls", "gsudo ls", "runas /user:admin cmd"] {
            let mut p = valid_plan();
            p.command = command.to_string();
            accept_llm_plan(&mut p).unwrap();
            assert!(p.escalates_privileges, "{command}");
            assert_eq!(p.risk, "high", "{command}");
            assert!(p.requires_approval, "{command}");
        }
        let mut status = valid_plan();
        status.command = "git status".to_string();
        accept_llm_plan(&mut status).unwrap();
        assert!(!status.destructive);
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

    #[test]
    fn floor_covers_aliases_system_commands_and_force_push() {
        for cmd in [
            "ri old",
            "unlink f",
            "truncate -s 0 f",
            "wipefs -a /dev/sda",
            "fdisk /dev/sda",
            "parted /dev/sda",
            "shutdown now",
            "reboot",
            "poweroff",
            "halt",
            "Stop-Computer",
            "Restart-Computer",
            "git push --force origin main",
            "git push -f",
            "chmod -R 777 /",
            "chown -R me /",
            "if ($x) {rm x}",
        ] {
            let mut p = valid_plan();
            p.command = cmd.to_string();
            accept_llm_plan(&mut p).unwrap();
            assert!(p.destructive, "{cmd}");
            assert_eq!(p.risk, "high", "{cmd}");
            assert!(p.requires_approval, "{cmd}");
        }
        let mut su = valid_plan();
        su.command = "su -".to_string();
        accept_llm_plan(&mut su).unwrap();
        assert!(su.escalates_privileges && su.requires_approval);

        let mut push = valid_plan();
        push.command = "git push origin main".to_string();
        accept_llm_plan(&mut push).unwrap();
        assert!(!push.destructive);
    }

    #[test]
    fn overwrite_commands_are_medium_unless_appending() {
        for cmd in [
            "Set-Content notes.txt hi",
            "echo hi | Out-File notes.txt",
            "echo hi | tee notes.txt",
            "Clear-Content notes.txt",
        ] {
            let mut p = valid_plan();
            p.command = cmd.to_string();
            accept_llm_plan(&mut p).unwrap();
            assert_eq!(p.risk, "medium", "{cmd}");
            assert!(p.requires_approval, "{cmd}");
        }
        for cmd in ["echo hi | tee -a notes.txt", "echo hi | Out-File notes.txt -Append"] {
            let mut p = valid_plan();
            p.command = cmd.to_string();
            accept_llm_plan(&mut p).unwrap();
            assert_eq!(p.risk, "low", "{cmd}");
        }
    }

    #[test]
    fn truncating_redirect_needs_approval_but_append_and_null_do_not() {
        for cmd in ["echo hi > notes.txt", ": > notes.txt", "ls >out.txt"] {
            let mut p = valid_plan();
            p.command = cmd.to_string();
            accept_llm_plan(&mut p).unwrap();
            assert_eq!(p.risk, "medium", "{cmd}");
            assert!(p.requires_approval, "{cmd}");
            assert!(!p.destructive, "{cmd}");
        }
        for cmd in ["echo hi >> notes.txt", "ls > /dev/null", "make 2>&1", "echo \"a > b\""] {
            let mut p = valid_plan();
            p.command = cmd.to_string();
            accept_llm_plan(&mut p).unwrap();
            assert_eq!(p.risk, "low", "{cmd}");
            assert!(!p.requires_approval, "{cmd}");
        }
    }
}
