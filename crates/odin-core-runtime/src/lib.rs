//! Core runtime contracts and baseline orchestration flow.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use odin_audit::{AuditError, AuditRecord, AuditSink};
use odin_governance::plugins::{
    stagehand_policy_from_envelope, Action as StagehandAction,
    PermissionDecision as StagehandPermissionDecision,
};
use odin_plugin_protocol::{
    ActionOutcome, ActionRequest, ActionStatus, CapabilityManifest, CapabilityRequest,
    EventEnvelope, PluginManifest, PluginPermissionEnvelope, PolicyDecision, RiskTier, TrustLevel,
};
use odin_policy_engine::{PolicyEngine, PolicyError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("policy failure: {0}")]
    Policy(String),
    #[error("audit failure: {0}")]
    Audit(String),
    #[error("execution failure: {0}")]
    Execution(String),
    #[error("plugin failure: {0}")]
    Plugin(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

impl From<PolicyError> for RuntimeError {
    fn from(value: PolicyError) -> Self {
        RuntimeError::Policy(value.to_string())
    }
}

impl From<AuditError> for RuntimeError {
    fn from(value: AuditError) -> Self {
        RuntimeError::Audit(value.to_string())
    }
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;

pub trait ActionExecutor: Send + Sync {
    fn execute(&self, request: &ActionRequest) -> RuntimeResult<Value>;
}

pub trait TaskIngress: Send + Sync {
    fn write_task_payload(&self, payload: &str) -> RuntimeResult<()>;
}

pub trait BackendState: Send + Sync {
    fn get_active_backend(&self) -> RuntimeResult<String>;
    fn set_active_backend(&self, target: &str, reason: &str) -> RuntimeResult<()>;
}

pub trait FailoverController: Send + Sync {
    fn attempt_failover(&self, active_backend: Option<&str>) -> RuntimeResult<()>;
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WatchdogTaskEnvelope {
    pub schema_version: u32,
    pub task_id: String,
    #[serde(rename = "type")]
    pub task_kind: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    pub payload: WatchdogTaskPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WatchdogTaskPayload {
    pub task_type: String,
    #[serde(default)]
    pub source_key: Option<String>,
    pub project: String,
    pub plugin: String,
    #[serde(default)]
    pub trigger: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginCapabilityRef {
    pub id: String,
    #[serde(default)]
    pub project: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PluginDirective {
    RequestCapability {
        capability: PluginCapabilityRef,
        #[serde(default)]
        reason: String,
        #[serde(default)]
        input: Value,
        #[serde(default)]
        risk_tier: Option<RiskTier>,
    },
    EnqueueTask {
        task_type: String,
        #[serde(default)]
        project: Option<String>,
        #[serde(default)]
        reason: Option<String>,
        #[serde(default)]
        payload: Value,
    },
    Noop,
}

pub trait PluginEventRunner: Send + Sync {
    fn dispatch_event(
        &self,
        plugin: &str,
        event: &EventEnvelope,
    ) -> RuntimeResult<Vec<PluginDirective>>;
}

#[derive(Clone, Debug)]
pub struct ExternalProcessPluginRunner {
    plugins_root: PathBuf,
}

impl ExternalProcessPluginRunner {
    pub fn new(plugins_root: impl Into<PathBuf>) -> Self {
        Self {
            plugins_root: plugins_root.into(),
        }
    }

    pub fn plugins_root(&self) -> &Path {
        &self.plugins_root
    }

    fn resolve_plugin_dir(&self, plugin_name: &str) -> RuntimeResult<PathBuf> {
        let normalized = plugin_name.replace('.', "-");
        let leaf = plugin_name.rsplit('.').next().unwrap_or(plugin_name);
        let candidates = [
            self.plugins_root.join(plugin_name),
            self.plugins_root.join(normalized),
            self.plugins_root.join(leaf),
        ];

        for candidate in candidates {
            if candidate.join("odin.plugin.yaml").exists() {
                return Ok(candidate);
            }
        }

        Err(RuntimeError::Plugin(format!(
            "plugin manifest not found for {plugin_name} under {}",
            self.plugins_root.display()
        )))
    }

    fn resolve_command(plugin_dir: &Path, command: &str) -> PathBuf {
        let cmd_path = Path::new(command);
        if cmd_path.is_absolute() {
            return cmd_path.to_path_buf();
        }
        if command.starts_with("./") || command.contains('/') {
            return plugin_dir.join(cmd_path);
        }
        cmd_path.to_path_buf()
    }

    fn load_manifest(plugin_dir: &Path) -> RuntimeResult<PluginManifest> {
        let manifest_path = plugin_dir.join("odin.plugin.yaml");
        let raw = fs::read_to_string(&manifest_path).map_err(|e| {
            RuntimeError::Plugin(format!(
                "failed reading manifest {}: {e}",
                manifest_path.display()
            ))
        })?;
        serde_yaml::from_str::<PluginManifest>(&raw)
            .map_err(|e| RuntimeError::Plugin(format!("manifest parse failed: {e}")))
    }
}

impl PluginEventRunner for ExternalProcessPluginRunner {
    fn dispatch_event(
        &self,
        plugin: &str,
        event: &EventEnvelope,
    ) -> RuntimeResult<Vec<PluginDirective>> {
        let plugin_dir = self.resolve_plugin_dir(plugin)?;
        let manifest = Self::load_manifest(&plugin_dir)?;
        if manifest.plugin.name != plugin {
            return Err(RuntimeError::Plugin(format!(
                "plugin name mismatch: task requested {plugin}, manifest has {}",
                manifest.plugin.name
            )));
        }

        let command = Self::resolve_command(&plugin_dir, &manifest.plugin.entrypoint.command);
        let mut child = Command::new(command)
            .args(&manifest.plugin.entrypoint.args)
            .current_dir(&plugin_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| RuntimeError::Plugin(format!("failed to start plugin process: {e}")))?;

        if let Some(stdin) = child.stdin.as_mut() {
            let event_json = serde_json::to_string(event)
                .map_err(|e| RuntimeError::Plugin(format!("event serialization failed: {e}")))?;
            stdin.write_all(event_json.as_bytes()).map_err(|e| {
                RuntimeError::Plugin(format!("failed to write event to plugin: {e}"))
            })?;
            stdin
                .write_all(b"\n")
                .map_err(|e| RuntimeError::Plugin(format!("failed to flush plugin event: {e}")))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| RuntimeError::Plugin(format!("plugin wait failed: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).replace('\n', " ");
            return Err(RuntimeError::Plugin(format!(
                "plugin process failed (exit={}): {}",
                output.status, stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut directives = Vec::new();
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let directive = serde_json::from_str::<PluginDirective>(line).map_err(|e| {
                RuntimeError::Plugin(format!("invalid plugin directive output: {e}; line={line}"))
            })?;
            directives.push(directive);
        }

        if directives.is_empty() {
            directives.push(PluginDirective::Noop);
        }
        Ok(directives)
    }
}

#[derive(Clone, Debug, Default)]
pub struct DryRunExecutor;

impl ActionExecutor for DryRunExecutor {
    fn execute(&self, request: &ActionRequest) -> RuntimeResult<Value> {
        Ok(serde_json::json!({
            "request_id": request.request_id,
            "result": "dry_run",
            "capability": request.capability.capability,
            "plugin": request.capability.plugin
        }))
    }
}

pub struct OrchestratorRuntime<P, A, E>
where
    P: PolicyEngine,
    A: AuditSink,
    E: ActionExecutor,
{
    policy: P,
    audit: A,
    executor: E,
}

impl<P, A, E> OrchestratorRuntime<P, A, E>
where
    P: PolicyEngine,
    A: AuditSink,
    E: ActionExecutor,
{
    pub fn new(policy: P, audit: A, executor: E) -> Self {
        Self {
            policy,
            audit,
            executor,
        }
    }

    pub fn handle_action(&self, request: ActionRequest) -> RuntimeResult<ActionOutcome> {
        let decision = self.evaluate_policy(&request)?;
        match decision {
            PolicyDecision::Deny { reason_code } => Ok(ActionOutcome {
                request_id: request.request_id,
                status: ActionStatus::Blocked,
                detail: reason_code,
                output: Value::Null,
            }),
            PolicyDecision::RequireApproval { reason_code, .. } => Ok(ActionOutcome {
                request_id: request.request_id,
                status: ActionStatus::ApprovalPending,
                detail: reason_code,
                output: Value::Null,
            }),
            PolicyDecision::Allow { .. } => {
                let output = self.executor.execute(&request)?;
                self.audit.record(AuditRecord {
                    ts_unix: now_unix(),
                    event_type: "action.executed".to_string(),
                    request_id: Some(request.request_id.clone()),
                    task_id: None,
                    project: Some(request.capability.project.clone()),
                    metadata: serde_json::json!({
                        "plugin": request.capability.plugin,
                        "capability": request.capability.capability
                    }),
                })?;

                Ok(ActionOutcome {
                    request_id: request.request_id,
                    status: ActionStatus::Executed,
                    detail: "executed".to_string(),
                    output,
                })
            }
        }
    }

    pub fn handle_action_with_manifest(
        &self,
        request: ActionRequest,
        manifest: &CapabilityManifest,
    ) -> RuntimeResult<ActionOutcome> {
        validate_capability(&request.capability)?;
        let manifest_denial = if manifest.schema_version != 1 {
            Some("manifest_schema_version_unsupported".to_string())
        } else {
            manifest_denial_reason(&request, manifest)
        };
        if let Some(reason_code) = manifest_denial {
            self.audit.record(AuditRecord {
                ts_unix: now_unix(),
                event_type: "governance.manifest.denied".to_string(),
                request_id: Some(request.request_id.clone()),
                task_id: None,
                project: Some(request.capability.project.clone()),
                metadata: serde_json::json!({
                    "plugin": request.capability.plugin,
                    "manifest_plugin": manifest.plugin,
                    "capability": request.capability.capability,
                    "reason_code": reason_code
                }),
            })?;
            return Ok(ActionOutcome {
                request_id: request.request_id,
                status: ActionStatus::Blocked,
                detail: reason_code,
                output: Value::Null,
            });
        }

        self.audit.record(AuditRecord {
            ts_unix: now_unix(),
            event_type: "governance.manifest.validated".to_string(),
            request_id: Some(request.request_id.clone()),
            task_id: None,
            project: Some(request.capability.project.clone()),
            metadata: serde_json::json!({
                "plugin": request.capability.plugin,
                "manifest_plugin": manifest.plugin,
                "capability": request.capability.capability
            }),
        })?;

        let request_id = request.request_id.clone();
        let project = request.capability.project.clone();
        let plugin = request.capability.plugin.clone();
        let capability = request.capability.capability.clone();
        let outcome = self.handle_action(request)?;
        if outcome.status == ActionStatus::Executed {
            self.audit.record(AuditRecord {
                ts_unix: now_unix(),
                event_type: "governance.capability.used".to_string(),
                request_id: Some(request_id),
                task_id: None,
                project: Some(project),
                metadata: serde_json::json!({
                    "plugin": plugin,
                    "capability": capability
                }),
            })?;
        }

        Ok(outcome)
    }

    pub fn handle_watchdog_task<R, T>(
        &self,
        raw_task: &str,
        runner: &R,
        ingress: &T,
    ) -> RuntimeResult<Vec<ActionOutcome>>
    where
        R: PluginEventRunner,
        T: TaskIngress,
    {
        let task = parse_watchdog_task(raw_task)?;
        let event = EventEnvelope {
            event_id: format!("evt-{}-{}", task.task_id, now_unix()),
            event_type: "task.received".to_string(),
            task_id: Some(task.task_id.clone()),
            request_id: None,
            project: Some(task.payload.project.clone()),
            payload: serde_json::json!({
                "task_type": task.payload.task_type,
                "source_key": task.payload.source_key,
                "trigger": task.payload.trigger
            }),
        };

        let directives = runner.dispatch_event(&task.payload.plugin, &event)?;
        let mut outcomes = Vec::new();

        for (idx, directive) in directives.into_iter().enumerate() {
            match directive {
                PluginDirective::RequestCapability {
                    capability,
                    reason,
                    input,
                    risk_tier,
                } => {
                    let project = capability
                        .project
                        .unwrap_or_else(|| task.payload.project.clone());
                    let request = ActionRequest {
                        request_id: format!("{}-{}-cap", task.task_id, idx),
                        risk_tier: risk_tier.unwrap_or(RiskTier::Safe),
                        capability: CapabilityRequest {
                            plugin: task.payload.plugin.clone(),
                            project,
                            capability: capability.id,
                            scope: vec!["project".to_string()],
                            reason: if reason.trim().is_empty() {
                                "plugin requested capability".to_string()
                            } else {
                                reason
                            },
                        },
                        input,
                    };
                    outcomes.push(self.handle_action(request)?);
                }
                PluginDirective::EnqueueTask {
                    task_type,
                    project,
                    reason,
                    payload,
                } => {
                    if task_type.trim().is_empty() {
                        return Err(RuntimeError::InvalidInput(
                            "enqueue_task requires non-empty task_type".to_string(),
                        ));
                    }
                    let project = project.unwrap_or_else(|| task.payload.project.clone());
                    let request = ActionRequest {
                        request_id: format!("{}-{}-enqueue", task.task_id, idx),
                        risk_tier: RiskTier::Sensitive,
                        capability: CapabilityRequest {
                            plugin: task.payload.plugin.clone(),
                            project: project.clone(),
                            capability: "task.enqueue".to_string(),
                            scope: vec!["project".to_string()],
                            reason: reason.unwrap_or_else(|| {
                                format!("plugin enqueue request for {}", task_type)
                            }),
                        },
                        input: serde_json::json!({
                            "task_type": task_type,
                            "origin_task_id": task.task_id
                        }),
                    };

                    match self.evaluate_policy(&request)? {
                        PolicyDecision::Deny { reason_code } => outcomes.push(ActionOutcome {
                            request_id: request.request_id,
                            status: ActionStatus::Blocked,
                            detail: reason_code,
                            output: Value::Null,
                        }),
                        PolicyDecision::RequireApproval { reason_code, .. } => {
                            outcomes.push(ActionOutcome {
                                request_id: request.request_id,
                                status: ActionStatus::ApprovalPending,
                                detail: reason_code,
                                output: Value::Null,
                            })
                        }
                        PolicyDecision::Allow { .. } => {
                            let queued = build_enqueued_task(
                                &task,
                                idx,
                                &task_type,
                                &project,
                                payload.clone(),
                            );
                            let queued_json = serde_json::to_string(&queued).map_err(|e| {
                                RuntimeError::InvalidInput(format!(
                                    "failed serializing enqueued task: {e}"
                                ))
                            })?;
                            ingress.write_task_payload(&queued_json)?;

                            self.audit.record(AuditRecord {
                                ts_unix: now_unix(),
                                event_type: "task.enqueued".to_string(),
                                request_id: Some(request.request_id.clone()),
                                task_id: Some(task.task_id.clone()),
                                project: Some(project.clone()),
                                metadata: serde_json::json!({
                                    "plugin": task.payload.plugin,
                                    "task_type": task_type,
                                    "origin_task_id": task.task_id
                                }),
                            })?;

                            outcomes.push(ActionOutcome {
                                request_id: request.request_id,
                                status: ActionStatus::Executed,
                                detail: "task_enqueued".to_string(),
                                output: serde_json::json!({
                                    "task_type": task_type,
                                    "project": project
                                }),
                            });
                        }
                    }
                }
                PluginDirective::Noop => {
                    self.audit.record(AuditRecord {
                        ts_unix: now_unix(),
                        event_type: "plugin.noop".to_string(),
                        request_id: None,
                        task_id: Some(task.task_id.clone()),
                        project: Some(task.payload.project.clone()),
                        metadata: serde_json::json!({
                            "plugin": task.payload.plugin
                        }),
                    })?;
                }
            }
        }

        Ok(outcomes)
    }

    fn evaluate_policy(&self, request: &ActionRequest) -> RuntimeResult<PolicyDecision> {
        validate_capability(&request.capability)?;
        let decision = self.policy.decide(request)?;
        self.audit.record(AuditRecord {
            ts_unix: now_unix(),
            event_type: "policy.decision".to_string(),
            request_id: Some(request.request_id.clone()),
            task_id: None,
            project: Some(request.capability.project.clone()),
            metadata: serde_json::json!({
                "plugin": request.capability.plugin,
                "capability": request.capability.capability,
                "decision": decision_tag(&decision)
            }),
        })?;
        Ok(decision)
    }
}

fn parse_watchdog_task(raw_task: &str) -> RuntimeResult<WatchdogTaskEnvelope> {
    let task: WatchdogTaskEnvelope = serde_json::from_str(raw_task)
        .map_err(|e| RuntimeError::InvalidInput(format!("invalid watchdog task JSON: {e}")))?;

    if task.schema_version != 1 {
        return Err(RuntimeError::InvalidInput(format!(
            "unsupported schema_version: {}",
            task.schema_version
        )));
    }
    if task.task_kind != "watchdog_poll" {
        return Err(RuntimeError::InvalidInput(format!(
            "unsupported task type: {}",
            task.task_kind
        )));
    }
    if task.payload.plugin.trim().is_empty() {
        return Err(RuntimeError::InvalidInput(
            "watchdog task payload.plugin is required".to_string(),
        ));
    }
    if task.payload.project.trim().is_empty() {
        return Err(RuntimeError::InvalidInput(
            "watchdog task payload.project is required".to_string(),
        ));
    }
    if task.payload.task_type.trim().is_empty() {
        return Err(RuntimeError::InvalidInput(
            "watchdog task payload.task_type is required".to_string(),
        ));
    }

    Ok(task)
}

fn build_enqueued_task(
    origin: &WatchdogTaskEnvelope,
    sequence: usize,
    task_type: &str,
    project: &str,
    payload: Value,
) -> Value {
    let followup_task_id = format!("{}-followup-{}-{}", origin.task_id, sequence, now_unix());
    serde_json::json!({
        "schema_version": 1,
        "task_id": followup_task_id,
        "type": task_type,
        "source": "plugin",
        "created_at_unix": now_unix(),
        "payload": {
            "project": project,
            "plugin": origin.payload.plugin,
            "task_type": task_type,
            "origin_task_id": origin.task_id,
            "data": payload
        }
    })
}

fn validate_capability(capability: &CapabilityRequest) -> RuntimeResult<()> {
    if capability.plugin.trim().is_empty() {
        return Err(RuntimeError::InvalidInput("plugin is required".to_string()));
    }
    if capability.capability.trim().is_empty() {
        return Err(RuntimeError::InvalidInput(
            "capability is required".to_string(),
        ));
    }
    Ok(())
}

fn manifest_denial_reason(
    request: &ActionRequest,
    manifest: &CapabilityManifest,
) -> Option<String> {
    if manifest.plugin != request.capability.plugin {
        return Some("manifest_plugin_mismatch".to_string());
    }

    let matching_capabilities = manifest
        .capabilities
        .iter()
        .filter(|capability| capability.id == request.capability.capability)
        .collect::<Vec<_>>();
    if matching_capabilities.is_empty() {
        return Some("manifest_capability_not_granted".to_string());
    }
    if !matching_capabilities
        .iter()
        .any(|granted| manifest_scope_permits(&request.capability.scope, &granted.scope))
    {
        return Some("manifest_scope_not_granted".to_string());
    }

    let capability = request.capability.capability.as_str();
    if is_stagehand_capability(capability) && request.capability.plugin != "stagehand" {
        return Some("plugin_permission_denied".to_string());
    }

    stagehand_permission_denial(capability, &request.input, manifest)
}

fn stagehand_permission_denial(
    capability: &str,
    input: &Value,
    manifest: &CapabilityManifest,
) -> Option<String> {
    if manifest.plugin != "stagehand" {
        return None;
    }

    let action = match stagehand_action_from_capability(capability, input) {
        Some(action) => action,
        None if capability.starts_with("stagehand.") => {
            return Some("manifest_stagehand_capability_unknown".to_string())
        }
        None => return None,
    };
    let policy = stagehand_policy_from_envelope(&PluginPermissionEnvelope {
        plugin: manifest.plugin.clone(),
        trust_level: TrustLevel::Caution,
        permissions: manifest.capabilities.clone(),
    });
    match policy.evaluate(action) {
        StagehandPermissionDecision::Allow { .. } => None,
        StagehandPermissionDecision::Deny { reason_code } => Some(reason_code),
    }
}

fn stagehand_action_from_capability(capability: &str, input: &Value) -> Option<StagehandAction> {
    match capability {
        "browser.observe" | "stagehand.observe_url" | "stagehand.observe_domain" => Some(
            StagehandAction::ObserveUrl(input_string(input, "url").unwrap_or_default()),
        ),
        "workspace.read" | "stagehand.workspace.read" => Some(StagehandAction::ReadWorkspace(
            input_string(input, "workspace").unwrap_or_default(),
        )),
        "command.run" | "stagehand.command.run" => Some(StagehandAction::RunCommand(
            input_string(input, "command").unwrap_or_default(),
        )),
        "stagehand.login" => Some(StagehandAction::Login),
        "stagehand.payment" => Some(StagehandAction::Payment),
        "stagehand.pii_submit" => Some(StagehandAction::PiiSubmit),
        "stagehand.file_upload" => Some(StagehandAction::FileUpload),
        _ => None,
    }
}

fn is_stagehand_capability(capability: &str) -> bool {
    matches!(
        capability,
        "browser.observe" | "workspace.read" | "command.run"
    ) || capability.starts_with("stagehand.")
}

fn input_string(input: &Value, key: &str) -> Option<String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn manifest_scope_permits(requested_scope: &[String], granted_scope: &[String]) -> bool {
    if requested_scope.is_empty() {
        return granted_scope.is_empty();
    }
    if granted_scope.is_empty() {
        return false;
    }
    requested_scope
        .iter()
        .all(|requested| granted_scope.iter().any(|granted| granted == requested))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn decision_tag(decision: &PolicyDecision) -> &'static str {
    match decision {
        PolicyDecision::Allow { .. } => "allow",
        PolicyDecision::Deny { .. } => "deny",
        PolicyDecision::RequireApproval { .. } => "require_approval",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use odin_audit::{AuditRecord, AuditSink};
    use odin_plugin_protocol::{ActionRequest, CapabilityRequest, RiskTier};
    use odin_policy_engine::StaticPolicyEngine;

    use super::{
        ActionExecutor, OrchestratorRuntime, PluginCapabilityRef, PluginDirective,
        PluginEventRunner, RuntimeError, TaskIngress,
    };

    #[derive(Default)]
    struct MemoryAuditSink(Mutex<Vec<AuditRecord>>);

    impl AuditSink for MemoryAuditSink {
        fn record(&self, record: AuditRecord) -> Result<(), odin_audit::AuditError> {
            self.0
                .lock()
                .map_err(|_| odin_audit::AuditError::Write("poisoned lock".to_string()))?
                .push(record);
            Ok(())
        }
    }

    #[derive(Default)]
    struct MemoryIngress(Mutex<Vec<String>>);

    impl TaskIngress for MemoryIngress {
        fn write_task_payload(&self, payload: &str) -> Result<(), RuntimeError> {
            self.0
                .lock()
                .map_err(|_| RuntimeError::Execution("poisoned lock".to_string()))?
                .push(payload.to_string());
            Ok(())
        }
    }

    #[derive(Clone)]
    struct StubRunner {
        directives: Vec<PluginDirective>,
    }

    impl PluginEventRunner for StubRunner {
        fn dispatch_event(
            &self,
            _plugin: &str,
            _event: &odin_plugin_protocol::EventEnvelope,
        ) -> Result<Vec<PluginDirective>, RuntimeError> {
            Ok(self.directives.clone())
        }
    }

    struct FailingExecutor;

    impl ActionExecutor for FailingExecutor {
        fn execute(&self, _request: &ActionRequest) -> Result<serde_json::Value, RuntimeError> {
            Err(RuntimeError::Execution("boom".to_string()))
        }
    }

    fn request() -> ActionRequest {
        ActionRequest {
            request_id: "req-1".to_string(),
            risk_tier: RiskTier::Safe,
            capability: CapabilityRequest {
                plugin: "example.safe-github".to_string(),
                project: "demo".to_string(),
                capability: "repo.read".to_string(),
                scope: vec!["project".to_string()],
                reason: "unit test".to_string(),
            },
            input: serde_json::Value::Null,
        }
    }

    fn watchdog_task() -> String {
        serde_json::json!({
            "schema_version": 1,
            "task_id": "watchdog-poll-sentry-123",
            "type": "watchdog_poll",
            "source": "keepalive",
            "created_at": "2026-02-25T00:00:00Z",
            "payload": {
                "task_type": "watchdog.sentry.poll",
                "source_key": "sentry-check",
                "project": "private",
                "plugin": "private.ops-watchdog",
                "trigger": "feature_gate"
            }
        })
        .to_string()
    }

    #[test]
    fn blocked_when_not_granted() {
        let runtime = OrchestratorRuntime::new(
            StaticPolicyEngine::default(),
            MemoryAuditSink::default(),
            super::DryRunExecutor,
        );
        let outcome = runtime.handle_action(request()).expect("outcome");
        assert_eq!(outcome.status, odin_plugin_protocol::ActionStatus::Blocked);
    }

    #[test]
    fn execution_failure_bubbles_up() {
        let mut policy = StaticPolicyEngine::default();
        policy.allow_capability("example.safe-github", "demo", "repo.read");

        let runtime = OrchestratorRuntime::new(policy, MemoryAuditSink::default(), FailingExecutor);
        let err = runtime
            .handle_action(request())
            .expect_err("expected execution error");
        assert!(matches!(err, RuntimeError::Execution(_)));
    }

    #[test]
    fn watchdog_request_capability_routed() {
        let mut policy = StaticPolicyEngine::default();
        policy.allow_capability("private.ops-watchdog", "private", "monitoring.sentry.read");

        let runtime =
            OrchestratorRuntime::new(policy, MemoryAuditSink::default(), super::DryRunExecutor);
        let ingress = MemoryIngress::default();
        let runner = StubRunner {
            directives: vec![PluginDirective::RequestCapability {
                capability: PluginCapabilityRef {
                    id: "monitoring.sentry.read".to_string(),
                    project: None,
                },
                reason: "poll sentry".to_string(),
                input: serde_json::json!({"sample": true}),
                risk_tier: Some(RiskTier::Safe),
            }],
        };

        let outcomes = runtime
            .handle_watchdog_task(&watchdog_task(), &runner, &ingress)
            .expect("watchdog outcome");
        assert_eq!(outcomes.len(), 1);
        assert_eq!(
            outcomes[0].status,
            odin_plugin_protocol::ActionStatus::Executed
        );
        assert_eq!(
            outcomes[0]
                .output
                .get("capability")
                .and_then(|v| v.as_str()),
            Some("monitoring.sentry.read")
        );
    }

    #[test]
    fn watchdog_enqueue_task_writes_ingress() {
        let mut policy = StaticPolicyEngine::default();
        policy.allow_capability("private.ops-watchdog", "private", "task.enqueue");

        let runtime =
            OrchestratorRuntime::new(policy, MemoryAuditSink::default(), super::DryRunExecutor);
        let ingress = MemoryIngress::default();
        let runner = StubRunner {
            directives: vec![PluginDirective::EnqueueTask {
                task_type: "watchdog.remediation.dispatch".to_string(),
                project: None,
                reason: Some("queue remediation".to_string()),
                payload: serde_json::json!({"kind": "critical_issue"}),
            }],
        };

        let outcomes = runtime
            .handle_watchdog_task(&watchdog_task(), &runner, &ingress)
            .expect("watchdog outcome");
        assert_eq!(outcomes.len(), 1);
        assert_eq!(
            outcomes[0].status,
            odin_plugin_protocol::ActionStatus::Executed
        );

        let writes = ingress.0.lock().expect("lock");
        assert_eq!(writes.len(), 1);
        let queued: serde_json::Value = serde_json::from_str(&writes[0]).expect("queued json");
        assert_eq!(
            queued.get("type").and_then(|v| v.as_str()),
            Some("watchdog.remediation.dispatch")
        );
    }

    #[test]
    fn watchdog_noop_routes_without_outcome() {
        let runtime = OrchestratorRuntime::new(
            StaticPolicyEngine::default(),
            MemoryAuditSink::default(),
            super::DryRunExecutor,
        );
        let ingress = MemoryIngress::default();
        let runner = StubRunner {
            directives: vec![PluginDirective::Noop],
        };

        let outcomes = runtime
            .handle_watchdog_task(&watchdog_task(), &runner, &ingress)
            .expect("watchdog outcome");
        assert!(outcomes.is_empty());
        let writes = ingress.0.lock().expect("lock");
        assert!(writes.is_empty());
    }
}
