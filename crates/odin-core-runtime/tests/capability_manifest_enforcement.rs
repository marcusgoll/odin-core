use std::sync::{Arc, Mutex};

use odin_audit::{AuditError, AuditRecord, AuditSink};
use odin_core_runtime::{DryRunExecutor, OrchestratorRuntime};
use odin_plugin_protocol::{
    ActionRequest, ActionStatus, CapabilityManifest, CapabilityRequest, DelegationCapability,
    RiskTier,
};
use odin_policy_engine::StaticPolicyEngine;

#[derive(Clone, Default)]
struct MemoryAuditSink {
    records: Arc<Mutex<Vec<AuditRecord>>>,
}

impl MemoryAuditSink {
    fn events(&self) -> Vec<String> {
        self.records
            .lock()
            .expect("lock")
            .iter()
            .map(|record| record.event_type.clone())
            .collect()
    }
}

impl AuditSink for MemoryAuditSink {
    fn record(&self, record: AuditRecord) -> Result<(), AuditError> {
        self.records
            .lock()
            .map_err(|_| AuditError::Write("poisoned lock".to_string()))?
            .push(record);
        Ok(())
    }
}

fn request_for(plugin: &str, capability: &str) -> ActionRequest {
    ActionRequest {
        request_id: format!("req-{capability}"),
        risk_tier: RiskTier::Safe,
        capability: CapabilityRequest {
            plugin: plugin.to_string(),
            project: "demo".to_string(),
            capability: capability.to_string(),
            scope: vec!["project".to_string()],
            reason: "unit test".to_string(),
        },
        input: serde_json::json!({
            "url": "https://example.com"
        }),
    }
}

fn request_for_with_scope(plugin: &str, capability: &str, scope: &[&str]) -> ActionRequest {
    ActionRequest {
        request_id: format!("req-{capability}"),
        risk_tier: RiskTier::Safe,
        capability: CapabilityRequest {
            plugin: plugin.to_string(),
            project: "demo".to_string(),
            capability: capability.to_string(),
            scope: scope.iter().map(|value| value.to_string()).collect(),
            reason: "unit test".to_string(),
        },
        input: serde_json::json!({
            "url": "https://example.com"
        }),
    }
}

fn manifest_allowing(plugin: &str, capability: &str) -> CapabilityManifest {
    CapabilityManifest {
        schema_version: 1,
        plugin: plugin.to_string(),
        capabilities: vec![DelegationCapability {
            id: capability.to_string(),
            scope: vec!["project".to_string()],
        }],
    }
}

#[test]
fn denies_capability_not_in_manifest() {
    let mut policy = StaticPolicyEngine::default();
    policy.allow_capability("example.safe-github", "demo", "repo.delete");

    let audit = MemoryAuditSink::default();
    let runtime = OrchestratorRuntime::new(policy, audit.clone(), DryRunExecutor);
    let outcome = runtime
        .handle_action_with_manifest(
            request_for("example.safe-github", "repo.delete"),
            &manifest_allowing("example.safe-github", "repo.read"),
        )
        .expect("outcome");

    assert_eq!(outcome.status, ActionStatus::Blocked);
    assert_eq!(outcome.detail, "manifest_capability_not_granted");
    assert!(audit
        .events()
        .iter()
        .any(|event| event == "governance.manifest.denied"));
}

#[test]
fn denies_stagehand_for_non_browser_agent() {
    let mut policy = StaticPolicyEngine::default();
    policy.allow_capability("example.safe-github", "demo", "stagehand.observe_url");

    let audit = MemoryAuditSink::default();
    let runtime = OrchestratorRuntime::new(policy, audit.clone(), DryRunExecutor);
    let outcome = runtime
        .handle_action_with_manifest(
            request_for("example.safe-github", "stagehand.observe_url"),
            &manifest_allowing("example.safe-github", "stagehand.observe_url"),
        )
        .expect("outcome");

    assert_eq!(outcome.status, ActionStatus::Blocked);
    assert_eq!(outcome.detail, "plugin_permission_denied");
    assert!(audit
        .events()
        .iter()
        .any(|event| event == "governance.manifest.denied"));
}

#[test]
fn denies_unknown_stagehand_capability_fail_closed() {
    let mut policy = StaticPolicyEngine::default();
    policy.allow_capability("stagehand", "demo", "stagehand.superpower");

    let audit = MemoryAuditSink::default();
    let runtime = OrchestratorRuntime::new(policy, audit.clone(), DryRunExecutor);
    let outcome = runtime
        .handle_action_with_manifest(
            request_for("stagehand", "stagehand.superpower"),
            &manifest_allowing("stagehand", "stagehand.superpower"),
        )
        .expect("outcome");

    assert_eq!(outcome.status, ActionStatus::Blocked);
    assert_eq!(outcome.detail, "manifest_stagehand_capability_unknown");
    assert!(audit
        .events()
        .iter()
        .any(|event| event == "governance.manifest.denied"));
}

#[test]
fn denies_manifest_schema_version_mismatch() {
    let mut policy = StaticPolicyEngine::default();
    policy.allow_capability("example.safe-github", "demo", "repo.read");

    let audit = MemoryAuditSink::default();
    let runtime = OrchestratorRuntime::new(policy, audit.clone(), DryRunExecutor);
    let outcome = runtime
        .handle_action_with_manifest(
            request_for("example.safe-github", "repo.read"),
            &CapabilityManifest {
                schema_version: 2,
                plugin: "example.safe-github".to_string(),
                capabilities: vec![DelegationCapability {
                    id: "repo.read".to_string(),
                    scope: vec!["project".to_string()],
                }],
            },
        )
        .expect("outcome");

    assert_eq!(outcome.status, ActionStatus::Blocked);
    assert_eq!(outcome.detail, "manifest_schema_version_unsupported");
    assert!(audit
        .events()
        .iter()
        .any(|event| event == "governance.manifest.denied"));
}

#[test]
fn denies_scope_not_granted_by_manifest() {
    let mut policy = StaticPolicyEngine::default();
    policy.allow_capability("example.safe-github", "demo", "repo.read");

    let audit = MemoryAuditSink::default();
    let runtime = OrchestratorRuntime::new(policy, audit.clone(), DryRunExecutor);
    let outcome = runtime
        .handle_action_with_manifest(
            request_for_with_scope("example.safe-github", "repo.read", &["tenant"]),
            &manifest_allowing("example.safe-github", "repo.read"),
        )
        .expect("outcome");

    assert_eq!(outcome.status, ActionStatus::Blocked);
    assert_eq!(outcome.detail, "manifest_scope_not_granted");
    assert!(audit
        .events()
        .iter()
        .any(|event| event == "governance.manifest.denied"));
}

#[test]
fn denies_empty_request_scope_when_manifest_scope_is_constrained() {
    let mut policy = StaticPolicyEngine::default();
    policy.allow_capability("example.safe-github", "demo", "repo.read");

    let audit = MemoryAuditSink::default();
    let runtime = OrchestratorRuntime::new(policy, audit.clone(), DryRunExecutor);
    let outcome = runtime
        .handle_action_with_manifest(
            request_for_with_scope("example.safe-github", "repo.read", &[]),
            &manifest_allowing("example.safe-github", "repo.read"),
        )
        .expect("outcome");

    assert_eq!(outcome.status, ActionStatus::Blocked);
    assert_eq!(outcome.detail, "manifest_scope_not_granted");
    assert!(audit
        .events()
        .iter()
        .any(|event| event == "governance.manifest.denied"));
}

#[test]
fn emits_manifest_validated_and_capability_used_events_on_success() {
    let mut policy = StaticPolicyEngine::default();
    policy.allow_capability("example.safe-github", "demo", "repo.read");

    let audit = MemoryAuditSink::default();
    let runtime = OrchestratorRuntime::new(policy, audit.clone(), DryRunExecutor);
    let outcome = runtime
        .handle_action_with_manifest(
            request_for("example.safe-github", "repo.read"),
            &manifest_allowing("example.safe-github", "repo.read"),
        )
        .expect("outcome");

    assert_eq!(outcome.status, ActionStatus::Executed);

    let events = audit.events();
    assert!(events
        .iter()
        .any(|event| event == "governance.manifest.validated"));
    assert!(events
        .iter()
        .any(|event| event == "governance.capability.used"));
}

#[test]
fn executes_stagehand_observe_domain_with_domain_input() {
    let mut policy = StaticPolicyEngine::default();
    policy.allow_capability("stagehand", "demo", "stagehand.observe_domain");

    let audit = MemoryAuditSink::default();
    let runtime = OrchestratorRuntime::new(policy, audit.clone(), DryRunExecutor);
    let outcome = runtime
        .handle_action_with_manifest(
            ActionRequest {
                request_id: "req-stagehand-observe-domain".to_string(),
                risk_tier: RiskTier::Safe,
                capability: CapabilityRequest {
                    plugin: "stagehand".to_string(),
                    project: "demo".to_string(),
                    capability: "stagehand.observe_domain".to_string(),
                    scope: vec!["example.com".to_string()],
                    reason: "unit test".to_string(),
                },
                input: serde_json::json!({
                    "domain": "example.com"
                }),
            },
            &CapabilityManifest {
                schema_version: 1,
                plugin: "stagehand".to_string(),
                capabilities: vec![
                    DelegationCapability {
                        id: "stagehand.enabled".to_string(),
                        scope: vec![],
                    },
                    DelegationCapability {
                        id: "stagehand.observe_domain".to_string(),
                        scope: vec!["example.com".to_string()],
                    },
                ],
            },
        )
        .expect("outcome");

    assert_eq!(outcome.status, ActionStatus::Executed);
}
