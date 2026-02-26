use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context};
use odin_audit::NoopAuditSink;
use odin_compat_bash::{
    BashBackendStateAdapter, BashFailoverAdapter, BashTaskIngressAdapter, LegacyScriptPaths,
};
use odin_core_runtime::{
    BackendState, DryRunExecutor, ExternalProcessPluginRunner, OrchestratorRuntime, TaskIngress,
};
use odin_governance::import::{evaluate_install, Ack, InstallGateStatus, SkillImportCandidate};
use odin_governance::plugins::{stagehand_policy_from_envelope, Action, PermissionDecision};
use odin_governance::risk_scan::{RiskCategory, RiskFinding};
use odin_governance::skills;
use odin_plugin_protocol::{
    ActionRequest, CapabilityRequest, DelegationCapability, PluginPermissionEnvelope, RiskTier,
    SkillRecord, SkillScope, TrustLevel,
};
use odin_policy_engine::StaticPolicyEngine;
use serde_json::json;

#[derive(Clone, Debug)]
struct CliConfig {
    config_path: String,
    legacy_root: Option<PathBuf>,
    legacy_odin_dir: PathBuf,
    plugins_root: PathBuf,
    task_file: Option<PathBuf>,
    run_once: bool,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            config_path: "config/default.yaml".to_string(),
            legacy_root: None,
            legacy_odin_dir: PathBuf::from("/var/odin"),
            plugins_root: PathBuf::from("examples/private-plugins"),
            task_file: None,
            run_once: false,
        }
    }
}

fn parse_cli_config() -> CliConfig {
    let mut cfg = CliConfig::default();
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                if let Some(path) = args.next() {
                    cfg.config_path = path;
                }
            }
            "--legacy-root" => {
                if let Some(path) = args.next() {
                    cfg.legacy_root = Some(PathBuf::from(path));
                }
            }
            "--legacy-odin-dir" => {
                if let Some(path) = args.next() {
                    cfg.legacy_odin_dir = PathBuf::from(path);
                }
            }
            "--plugins-root" => {
                if let Some(path) = args.next() {
                    cfg.plugins_root = PathBuf::from(path);
                }
            }
            "--task-file" => {
                if let Some(path) = args.next() {
                    cfg.task_file = Some(PathBuf::from(path));
                }
            }
            "--run-once" => {
                cfg.run_once = true;
            }
            _ => {}
        }
    }

    cfg
}

fn sample_action_request() -> ActionRequest {
    ActionRequest {
        request_id: "bootstrap-request-1".to_string(),
        risk_tier: RiskTier::Safe,
        capability: CapabilityRequest {
            plugin: "example.safe-github".to_string(),
            project: "bootstrap".to_string(),
            capability: "repo.read".to_string(),
            scope: vec!["project".to_string()],
            reason: "bootstrap health check".to_string(),
        },
        input: json!({"probe": true}),
    }
}

fn emit_governance_summary(summary: serde_json::Value) -> anyhow::Result<()> {
    let rendered = serde_json::to_string_pretty(&summary)
        .context("failed to format governance summary JSON")?;
    println!("{rendered}");
    Ok(())
}

fn scope_name(scope: SkillScope) -> &'static str {
    match scope {
        SkillScope::Global => "global",
        SkillScope::Project => "project",
        SkillScope::User => "user",
    }
}

#[derive(Clone, Debug)]
struct GovernanceParseError {
    error_code: &'static str,
    detail: String,
}

impl GovernanceParseError {
    fn new(error_code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            error_code,
            detail: detail.into(),
        }
    }
}

fn governance_error(
    command: &str,
    error_code: &str,
    detail: impl Into<String>,
) -> anyhow::Result<()> {
    let detail = detail.into();
    emit_governance_summary(json!({
        "command": command,
        "status": "error",
        "error_code": error_code,
        "detail": detail,
    }))?;
    Err(anyhow!(detail))
}

fn governance_parse_error(command: &str, err: GovernanceParseError) -> anyhow::Result<()> {
    governance_error(command, err.error_code, err.detail)
}

fn parse_scope(value: &str) -> Result<SkillScope, GovernanceParseError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "global" => Ok(SkillScope::Global),
        "project" => Ok(SkillScope::Project),
        "user" => Ok(SkillScope::User),
        other => Err(GovernanceParseError::new(
            "invalid_scope",
            format!("invalid scope: {other}"),
        )),
    }
}

fn parse_trust_level(value: &str) -> Result<TrustLevel, GovernanceParseError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "trusted" => Ok(TrustLevel::Trusted),
        "caution" => Ok(TrustLevel::Caution),
        "untrusted" => Ok(TrustLevel::Untrusted),
        other => Err(GovernanceParseError::new(
            "invalid_trust_level",
            format!("invalid trust level: {other}"),
        )),
    }
}

fn next_arg_value(
    args: &[String],
    index: &mut usize,
    flag: &str,
) -> Result<String, GovernanceParseError> {
    *index += 1;
    match args.get(*index) {
        Some(value) if !value.starts_with("--") => Ok(value.clone()),
        _ => Err(GovernanceParseError::new(
            "missing_required_value",
            format!("missing value for {flag}"),
        )),
    }
}

fn default_registry_path(scope: SkillScope) -> PathBuf {
    match scope {
        SkillScope::Project => PathBuf::from("config/skills.project.yaml"),
        SkillScope::User => PathBuf::from("config/skills.user.yaml"),
        SkillScope::Global => PathBuf::from("config/skills.global.yaml"),
    }
}

fn load_registry(
    path: &Path,
    scope: SkillScope,
) -> anyhow::Result<odin_plugin_protocol::SkillRegistry> {
    skills::load_scoped_registry(path, scope).map_err(|err| anyhow!(err.to_string()))
}

fn risk_category_name(category: &RiskCategory) -> &'static str {
    match category {
        RiskCategory::Shell => "shell",
        RiskCategory::Network => "network",
        RiskCategory::Secret => "secret",
        RiskCategory::Delete => "delete",
    }
}

fn risk_findings_json(findings: &[RiskFinding]) -> Vec<serde_json::Value> {
    findings
        .iter()
        .map(|finding| {
            json!({
                "category": risk_category_name(&finding.category),
                "pattern": finding.pattern,
            })
        })
        .collect()
}

fn decision_name(decision: &PermissionDecision) -> &'static str {
    match decision {
        PermissionDecision::Allow { .. } => "allow",
        PermissionDecision::Deny { .. } => "deny",
    }
}

fn decision_reason(decision: &PermissionDecision) -> &str {
    match decision {
        PermissionDecision::Allow { reason_code } | PermissionDecision::Deny { reason_code } => {
            reason_code.as_str()
        }
    }
}

fn append_csv_values(target: &mut Vec<String>, raw: &str) {
    for item in raw.split(',') {
        let value = item.trim();
        if !value.is_empty() {
            target.push(value.to_string());
        }
    }
}

fn normalize_domain_probe_input(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}/")
    }
}

fn governance_subcommand_args<'a>(argv: &'a [String]) -> Option<&'a [String]> {
    let mut index = 1;
    while index < argv.len() {
        match argv[index].as_str() {
            "governance" => return Some(&argv[index + 1..]),
            "--config" | "--legacy-root" | "--legacy-odin-dir" | "--plugins-root"
            | "--task-file" => {
                index += 2;
            }
            "--run-once" => {
                index += 1;
            }
            other if other.starts_with("--") => {
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }
    None
}

fn governance_discover(args: &[String]) -> anyhow::Result<()> {
    let mut scope = SkillScope::Project;
    let mut registry_path: Option<PathBuf> = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--scope" => {
                let value = match next_arg_value(args, &mut index, "--scope") {
                    Ok(value) => value,
                    Err(err) => return governance_parse_error("discover", err),
                };
                scope = match parse_scope(&value) {
                    Ok(scope) => scope,
                    Err(err) => return governance_parse_error("discover", err),
                };
            }
            "--registry" => {
                let value = match next_arg_value(args, &mut index, "--registry") {
                    Ok(value) => value,
                    Err(err) => return governance_parse_error("discover", err),
                };
                registry_path = Some(PathBuf::from(value));
            }
            "--run-once" => {}
            other => {
                let summary = json!({
                    "command": "discover",
                    "status": "error",
                    "error_code": "invalid_argument",
                    "detail": format!("unsupported argument: {other}"),
                });
                emit_governance_summary(summary)?;
                return Err(anyhow!("unsupported argument"));
            }
        }

        index += 1;
    }

    let registry_path = registry_path.unwrap_or_else(|| default_registry_path(scope.clone()));

    match load_registry(&registry_path, scope.clone()) {
        Ok(registry) => emit_governance_summary(json!({
            "command": "discover",
            "status": "ok",
            "scope": scope_name(scope),
            "registry": registry_path.display().to_string(),
            "candidates": registry.skills,
        })),
        Err(err) => {
            let summary = json!({
                "command": "discover",
                "status": "error",
                "scope": scope_name(scope),
                "registry": registry_path.display().to_string(),
                "error_code": "registry_load_failed",
                "detail": err.to_string(),
            });
            emit_governance_summary(summary)?;
            Err(err)
        }
    }
}

fn governance_install(args: &[String]) -> anyhow::Result<()> {
    let mut skill_name: Option<String> = None;
    let mut trust_level = TrustLevel::Untrusted;
    let mut source = "project:manual".to_string();
    let mut scripts: Vec<String> = Vec::new();
    let mut readme: Option<String> = None;
    let mut ack = Ack::None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--name" => {
                let value = match next_arg_value(args, &mut index, "--name") {
                    Ok(value) => value,
                    Err(err) => return governance_parse_error("install", err),
                };
                skill_name = Some(value);
            }
            "--trust-level" => {
                let value = match next_arg_value(args, &mut index, "--trust-level") {
                    Ok(value) => value,
                    Err(err) => return governance_parse_error("install", err),
                };
                trust_level = match parse_trust_level(&value) {
                    Ok(level) => level,
                    Err(err) => return governance_parse_error("install", err),
                };
            }
            "--source" => {
                source = match next_arg_value(args, &mut index, "--source") {
                    Ok(value) => value,
                    Err(err) => return governance_parse_error("install", err),
                };
            }
            "--script" => {
                let value = match next_arg_value(args, &mut index, "--script") {
                    Ok(value) => value,
                    Err(err) => return governance_parse_error("install", err),
                };
                scripts.push(value);
            }
            "--readme" => {
                let value = match next_arg_value(args, &mut index, "--readme") {
                    Ok(value) => value,
                    Err(err) => return governance_parse_error("install", err),
                };
                readme = Some(value);
            }
            "--ack" => {
                ack = Ack::Accepted;
            }
            "--run-once" => {}
            other => {
                let summary = json!({
                    "command": "install",
                    "status": "error",
                    "error_code": "invalid_argument",
                    "detail": format!("unsupported argument: {other}"),
                });
                emit_governance_summary(summary)?;
                return Err(anyhow!("unsupported argument"));
            }
        }

        index += 1;
    }

    let Some(skill_name) = skill_name else {
        let summary = json!({
            "command": "install",
            "status": "error",
            "error_code": "missing_skill_name",
            "detail": "--name is required",
        });
        emit_governance_summary(summary)?;
        return Err(anyhow!("missing --name"));
    };

    let mut record = SkillRecord::default_for(skill_name.clone());
    record.trust_level = trust_level;
    record.source = source;

    let candidate = SkillImportCandidate {
        record,
        scripts,
        readme,
    };

    let plan = evaluate_install(&candidate, ack).map_err(|err| anyhow!(err.to_string()))?;
    let findings_json = risk_findings_json(&plan.findings);

    if matches!(plan.status, InstallGateStatus::BlockedAckRequired) {
        let summary = json!({
            "command": "install",
            "status": "blocked",
            "error_code": "ack_required",
            "skill": skill_name,
            "gate_status": "blocked_ack_required",
            "reasons": plan.reasons,
            "findings": findings_json,
        });
        emit_governance_summary(summary)?;
        return Err(anyhow!("ack required before install"));
    }

    emit_governance_summary(json!({
        "command": "install",
        "status": "ok",
        "skill": skill_name,
        "gate_status": "allowed",
        "reasons": plan.reasons,
        "findings": findings_json,
    }))
}

fn governance_enable_plugin(args: &[String]) -> anyhow::Result<()> {
    let mut plugin: Option<String> = None;
    let mut domains: Vec<String> = Vec::new();
    let mut workspaces: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--plugin" => {
                let value = match next_arg_value(args, &mut index, "--plugin") {
                    Ok(value) => value,
                    Err(err) => return governance_parse_error("enable-plugin", err),
                };
                plugin = Some(value);
            }
            "--domain" | "--domains" => {
                let raw = match next_arg_value(args, &mut index, "--domains") {
                    Ok(value) => value,
                    Err(err) => return governance_parse_error("enable-plugin", err),
                };
                append_csv_values(&mut domains, &raw);
            }
            "--workspace" | "--workspaces" => {
                let raw = match next_arg_value(args, &mut index, "--workspaces") {
                    Ok(value) => value,
                    Err(err) => return governance_parse_error("enable-plugin", err),
                };
                append_csv_values(&mut workspaces, &raw);
            }
            "--command" | "--commands" => {
                let raw = match next_arg_value(args, &mut index, "--commands") {
                    Ok(value) => value,
                    Err(err) => return governance_parse_error("enable-plugin", err),
                };
                append_csv_values(&mut commands, &raw);
            }
            "--run-once" => {}
            other => {
                let summary = json!({
                    "command": "enable-plugin",
                    "status": "error",
                    "error_code": "invalid_argument",
                    "detail": format!("unsupported argument: {other}"),
                });
                emit_governance_summary(summary)?;
                return Err(anyhow!("unsupported argument"));
            }
        }

        index += 1;
    }

    let Some(plugin) = plugin else {
        let summary = json!({
            "command": "enable-plugin",
            "status": "error",
            "error_code": "missing_plugin",
            "detail": "--plugin is required",
        });
        emit_governance_summary(summary)?;
        return Err(anyhow!("missing --plugin"));
    };

    let normalized_plugin = plugin.trim().to_ascii_lowercase();
    if normalized_plugin == "stagehand" {
        let mut reasons = Vec::new();
        if domains.is_empty() {
            reasons.push("domains_required");
        }
        if workspaces.is_empty() {
            reasons.push("workspaces_required");
        }

        if !reasons.is_empty() {
            let summary = json!({
                "command": "enable-plugin",
                "status": "blocked",
                "error_code": "policy_requirements_missing",
                "plugin": plugin,
                "reasons": reasons,
            });
            emit_governance_summary(summary)?;
            return Err(anyhow!("stagehand requires explicit domains/workspaces"));
        }

        let mut permissions = vec![
            DelegationCapability {
                id: "stagehand.enabled".to_string(),
                scope: Vec::new(),
            },
            DelegationCapability {
                id: "browser.observe".to_string(),
                scope: domains.clone(),
            },
            DelegationCapability {
                id: "workspace.read".to_string(),
                scope: workspaces.clone(),
            },
        ];

        if !commands.is_empty() {
            permissions.push(DelegationCapability {
                id: "command.run".to_string(),
                scope: commands.clone(),
            });
        }

        let envelope = PluginPermissionEnvelope {
            plugin: normalized_plugin,
            trust_level: TrustLevel::Caution,
            permissions,
        };

        let policy = stagehand_policy_from_envelope(&envelope);
        let mut checks = Vec::new();
        let mut has_denied_check = false;

        for domain in &domains {
            let probe_domain = normalize_domain_probe_input(domain);
            let decision = policy.evaluate(Action::ObserveUrl(probe_domain.clone()));
            if matches!(decision, PermissionDecision::Deny { .. }) {
                has_denied_check = true;
            }
            checks.push(json!({
                "name": "domain_allowlist",
                "value": domain,
                "probe": probe_domain,
                "decision": decision_name(&decision),
                "reason": decision_reason(&decision),
            }));
        }

        for workspace in &workspaces {
            let decision = policy.evaluate(Action::ReadWorkspace(workspace.clone()));
            if matches!(decision, PermissionDecision::Deny { .. }) {
                has_denied_check = true;
            }
            checks.push(json!({
                "name": "workspace_allowlist",
                "value": workspace,
                "decision": decision_name(&decision),
                "reason": decision_reason(&decision),
            }));
        }

        for command in &commands {
            let decision = policy.evaluate(Action::RunCommand(command.clone()));
            if matches!(decision, PermissionDecision::Deny { .. }) {
                has_denied_check = true;
            }
            checks.push(json!({
                "name": "command_allowlist",
                "value": command,
                "decision": decision_name(&decision),
                "reason": decision_reason(&decision),
            }));
        }

        if has_denied_check {
            emit_governance_summary(json!({
                "command": "enable-plugin",
                "status": "blocked",
                "error_code": "policy_checks_denied",
                "plugin": plugin,
                "domains": domains,
                "workspaces": workspaces,
                "commands": commands,
                "checks": checks,
            }))?;
            return Err(anyhow!("stagehand policy checks denied requested scope"));
        }

        return emit_governance_summary(json!({
            "command": "enable-plugin",
            "status": "ok",
            "plugin": plugin,
            "domains": domains,
            "workspaces": workspaces,
            "commands": commands,
            "checks": checks
        }));
    }

    emit_governance_summary(json!({
        "command": "enable-plugin",
        "status": "ok",
        "plugin": plugin,
        "detail": "no governance policy handler for this plugin",
    }))
}

fn governance_verify(args: &[String]) -> anyhow::Result<()> {
    let mut scope = SkillScope::Project;
    let mut registry_path: Option<PathBuf> = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--scope" => {
                let value = match next_arg_value(args, &mut index, "--scope") {
                    Ok(value) => value,
                    Err(err) => return governance_parse_error("verify", err),
                };
                scope = match parse_scope(&value) {
                    Ok(scope) => scope,
                    Err(err) => return governance_parse_error("verify", err),
                };
            }
            "--registry" => {
                let value = match next_arg_value(args, &mut index, "--registry") {
                    Ok(value) => value,
                    Err(err) => return governance_parse_error("verify", err),
                };
                registry_path = Some(PathBuf::from(value));
            }
            "--run-once" => {}
            other => {
                let summary = json!({
                    "command": "verify",
                    "status": "error",
                    "error_code": "invalid_argument",
                    "detail": format!("unsupported argument: {other}"),
                });
                emit_governance_summary(summary)?;
                return Err(anyhow!("unsupported argument"));
            }
        }

        index += 1;
    }

    let registry_path = registry_path.unwrap_or_else(|| default_registry_path(scope.clone()));
    let mut checks = Vec::new();

    match load_registry(&registry_path, scope.clone()) {
        Ok(registry) => {
            checks.push(json!({
                "name": "registry_load",
                "status": "pass",
                "detail": format!("loaded {} skill record(s)", registry.skills.len()),
            }));

            let has_trusted = registry
                .skills
                .iter()
                .any(|record| matches!(record.trust_level, TrustLevel::Trusted));
            checks.push(json!({
                "name": "trusted_skill_present",
                "status": if has_trusted { "pass" } else { "fail" },
                "detail": if has_trusted {
                    "registry has at least one trusted skill"
                } else {
                    "registry has no trusted skills"
                },
            }));

            let has_stagehand = registry
                .skills
                .iter()
                .any(|record| record.name.eq_ignore_ascii_case("stagehand"));
            checks.push(json!({
                "name": "stagehand_skill_registered",
                "status": if has_stagehand { "pass" } else { "fail" },
                "detail": if has_stagehand {
                    "stagehand is explicitly registered"
                } else {
                    "stagehand is not present in this registry"
                },
            }));
        }
        Err(err) => {
            checks.push(json!({
                "name": "registry_load",
                "status": "fail",
                "detail": err.to_string(),
            }));
        }
    }

    let pass_count = checks
        .iter()
        .filter(|check| check.get("status").and_then(|v| v.as_str()) == Some("pass"))
        .count();
    let fail_count = checks
        .iter()
        .filter(|check| check.get("status").and_then(|v| v.as_str()) == Some("fail"))
        .count();

    let overall = if fail_count == 0 { "pass" } else { "fail" };
    let status = if fail_count == 0 { "ok" } else { "failed" };

    emit_governance_summary(json!({
        "command": "verify",
        "status": status,
        "scope": scope_name(scope),
        "registry": registry_path.display().to_string(),
        "summary": {
            "pass": pass_count,
            "fail": fail_count,
            "overall": overall,
        },
        "checks": checks,
    }))?;

    if fail_count > 0 {
        return Err(anyhow!("governance verification failed"));
    }

    Ok(())
}

fn handle_governance_command(args: &[String]) -> anyhow::Result<()> {
    let Some(command) = args.first() else {
        let summary = json!({
            "command": "governance",
            "status": "error",
            "error_code": "missing_subcommand",
            "detail": "expected one of: discover | install | enable-plugin | verify",
        });
        emit_governance_summary(summary)?;
        return Err(anyhow!("missing governance subcommand"));
    };

    match command.as_str() {
        "discover" => governance_discover(&args[1..]),
        "install" => governance_install(&args[1..]),
        "enable-plugin" => governance_enable_plugin(&args[1..]),
        "verify" => governance_verify(&args[1..]),
        other => {
            let summary = json!({
                "command": "governance",
                "status": "error",
                "error_code": "unknown_subcommand",
                "detail": format!("unsupported governance subcommand: {other}"),
            });
            emit_governance_summary(summary)?;
            Err(anyhow!("unsupported governance subcommand"))
        }
    }
}

#[derive(Clone, Debug, Default)]
struct StdoutTaskIngress;

impl TaskIngress for StdoutTaskIngress {
    fn write_task_payload(&self, payload: &str) -> odin_core_runtime::RuntimeResult<()> {
        println!("enqueue payload (stdout ingress): {payload}");
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if let Some(governance_args) = governance_subcommand_args(&args) {
        return handle_governance_command(governance_args);
    }

    let cfg = parse_cli_config();
    println!("odin-cli starting with config: {}", cfg.config_path);
    println!("plugins root: {}", cfg.plugins_root.display());

    let mut legacy_paths: Option<LegacyScriptPaths> = None;

    if let Some(root) = &cfg.legacy_root {
        let paths = LegacyScriptPaths::from_legacy_root(root);
        legacy_paths = Some(paths.clone());

        let ingress_adapter = BashTaskIngressAdapter::from_paths(&paths);
        println!(
            "compat ingress adapter initialized: {}",
            ingress_adapter.script_path().display()
        );

        let backend_adapter =
            BashBackendStateAdapter::from_paths(&paths, cfg.legacy_odin_dir.clone());
        println!(
            "compat backend-state adapter initialized: {}",
            backend_adapter.backend_state_lib().display()
        );

        let failover_adapter = BashFailoverAdapter::from_paths(&paths, cfg.legacy_odin_dir.clone());
        println!(
            "compat failover adapter initialized: {}",
            failover_adapter.failover_lib().display()
        );

        match backend_adapter.get_active_backend() {
            Ok(backend) => println!("legacy active backend: {backend}"),
            Err(err) => println!("legacy active backend unavailable: {err}"),
        }
    }

    let mut policy = StaticPolicyEngine::default();
    policy.set_require_approval_for_destructive(true);
    policy.allow_capability("example.safe-github", "*", "repo.read");
    policy.allow_capability("private.ops-watchdog", "*", "monitoring.sentry.read");
    policy.allow_capability("private.ops-watchdog", "*", "vcs.pr.read");
    policy.allow_capability("private.ops-watchdog", "*", "task.enqueue");

    let runtime = OrchestratorRuntime::new(policy, NoopAuditSink, DryRunExecutor);

    if let Some(task_file) = &cfg.task_file {
        let task_json = fs::read_to_string(task_file)
            .with_context(|| format!("failed to read task file {}", task_file.display()))?;
        let plugin_runner = ExternalProcessPluginRunner::new(cfg.plugins_root.clone());

        let outcomes = if let Some(paths) = &legacy_paths {
            let ingress = BashTaskIngressAdapter::from_paths(paths);
            runtime.handle_watchdog_task(&task_json, &plugin_runner, &ingress)?
        } else {
            let ingress = StdoutTaskIngress;
            runtime.handle_watchdog_task(&task_json, &plugin_runner, &ingress)?
        };

        let outcomes_json =
            serde_json::to_string_pretty(&outcomes).context("failed to format task outcomes")?;
        println!("task outcomes:\n{outcomes_json}");
        return Ok(());
    }

    let outcome = runtime
        .handle_action(sample_action_request())
        .context("bootstrap action evaluation failed")?;

    let outcome_json =
        serde_json::to_string_pretty(&outcome).context("failed to format bootstrap outcome")?;
    println!("bootstrap outcome:\n{outcome_json}");

    if cfg.run_once {
        return Ok(());
    }

    loop {
        thread::sleep(Duration::from_secs(60));
    }
}
