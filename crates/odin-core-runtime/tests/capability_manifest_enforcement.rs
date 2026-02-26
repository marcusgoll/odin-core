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
