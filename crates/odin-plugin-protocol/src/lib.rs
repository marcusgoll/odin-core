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
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    Trusted,
    Caution,
    Untrusted,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillScope {
    Global,
    Project,
    User,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillRecord {
    pub name: String,
    pub trust_level: TrustLevel,
    pub source: String,
    pub pinned_version: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<DelegationCapability>,
}

impl SkillRecord {
    pub fn default_for(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            trust_level: TrustLevel::Untrusted,
            source: "local:unknown".to_string(),
            pinned_version: None,
            capabilities: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillRegistry {
    pub schema_version: u32,
    pub scope: SkillScope,
    #[serde(default)]
    pub skills: Vec<SkillRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DelegationCapability {
    pub id: String,
    #[serde(default)]
    pub scope: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginPermissionEnvelope {
    pub plugin: String,
    pub trust_level: TrustLevel,
    #[serde(default)]
    pub permissions: Vec<DelegationCapability>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityManifest {
    pub schema_version: u32,
    pub plugin: String,
    #[serde(default)]
    pub capabilities: Vec<DelegationCapability>,
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
    use std::fs;
    use std::path::PathBuf;

    fn load_schema(file_name: &str) -> serde_json::Value {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas")
            .join(file_name);
        let raw = fs::read_to_string(&path).expect("read schema");
        serde_json::from_str(&raw).expect("decode schema")
    }

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

    #[test]
    fn skill_registry_round_trip() {
        let registry = SkillRegistry {
            schema_version: 1,
            scope: SkillScope::Project,
            skills: vec![SkillRecord {
                name: "brainstorming".to_string(),
                trust_level: TrustLevel::Trusted,
                source: "local:/skills/brainstorming".to_string(),
                pinned_version: Some("abc123".to_string()),
                ..SkillRecord::default_for("brainstorming")
            }],
        };

        let encoded = serde_json::to_string(&registry).expect("encode");
        let decoded: SkillRegistry = serde_json::from_str(&encoded).expect("decode");

        assert_eq!(decoded.scope, SkillScope::Project);
        assert_eq!(decoded.skills.len(), 1);
        assert_eq!(decoded.skills[0].name, "brainstorming");
    }

    #[test]
    fn capability_manifest_round_trip() {
        let manifest = CapabilityManifest {
            schema_version: 1,
            plugin: "stagehand".to_string(),
            capabilities: vec![DelegationCapability {
                id: "browser.observe".to_string(),
                scope: vec!["example.com".to_string()],
            }],
        };

        let encoded = serde_json::to_string(&manifest).expect("encode");
        let decoded: CapabilityManifest = serde_json::from_str(&encoded).expect("decode");

        assert_eq!(decoded.plugin, "stagehand");
        assert_eq!(decoded.capabilities.len(), 1);
        assert_eq!(decoded.capabilities[0].id, "browser.observe");
    }

    #[test]
    fn plugin_permission_envelope_round_trip() {
        let envelope = PluginPermissionEnvelope {
            plugin: "stagehand".to_string(),
            trust_level: TrustLevel::Caution,
            permissions: vec![DelegationCapability {
                id: "browser.observe".to_string(),
                scope: vec!["example.com".to_string()],
            }],
        };

        let encoded = serde_json::to_string(&envelope).expect("encode");
        let decoded: PluginPermissionEnvelope = serde_json::from_str(&encoded).expect("decode");

        assert_eq!(decoded.plugin, "stagehand");
        assert_eq!(decoded.trust_level, TrustLevel::Caution);
        assert_eq!(decoded.permissions[0].id, "browser.observe");
    }

    #[test]
    fn skill_registry_defaults_missing_skills_array() {
        let value = json!({
            "schema_version": 1,
            "scope": "project"
        });

        let decoded: SkillRegistry = serde_json::from_value(value).expect("decode");
        assert!(decoded.skills.is_empty());
    }

    #[test]
    fn skill_record_defaults_missing_capabilities_array() {
        let value = json!({
            "name": "brainstorming",
            "trust_level": "trusted",
            "source": "local:/skills/brainstorming",
            "pinned_version": null
        });

        let decoded: SkillRecord = serde_json::from_value(value).expect("decode");
        assert!(decoded.capabilities.is_empty());
    }

    #[test]
    fn delegation_capability_defaults_missing_scope_array() {
        let value = json!({
            "id": "browser.observe"
        });

        let decoded: DelegationCapability = serde_json::from_value(value).expect("decode");
        assert!(decoded.scope.is_empty());
    }

    #[test]
    fn capability_manifest_defaults_missing_capabilities_array() {
        let value = json!({
            "schema_version": 1,
            "plugin": "stagehand"
        });

        let decoded: CapabilityManifest = serde_json::from_value(value).expect("decode");
        assert!(decoded.capabilities.is_empty());
    }

    #[test]
    fn plugin_permission_envelope_defaults_missing_permissions_array() {
        let value = json!({
            "plugin": "stagehand",
            "trust_level": "caution"
        });

        let decoded: PluginPermissionEnvelope = serde_json::from_value(value).expect("decode");
        assert!(decoded.permissions.is_empty());
    }

    #[test]
    fn skill_registry_schema_allows_serde_defaulted_arrays() {
        let schema = load_schema("skill-registry.v1.schema.json");

        let root_required = schema["required"].as_array().expect("root required");
        assert!(!root_required.iter().any(|item| item.as_str() == Some("skills")));
        assert_eq!(schema["properties"]["skills"]["default"], json!([]));

        let skill_record_required = schema["$defs"]["skill_record"]["required"]
            .as_array()
            .expect("skill_record required");
        assert!(!skill_record_required
            .iter()
            .any(|item| item.as_str() == Some("capabilities")));
        assert_eq!(
            schema["$defs"]["skill_record"]["properties"]["capabilities"]["default"],
            json!([])
        );
        assert_eq!(
            schema["$defs"]["skill_record"]["properties"]["capabilities"]["items"]["$ref"],
            json!(
                "https://odin-core.dev/schemas/capability-manifest.v1.schema.json#/$defs/delegation_capability"
            )
        );
    }

    #[test]
    fn capability_manifest_schema_allows_serde_defaulted_arrays() {
        let schema = load_schema("capability-manifest.v1.schema.json");

        let root_required = schema["required"].as_array().expect("root required");
        assert!(!root_required
            .iter()
            .any(|item| item.as_str() == Some("capabilities")));
        assert_eq!(schema["properties"]["capabilities"]["default"], json!([]));

        let capability_required = schema["$defs"]["delegation_capability"]["required"]
            .as_array()
            .expect("delegation_capability required");
        assert!(!capability_required
            .iter()
            .any(|item| item.as_str() == Some("scope")));
        assert_eq!(
            schema["$defs"]["delegation_capability"]["properties"]["scope"]["default"],
            json!([])
        );
    }
}
