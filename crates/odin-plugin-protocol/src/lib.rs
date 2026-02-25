//! Shared protocol types for plugin manifests, policy requests, and runtime events.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskTier {
    Safe,
    Sensitive,
    Destructive,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityRequest {
    pub plugin: String,
    pub project: String,
    pub capability: String,
    #[serde(default)]
    pub scope: Vec<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum PolicyDecision {
    Allow { reason_code: String },
    Deny { reason_code: String },
    RequireApproval { reason_code: String, tier: RiskTier },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ActionRequest {
    pub request_id: String,
    pub risk_tier: RiskTier,
    pub capability: CapabilityRequest,
    #[serde(default)]
    pub input: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    Executed,
    Blocked,
    ApprovalPending,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ActionOutcome {
    pub request_id: String,
    pub status: ActionStatus,
    pub detail: String,
    #[serde(default)]
    pub output: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EventEnvelope {
    pub event_id: String,
    pub event_type: String,
    pub task_id: Option<String>,
    pub request_id: Option<String>,
    pub project: Option<String>,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    pub schema_version: u32,
    pub plugin: PluginSpec,
    pub distribution: DistributionSpec,
    pub signing: Option<SigningSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginSpec {
    pub name: String,
    pub version: String,
    pub runtime: String,
    pub compatibility: CompatibilitySpec,
    pub entrypoint: EntrypointSpec,
    #[serde(default)]
    pub capabilities: Vec<CapabilitySpec>,
    #[serde(default)]
    pub hooks: Vec<HookSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompatibilitySpec {
    pub core_version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntrypointSpec {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilitySpec {
    pub id: String,
    #[serde(default)]
    pub scope: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HookSpec {
    pub event: String,
    pub handler: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DistributionSpec {
    pub source: DistributionSource,
    pub integrity: IntegritySpec,
    pub provenance: Option<ProvenanceSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DistributionSource {
    #[serde(rename = "type")]
    pub source_type: String,
    #[serde(rename = "ref")]
    pub ref_value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntegritySpec {
    pub checksum_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvenanceSpec {
    pub builder: Option<String>,
    pub repo: Option<String>,
    pub commit: Option<String>,
    pub build_time: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SigningSpec {
    pub required: Option<bool>,
    pub method: Option<String>,
    pub signature: Option<String>,
    pub certificate: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn policy_decision_round_trip() {
        let decision = PolicyDecision::RequireApproval {
            reason_code: "destructive_requires_approval".to_string(),
            tier: RiskTier::Destructive,
        };

        let encoded = serde_json::to_string(&decision).expect("serialize");
        let decoded: PolicyDecision = serde_json::from_str(&encoded).expect("deserialize");

        assert_eq!(decoded, decision);
    }

    #[test]
    fn action_outcome_defaults_output() {
        let value = json!({
            "request_id": "r1",
            "status": "executed",
            "detail": "ok"
        });
        let decoded: ActionOutcome = serde_json::from_value(value).expect("decode");
        assert_eq!(decoded.output, json!(null));
    }
}
