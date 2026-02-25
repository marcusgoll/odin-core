use odin_plugin_protocol::{SkillRecord, TrustLevel};
use thiserror::Error;

use crate::risk_scan::{RiskCategory, RiskFinding, scan_skill_content};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ack {
    None,
    Accepted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallGateStatus {
    Allowed,
    BlockedAckRequired,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillImportCandidate {
    pub record: SkillRecord,
    pub scripts: Vec<String>,
    pub readme: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallPlan {
    pub status: InstallGateStatus,
    pub findings: Vec<RiskFinding>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ImportGateError {
    #[error("skill name must not be empty")]
    EmptyName,
}

pub fn evaluate_install(candidate: &SkillImportCandidate, ack: Ack) -> Result<InstallPlan, ImportGateError> {
    if candidate.record.name.trim().is_empty() {
        return Err(ImportGateError::EmptyName);
    }

    let findings = scan_skill_content(&candidate.scripts, candidate.readme.as_deref());
    let mut reasons = Vec::new();
    let has_secret_finding = findings
        .iter()
        .any(|finding| finding.category == RiskCategory::Secret);

    if candidate.record.trust_level == TrustLevel::Untrusted {
        reasons.push("untrusted_skill".to_string());
    }
    if !candidate.scripts.is_empty() {
        reasons.push("script_present".to_string());
    }
    if has_secret_finding {
        reasons.push("secret_touching_risk".to_string());
    }

    let ack_required = !reasons.is_empty();
    let status = if ack_required && matches!(ack, Ack::None) {
        InstallGateStatus::BlockedAckRequired
    } else {
        InstallGateStatus::Allowed
    };

    Ok(InstallPlan {
        status,
        findings,
        reasons,
    })
}
