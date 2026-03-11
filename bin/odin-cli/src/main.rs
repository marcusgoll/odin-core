use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context};
use clap::{Parser, Subcommand, ValueEnum};
use odin_audit::NoopAuditSink;
use odin_compat_bash::{
    BashBackendStateAdapter, BashFailoverAdapter, BashTaskIngressAdapter, LegacyScriptPaths,
};
use odin_core_runtime::{
    BackendState, DryRunExecutor, ExternalProcessPluginRunner, OrchestratorRuntime, TaskIngress,
};
use odin_governance::import::{evaluate_install, Ack, InstallGateStatus, SkillImportCandidate};
use odin_governance::plugins::{
    huginn_policy_from_envelope, Action as HuginnAction, PermissionDecision as HuginnDecision,
};
use odin_governance::risk_scan::{RiskCategory, RiskFinding};
use odin_governance::skills::{load_global_registry, load_project_registry, load_user_registry};
use odin_plugin_protocol::{
    ActionRequest, CapabilityRequest, DelegationCapability, PluginPermissionEnvelope, RiskTier,
    SkillRecord, SkillScope, TrustLevel,
};
use odin_policy_engine::StaticPolicyEngine;
use serde_json::{json, Value};

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

#[derive(Clone, Debug, Parser)]
#[command(name = "odin-cli")]
#[command(about = "Odin runtime and bootstrap CLI")]
struct Cli {
    #[arg(long = "config", default_value = "config/default.yaml", global = true)]
    config_path: String,
    #[arg(long, global = true)]
    legacy_root: Option<PathBuf>,
    #[arg(long, default_value = "/var/odin", global = true)]
    legacy_odin_dir: PathBuf,
    #[arg(long, default_value = "examples/private-plugins", global = true)]
    plugins_root: PathBuf,
    #[arg(long, global = true)]
    task_file: Option<PathBuf>,
    #[arg(long, global = true)]
    run_once: bool,
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Clone, Debug, Subcommand)]
enum CliCommand {
    Connect {
        provider: String,
        #[arg(value_enum)]
        auth_mode: AuthMode,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        confirm: bool,
    },
    Start {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        confirm: bool,
    },
    Tui {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        confirm: bool,
    },
    Inbox {
        #[command(subcommand)]
        command: InboxCommand,
    },
    Gateway {
        #[command(subcommand)]
        command: GatewayCommand,
    },
    Verify {
        #[arg(long)]
        dry_run: bool,
    },
    Skill {
        #[command(subcommand)]
        command: SkillCommand,
    },
    /// Orchestrator-to-core migration tools
    Migrate {
        #[command(subcommand)]
        command: MigrateSubcommand,
    },
}

#[derive(Clone, Debug, Subcommand)]
enum InboxCommand {
    Add {
        title: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        confirm: bool,
    },
    List {
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Clone, Debug, Subcommand)]
enum GatewayCommand {
    Add {
        #[arg(value_enum)]
        source: GatewaySource,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(Clone, Debug, Subcommand)]
enum SkillCommand {
    Validate { file: PathBuf },
    Mermaid { file: PathBuf },
}

#[derive(Clone, Debug, Subcommand)]
enum MigrateSubcommand {
    /// Export a migration bundle from the orchestrator
    Export {
        #[arg(long)]
        source_root: Option<PathBuf>,
        #[arg(long, default_value = "/var/odin")]
        odin_dir: PathBuf,
        #[arg(long, default_value = "migration-bundle")]
        out_dir: PathBuf,
    },
    /// Validate a migration bundle
    Validate {
        #[arg(long)]
        bundle: Option<PathBuf>,
    },
    /// Import a migration bundle into odin-core
    Import,
    #[command(external_subcommand)]
    Unknown(Vec<OsString>),
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum AuthMode {
    Oauth,
    Api,
}

impl AuthMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Oauth => "oauth",
            Self::Api => "api",
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum GatewaySource {
    Cli,
    Slack,
    Telegram,
}

impl GatewaySource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Slack => "slack",
            Self::Telegram => "telegram",
        }
    }
}

fn now_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn print_inbox_normalized_fields(title: &str) {
    let timestamp = now_unix_timestamp();
    println!(
        "normalized inbox item title={title} raw_text={title} source=cli timestamp={timestamp}"
    );
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
        input: serde_json::json!({"probe": true}),
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

fn parse_legacy_cli_config(raw_args: &[String]) -> CliConfig {
    let mut cfg = CliConfig::default();
    let mut idx = 0usize;

    while idx < raw_args.len() {
        let arg = raw_args[idx].as_str();
        match arg {
            "--config" => {
                if let Some(path) = raw_args.get(idx + 1) {
                    cfg.config_path = path.clone();
                    idx += 2;
                    continue;
                }
            }
            "--legacy-root" => {
                if let Some(path) = raw_args.get(idx + 1) {
                    cfg.legacy_root = Some(PathBuf::from(path));
                    idx += 2;
                    continue;
                }
            }
            "--legacy-odin-dir" => {
                if let Some(path) = raw_args.get(idx + 1) {
                    cfg.legacy_odin_dir = PathBuf::from(path);
                    idx += 2;
                    continue;
                }
            }
            "--plugins-root" => {
                if let Some(path) = raw_args.get(idx + 1) {
                    cfg.plugins_root = PathBuf::from(path);
                    idx += 2;
                    continue;
                }
            }
            "--task-file" => {
                if let Some(path) = raw_args.get(idx + 1) {
                    cfg.task_file = Some(PathBuf::from(path));
                    idx += 2;
                    continue;
                }
            }
            "--run-once" => {
                cfg.run_once = true;
                idx += 1;
                continue;
            }
            _ => {}
        }

        if let Some(path) = arg.strip_prefix("--config=") {
            if !path.is_empty() {
                cfg.config_path = path.to_string();
            }
        } else if let Some(path) = arg.strip_prefix("--legacy-root=") {
            if !path.is_empty() {
                cfg.legacy_root = Some(PathBuf::from(path));
            }
        } else if let Some(path) = arg.strip_prefix("--legacy-odin-dir=") {
            if !path.is_empty() {
                cfg.legacy_odin_dir = PathBuf::from(path);
            }
        } else if let Some(path) = arg.strip_prefix("--plugins-root=") {
            if !path.is_empty() {
                cfg.plugins_root = PathBuf::from(path);
            }
        } else if let Some(path) = arg.strip_prefix("--task-file=") {
            if !path.is_empty() {
                cfg.task_file = Some(PathBuf::from(path));
            }
        }

        idx += 1;
    }

    cfg
}

fn parse_error_targets_native_contract(raw_args: &[String]) -> bool {
    let mut idx = 0usize;
    while idx < raw_args.len() {
        let arg = raw_args[idx].as_str();
        match arg {
            "--config" | "--legacy-root" | "--legacy-odin-dir" | "--plugins-root"
            | "--task-file" => {
                idx += 2;
                continue;
            }
            "--run-once" | "--help" | "-h" => {
                idx += 1;
                continue;
            }
            "--" => {
                idx += 1;
                break;
            }
            _ => {}
        }

        if arg.starts_with("--config=")
            || arg.starts_with("--legacy-root=")
            || arg.starts_with("--legacy-odin-dir=")
            || arg.starts_with("--plugins-root=")
            || arg.starts_with("--task-file=")
        {
            idx += 1;
            continue;
        }

        if arg.starts_with('-') {
            idx += 1;
            continue;
        }

        return matches!(
            arg,
            "connect"
                | "start"
                | "tui"
                | "inbox"
                | "gateway"
                | "verify"
                | "skill"
                | "migrate"
                | "governance"
        );
    }

    if let Some(token) = raw_args.get(idx).map(String::as_str) {
        return matches!(
            token,
            "connect"
                | "start"
                | "tui"
                | "inbox"
                | "gateway"
                | "verify"
                | "skill"
                | "migrate"
                | "governance"
        );
    }

    false
}

enum GovernanceBody {
    Json(Value),
    Text(String),
}

struct GovernanceOutcome {
    exit_code: i32,
    body: GovernanceBody,
}

fn governance_command_index(raw_args: &[String]) -> Option<usize> {
    let mut idx = 0usize;

    while idx < raw_args.len() {
        let token = raw_args[idx].as_str();
        match token {
            "--run-once" => idx += 1,
            "--config" | "--legacy-root" | "--legacy-odin-dir" | "--plugins-root"
            | "--task-file" => idx += 2,
            _ if token.starts_with("--config=")
                || token.starts_with("--legacy-root=")
                || token.starts_with("--legacy-odin-dir=")
                || token.starts_with("--plugins-root=")
                || token.starts_with("--task-file=") =>
            {
                idx += 1;
            }
            "governance" => return Some(idx),
            _ => return None,
        }
    }

    None
}

fn governance_help_text(subcommand: Option<&str>) -> String {
    match subcommand {
        Some("discover") => "\
Usage: odin-cli governance discover --scope <global|project|user> [--registry <path>]

Print registered skill candidates for the selected scope.
"
        .to_string(),
        Some("install") => "\
Usage: odin-cli governance install --name <skill> --trust-level <trusted|caution|untrusted> [--ack]

Evaluate install gates for a skill candidate and report required acknowledgements.
"
        .to_string(),
        Some("verify") => "\
Usage: odin-cli governance verify --scope <global|project|user> [--registry <path>]

Run governance verification checks for a skill registry.
"
        .to_string(),
        Some("enable-plugin") => "\
Usage: odin-cli governance enable-plugin --plugin huginn [--domains <csv>] [--workspaces <csv>] [--commands <csv>]

Evaluate Huginn plugin policy requirements before enabling browser access.
"
        .to_string(),
        _ => "\
Usage: odin-cli governance <command> [options]

Commands:
  discover       List registered skill candidates for a governance scope
  install        Evaluate install gates for a skill candidate
  verify         Run governance verification checks
  enable-plugin  Evaluate Huginn plugin policy inputs
"
        .to_string(),
    }
}

fn skip_global_option(tokens: &[String], idx: &mut usize) -> bool {
    match tokens.get(*idx).map(String::as_str) {
        Some("--run-once") => {
            *idx += 1;
            true
        }
        Some("--config")
        | Some("--legacy-root")
        | Some("--legacy-odin-dir")
        | Some("--plugins-root")
        | Some("--task-file") => {
            *idx += 2;
            true
        }
        Some(token)
            if token.starts_with("--config=")
                || token.starts_with("--legacy-root=")
                || token.starts_with("--legacy-odin-dir=")
                || token.starts_with("--plugins-root=")
                || token.starts_with("--task-file=") =>
        {
            *idx += 1;
            true
        }
        _ => false,
    }
}

fn missing_required_value(command: &str, option: &str) -> GovernanceOutcome {
    GovernanceOutcome {
        exit_code: 1,
        body: GovernanceBody::Json(json!({
            "command": command,
            "status": "error",
            "error_code": "missing_required_value",
            "option": option,
        })),
    }
}

fn governance_error(command: &str, error_code: &str, detail: &str) -> GovernanceOutcome {
    GovernanceOutcome {
        exit_code: 1,
        body: GovernanceBody::Json(json!({
            "command": command,
            "status": "error",
            "error_code": error_code,
            "detail": detail,
        })),
    }
}

fn governance_scope_as_str(scope: &SkillScope) -> &'static str {
    match scope {
        SkillScope::Global => "global",
        SkillScope::Project => "project",
        SkillScope::User => "user",
    }
}

fn parse_governance_scope(command: &str, value: &str) -> Result<SkillScope, GovernanceOutcome> {
    match value.trim().to_ascii_lowercase().as_str() {
        "global" => Ok(SkillScope::Global),
        "project" => Ok(SkillScope::Project),
        "user" => Ok(SkillScope::User),
        _ => Err(governance_error(
            command,
            "invalid_scope",
            "unsupported scope",
        )),
    }
}

fn parse_trust_level(command: &str, value: &str) -> Result<TrustLevel, GovernanceOutcome> {
    match value.trim().to_ascii_lowercase().as_str() {
        "trusted" => Ok(TrustLevel::Trusted),
        "caution" => Ok(TrustLevel::Caution),
        "untrusted" => Ok(TrustLevel::Untrusted),
        _ => Err(governance_error(
            command,
            "invalid_trust_level",
            "unsupported trust level",
        )),
    }
}

fn parse_csv_values(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn default_registry_path(scope: &SkillScope) -> &'static str {
    match scope {
        SkillScope::Global => "config/skills.global.yaml",
        SkillScope::Project => "config/skills.project.yaml",
        SkillScope::User => "config/skills.user.yaml",
    }
}

fn load_registry(
    scope: &SkillScope,
    path: &Path,
) -> Result<odin_plugin_protocol::SkillRegistry, String> {
    match scope {
        SkillScope::Global => load_global_registry(path),
        SkillScope::Project => load_project_registry(path),
        SkillScope::User => load_user_registry(path),
    }
    .map_err(|err| err.to_string())
}

fn trust_level_as_str(level: &TrustLevel) -> &'static str {
    match level {
        TrustLevel::Trusted => "trusted",
        TrustLevel::Caution => "caution",
        TrustLevel::Untrusted => "untrusted",
    }
}

fn risk_category_as_str(category: &RiskCategory) -> &'static str {
    match category {
        RiskCategory::Shell => "shell",
        RiskCategory::Network => "network",
        RiskCategory::Secret => "secret",
        RiskCategory::Delete => "delete",
    }
}

fn risk_finding_json(finding: &RiskFinding) -> Value {
    json!({
        "category": risk_category_as_str(&finding.category),
        "pattern": finding.pattern,
    })
}

fn skill_record_json(record: &SkillRecord) -> Value {
    json!({
        "name": record.name,
        "trust_level": trust_level_as_str(&record.trust_level),
        "source": record.source,
        "capabilities": record.capabilities,
    })
}

fn decision_to_str(decision: &HuginnDecision) -> &'static str {
    match decision {
        HuginnDecision::Allow { .. } => "allow",
        HuginnDecision::Deny { .. } => "deny",
    }
}

fn decision_reason(decision: &HuginnDecision) -> &str {
    match decision {
        HuginnDecision::Allow { reason_code } | HuginnDecision::Deny { reason_code } => reason_code,
    }
}

fn command_value(
    tokens: &[String],
    idx: &mut usize,
    command: &str,
    option: &str,
) -> Result<String, GovernanceOutcome> {
    let Some(value) = tokens.get(*idx + 1) else {
        return Err(missing_required_value(command, option));
    };
    if value.starts_with('-') {
        return Err(missing_required_value(command, option));
    }
    *idx += 2;
    Ok(value.clone())
}

fn command_value_or_inline(
    tokens: &[String],
    idx: &mut usize,
    command: &str,
    option: &str,
) -> Result<String, GovernanceOutcome> {
    let token = tokens[*idx].as_str();
    if let Some(value) = token.strip_prefix(&format!("{option}=")) {
        *idx += 1;
        return Ok(value.to_string());
    }
    command_value(tokens, idx, command, option)
}

fn handle_governance_discover(tokens: &[String]) -> GovernanceOutcome {
    let command = "discover";
    let mut scope: Option<SkillScope> = None;
    let mut registry: Option<PathBuf> = None;
    let mut idx = 0usize;

    if tokens
        .iter()
        .any(|token| token == "--help" || token == "-h")
    {
        return GovernanceOutcome {
            exit_code: 0,
            body: GovernanceBody::Text(governance_help_text(Some(command))),
        };
    }

    while idx < tokens.len() {
        if skip_global_option(tokens, &mut idx) {
            continue;
        }

        let token = tokens[idx].as_str();
        match token {
            "--scope" => match command_value(tokens, &mut idx, command, "--scope") {
                Ok(value) => match parse_governance_scope(command, &value) {
                    Ok(parsed) => scope = Some(parsed),
                    Err(outcome) => return outcome,
                },
                Err(outcome) => return outcome,
            },
            "--registry" => match command_value(tokens, &mut idx, command, "--registry") {
                Ok(value) => registry = Some(PathBuf::from(value)),
                Err(outcome) => return outcome,
            },
            _ if token.starts_with("--scope=") => {
                let value = token.trim_start_matches("--scope=");
                match parse_governance_scope(command, value) {
                    Ok(parsed) => scope = Some(parsed),
                    Err(outcome) => return outcome,
                }
                idx += 1;
            }
            _ if token.starts_with("--registry=") => {
                registry = Some(PathBuf::from(token.trim_start_matches("--registry=")));
                idx += 1;
            }
            _ => return governance_error(command, "unknown_argument", token),
        }
    }

    let Some(scope) = scope else {
        return missing_required_value(command, "--scope");
    };
    let registry_path = registry.unwrap_or_else(|| PathBuf::from(default_registry_path(&scope)));
    match load_registry(&scope, &registry_path) {
        Ok(registry) => GovernanceOutcome {
            exit_code: 0,
            body: GovernanceBody::Json(json!({
                "command": command,
                "status": "ok",
                "scope": governance_scope_as_str(&scope),
                "registry": registry_path.display().to_string(),
                "candidates": registry.skills.iter().map(skill_record_json).collect::<Vec<_>>(),
            })),
        },
        Err(detail) => GovernanceOutcome {
            exit_code: 1,
            body: GovernanceBody::Json(json!({
                "command": command,
                "status": "failed",
                "error_code": "registry_load_failed",
                "registry": registry_path.display().to_string(),
                "detail": detail,
                "candidates": Vec::<Value>::new(),
            })),
        },
    }
}

fn handle_governance_install(tokens: &[String]) -> GovernanceOutcome {
    let command = "install";
    let mut name: Option<String> = None;
    let mut trust_level: Option<TrustLevel> = None;
    let mut ack = false;
    let mut idx = 0usize;

    if tokens
        .iter()
        .any(|token| token == "--help" || token == "-h")
    {
        return GovernanceOutcome {
            exit_code: 0,
            body: GovernanceBody::Text(governance_help_text(Some(command))),
        };
    }

    while idx < tokens.len() {
        if skip_global_option(tokens, &mut idx) {
            continue;
        }

        let token = tokens[idx].as_str();
        match token {
            "--name" => match command_value(tokens, &mut idx, command, "--name") {
                Ok(value) => name = Some(value),
                Err(outcome) => return outcome,
            },
            "--trust-level" => match command_value(tokens, &mut idx, command, "--trust-level") {
                Ok(value) => match parse_trust_level(command, &value) {
                    Ok(parsed) => trust_level = Some(parsed),
                    Err(outcome) => return outcome,
                },
                Err(outcome) => return outcome,
            },
            "--ack" => {
                ack = true;
                idx += 1;
            }
            _ if token.starts_with("--name=") => {
                name = Some(token.trim_start_matches("--name=").to_string());
                idx += 1;
            }
            _ if token.starts_with("--trust-level=") => {
                match parse_trust_level(command, token.trim_start_matches("--trust-level=")) {
                    Ok(parsed) => trust_level = Some(parsed),
                    Err(outcome) => return outcome,
                }
                idx += 1;
            }
            _ => return governance_error(command, "unknown_argument", token),
        }
    }

    let Some(name) = name else {
        return missing_required_value(command, "--name");
    };
    let Some(trust_level) = trust_level else {
        return missing_required_value(command, "--trust-level");
    };

    let candidate = SkillImportCandidate {
        record: SkillRecord {
            trust_level,
            source: format!("project:/skills/{name}"),
            ..SkillRecord::default_for(name)
        },
        scripts: Vec::new(),
        readme: None,
    };

    match evaluate_install(&candidate, if ack { Ack::Accepted } else { Ack::None }) {
        Ok(plan) => {
            let findings = plan
                .findings
                .iter()
                .map(risk_finding_json)
                .collect::<Vec<_>>();

            match plan.status {
                InstallGateStatus::Allowed => GovernanceOutcome {
                    exit_code: 0,
                    body: GovernanceBody::Json(json!({
                        "command": command,
                        "status": "ok",
                        "reasons": plan.reasons,
                        "findings": findings,
                    })),
                },
                InstallGateStatus::BlockedAckRequired => GovernanceOutcome {
                    exit_code: 1,
                    body: GovernanceBody::Json(json!({
                        "command": command,
                        "status": "blocked",
                        "error_code": "ack_required",
                        "reasons": plan.reasons,
                        "findings": findings,
                    })),
                },
            }
        }
        Err(err) => governance_error(command, "invalid_name", &err.to_string()),
    }
}

fn handle_governance_verify(tokens: &[String]) -> GovernanceOutcome {
    let command = "verify";
    let mut scope: Option<SkillScope> = None;
    let mut registry: Option<PathBuf> = None;
    let mut idx = 0usize;

    if tokens
        .iter()
        .any(|token| token == "--help" || token == "-h")
    {
        return GovernanceOutcome {
            exit_code: 0,
            body: GovernanceBody::Text(governance_help_text(Some(command))),
        };
    }

    while idx < tokens.len() {
        if skip_global_option(tokens, &mut idx) {
            continue;
        }

        let token = tokens[idx].as_str();
        match token {
            "--scope" => match command_value(tokens, &mut idx, command, "--scope") {
                Ok(value) => match parse_governance_scope(command, &value) {
                    Ok(parsed) => scope = Some(parsed),
                    Err(outcome) => return outcome,
                },
                Err(outcome) => return outcome,
            },
            "--registry" => match command_value(tokens, &mut idx, command, "--registry") {
                Ok(value) => registry = Some(PathBuf::from(value)),
                Err(outcome) => return outcome,
            },
            _ if token.starts_with("--scope=") => {
                let value = token.trim_start_matches("--scope=");
                match parse_governance_scope(command, value) {
                    Ok(parsed) => scope = Some(parsed),
                    Err(outcome) => return outcome,
                }
                idx += 1;
            }
            _ if token.starts_with("--registry=") => {
                registry = Some(PathBuf::from(token.trim_start_matches("--registry=")));
                idx += 1;
            }
            _ => return governance_error(command, "unknown_argument", token),
        }
    }

    let Some(scope) = scope else {
        return missing_required_value(command, "--scope");
    };
    let registry_path = registry.unwrap_or_else(|| PathBuf::from(default_registry_path(&scope)));
    let mut checks = Vec::new();

    match load_registry(&scope, &registry_path) {
        Ok(registry) => {
            checks.push(json!({
                "name": "registry_load",
                "status": "pass",
                "detail": "registry loaded",
            }));

            let has_browser_skill = registry.skills.iter().any(|record| {
                record
                    .capabilities
                    .iter()
                    .any(|capability| capability.id == "browser.observe")
            });
            checks.push(json!({
                "name": "browser_capability_present",
                "status": if has_browser_skill { "pass" } else { "fail" },
                "detail": if has_browser_skill {
                    "browser capability found"
                } else {
                    "no browser.observe capability found in registry"
                },
            }));

            let failed = checks.iter().any(|check| check["status"] == "fail");
            GovernanceOutcome {
                exit_code: if failed { 1 } else { 0 },
                body: GovernanceBody::Json(json!({
                    "command": command,
                    "status": if failed { "failed" } else { "ok" },
                    "registry": registry_path.display().to_string(),
                    "checks": checks,
                })),
            }
        }
        Err(detail) => {
            checks.push(json!({
                "name": "registry_load",
                "status": "fail",
                "detail": detail,
            }));
            GovernanceOutcome {
                exit_code: 1,
                body: GovernanceBody::Json(json!({
                    "command": command,
                    "status": "failed",
                    "registry": registry_path.display().to_string(),
                    "checks": checks,
                })),
            }
        }
    }
}

fn canonicalize_domain_probe(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

fn handle_governance_enable_plugin(tokens: &[String]) -> GovernanceOutcome {
    let command = "enable-plugin";
    let mut plugin: Option<String> = None;
    let mut domains: Vec<String> = Vec::new();
    let mut workspaces: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();
    let mut idx = 0usize;

    if tokens
        .iter()
        .any(|token| token == "--help" || token == "-h")
    {
        return GovernanceOutcome {
            exit_code: 0,
            body: GovernanceBody::Text(governance_help_text(Some(command))),
        };
    }

    while idx < tokens.len() {
        if skip_global_option(tokens, &mut idx) {
            continue;
        }

        let token = tokens[idx].as_str();
        match token {
            "--plugin" => match command_value(tokens, &mut idx, command, "--plugin") {
                Ok(value) => plugin = Some(value.to_ascii_lowercase()),
                Err(outcome) => return outcome,
            },
            "--domains" => match command_value_or_inline(tokens, &mut idx, command, "--domains") {
                Ok(value) => domains = parse_csv_values(&value),
                Err(outcome) => return outcome,
            },
            "--workspaces" => {
                match command_value_or_inline(tokens, &mut idx, command, "--workspaces") {
                    Ok(value) => workspaces = parse_csv_values(&value),
                    Err(outcome) => return outcome,
                }
            }
            "--commands" => {
                match command_value_or_inline(tokens, &mut idx, command, "--commands") {
                    Ok(value) => commands = parse_csv_values(&value),
                    Err(outcome) => return outcome,
                }
            }
            _ if token.starts_with("--plugin=") => {
                plugin = Some(token.trim_start_matches("--plugin=").to_ascii_lowercase());
                idx += 1;
            }
            _ => return governance_error(command, "unknown_argument", token),
        }
    }

    let Some(plugin) = plugin else {
        return missing_required_value(command, "--plugin");
    };
    if plugin != "huginn" {
        return governance_error(command, "unknown_plugin", "only huginn is supported");
    }

    let mut reasons = Vec::new();
    if domains.is_empty() {
        reasons.push("domains_required".to_string());
    }
    if workspaces.is_empty() {
        reasons.push("workspaces_required".to_string());
    }

    let mut permissions = vec![DelegationCapability {
        id: "huginn.enabled".to_string(),
        scope: vec![],
    }];
    if !domains.is_empty() {
        permissions.push(DelegationCapability {
            id: "browser.observe".to_string(),
            scope: domains.clone(),
        });
    }
    if !workspaces.is_empty() {
        permissions.push(DelegationCapability {
            id: "workspace.read".to_string(),
            scope: workspaces.clone(),
        });
    }
    if !commands.is_empty() {
        permissions.push(DelegationCapability {
            id: "command.run".to_string(),
            scope: commands.clone(),
        });
    }

    let policy = huginn_policy_from_envelope(&PluginPermissionEnvelope {
        plugin: "huginn".to_string(),
        trust_level: TrustLevel::Caution,
        permissions,
    });

    let mut checks = Vec::new();
    for domain in &domains {
        let decision = policy.evaluate(HuginnAction::ObserveUrl(canonicalize_domain_probe(domain)));
        checks.push(json!({
            "name": "domain_allowlist",
            "input": domain,
            "decision": decision_to_str(&decision),
            "reason_code": decision_reason(&decision),
        }));
    }
    for workspace in &workspaces {
        let decision = policy.evaluate(HuginnAction::ReadWorkspace(workspace.clone()));
        checks.push(json!({
            "name": "workspace_allowlist",
            "input": workspace,
            "decision": decision_to_str(&decision),
            "reason_code": decision_reason(&decision),
        }));
    }
    for command_value in &commands {
        let decision = policy.evaluate(HuginnAction::RunCommand(command_value.clone()));
        checks.push(json!({
            "name": "command_allowlist",
            "input": command_value,
            "decision": decision_to_str(&decision),
            "reason_code": decision_reason(&decision),
        }));
    }

    let has_denied_checks = checks.iter().any(|check| check["decision"] == "deny");
    if !reasons.is_empty() {
        return GovernanceOutcome {
            exit_code: 1,
            body: GovernanceBody::Json(json!({
                "command": command,
                "status": "blocked",
                "error_code": "policy_requirements_missing",
                "plugin": plugin,
                "reasons": reasons,
                "checks": checks,
            })),
        };
    }

    if has_denied_checks {
        return GovernanceOutcome {
            exit_code: 1,
            body: GovernanceBody::Json(json!({
                "command": command,
                "status": "blocked",
                "plugin": plugin,
                "checks": checks,
            })),
        };
    }

    GovernanceOutcome {
        exit_code: 0,
        body: GovernanceBody::Json(json!({
            "command": command,
            "status": "ok",
            "plugin": plugin,
            "checks": checks,
        })),
    }
}

fn try_handle_governance_command(raw_args: &[String]) -> Option<GovernanceOutcome> {
    let governance_idx = governance_command_index(raw_args)?;
    let Some(subcommand) = raw_args.get(governance_idx + 1).map(String::as_str) else {
        return Some(GovernanceOutcome {
            exit_code: 0,
            body: GovernanceBody::Text(governance_help_text(None)),
        });
    };
    let tokens = &raw_args[governance_idx + 2..];

    Some(match subcommand {
        "--help" | "-h" => GovernanceOutcome {
            exit_code: 0,
            body: GovernanceBody::Text(governance_help_text(None)),
        },
        "discover" => handle_governance_discover(tokens),
        "install" => handle_governance_install(tokens),
        "verify" => handle_governance_verify(tokens),
        "enable-plugin" => handle_governance_enable_plugin(tokens),
        other => governance_error("governance", "unknown_subcommand", other),
    })
}

// ---------------------------------------------------------------------------
// SASS skill XML parsing, validation, and mermaid generation
// ---------------------------------------------------------------------------

struct SassTransition {
    target: String,
    has_guard: bool,
}

struct SassState {
    id: String,
    is_end: bool,
    on_failure: Option<String>,
    transitions: Vec<SassTransition>,
}

struct SassSkill {
    wake_up_state: Option<String>,
    initial_state: Option<String>,
    states: Vec<SassState>,
}

fn parse_sass_skill(path: &Path) -> anyhow::Result<SassSkill> {
    let xml = fs::read_to_string(path)
        .with_context(|| format!("failed to read skill file: {}", path.display()))?;
    let doc = roxmltree::Document::parse(&xml)
        .with_context(|| format!("failed to parse XML: {}", path.display()))?;

    let root = doc.root_element();

    let wake_up_state = root
        .children()
        .find(|n| n.has_tag_name("wake_up"))
        .and_then(|n| n.attribute("state").map(String::from));

    let states_elem = root.children().find(|n| n.has_tag_name("states"));

    let initial_state = states_elem
        .as_ref()
        .and_then(|n| n.attribute("initial_state").map(String::from));

    let mut states = Vec::new();
    if let Some(states_elem) = &states_elem {
        for state_node in states_elem.children().filter(|n| n.has_tag_name("state")) {
            let id = state_node.attribute("id").unwrap_or_default().to_string();
            let is_end = state_node.attribute("end") == Some("true");
            let on_failure = state_node.attribute("on_failure").map(String::from);

            let transitions: Vec<SassTransition> = state_node
                .children()
                .filter(|n| n.has_tag_name("transition"))
                .map(|t| {
                    let target = t.attribute("target").unwrap_or_default().to_string();
                    let has_guard = t.children().any(|c| c.has_tag_name("guard"));
                    SassTransition { target, has_guard }
                })
                .collect();

            states.push(SassState {
                id,
                is_end,
                on_failure,
                transitions,
            });
        }
    }

    Ok(SassSkill {
        wake_up_state,
        initial_state,
        states,
    })
}

fn validate_sass_skill(skill: &SassSkill) -> Vec<String> {
    let mut errors = Vec::new();

    // Rule 1: wake_up must exist
    let wake_up = match &skill.wake_up_state {
        Some(w) => w.clone(),
        None => {
            errors.push("wake_up element is required".to_string());
            return errors;
        }
    };

    // Rule 2: initial_state must exist and match wake_up
    match &skill.initial_state {
        Some(initial) => {
            if *initial != wake_up {
                errors.push(format!(
                    "wake_up state '{wake_up}' does not match initial_state '{initial}'"
                ));
            }
        }
        None => {
            errors.push("states element with initial_state attribute is required".to_string());
        }
    }

    let state_ids: HashSet<&str> = skill.states.iter().map(|s| s.id.as_str()).collect();
    // Rule 3: at least one end state
    if !skill.states.iter().any(|s| s.is_end) {
        errors.push("at least one end state is required".to_string());
    }

    for state in &skill.states {
        // Rule 4: non-end states must have on_failure
        if !state.is_end && state.on_failure.is_none() {
            errors.push(format!(
                "state '{}' missing on_failure (required for non-end states)",
                state.id
            ));
        }

        // Rule 4b: on_failure target must exist
        if let Some(ref target) = state.on_failure {
            if !state_ids.contains(target.as_str()) {
                errors.push(format!(
                    "state '{}' on_failure transitions to unknown target '{target}'",
                    state.id
                ));
            }
        }

        // Rule 5: all transition targets must exist
        for t in &state.transitions {
            if !state_ids.contains(t.target.as_str()) {
                errors.push(format!(
                    "state '{}' transitions to unknown target '{}'",
                    state.id, t.target
                ));
            }
        }

        // Rule 6: decision states (>1 transition) must guard every branch
        if state.transitions.len() > 1 && state.transitions.iter().any(|t| !t.has_guard) {
            errors.push(format!(
                "state '{}' has decision transitions without guards",
                state.id
            ));
        }
    }

    // Check wake_up state exists
    if !state_ids.contains(wake_up.as_str()) {
        errors.push(format!("wake_up references unknown state '{wake_up}'"));
    }

    errors
}

fn generate_mermaid(skill: &SassSkill) -> String {
    let mut lines = Vec::new();
    lines.push("stateDiagram-v2".to_string());

    if let Some(ref wake_up) = skill.wake_up_state {
        lines.push(format!("    %% wake_up: {wake_up}"));
        lines.push(format!("    [*] --> {wake_up}"));
    }

    for state in &skill.states {
        for t in &state.transitions {
            lines.push(format!("    {} --> {}", state.id, t.target));
        }
    }

    lines.join("\n")
}

fn handle_skill_command(command: SkillCommand) -> anyhow::Result<()> {
    match command {
        SkillCommand::Validate { file } => {
            let skill = parse_sass_skill(&file)?;
            let errors = validate_sass_skill(&skill);
            if errors.is_empty() {
                println!("validation ok");
                Ok(())
            } else {
                for err in &errors {
                    eprintln!("validation failed: {err}");
                }
                process::exit(1);
            }
        }
        SkillCommand::Mermaid { file } => {
            let skill = parse_sass_skill(&file)?;
            let errors = validate_sass_skill(&skill);
            if !errors.is_empty() {
                for err in &errors {
                    eprintln!("validation failed: {err}");
                }
                process::exit(1);
            }
            println!("{}", generate_mermaid(&skill));
            Ok(())
        }
    }
}

fn handle_bootstrap_command(command: CliCommand) -> anyhow::Result<()> {
    match command {
        CliCommand::Connect {
            provider,
            auth_mode,
            dry_run,
            confirm: _,
        } => {
            if dry_run {
                println!(
                    "DRY-RUN connect provider={provider} auth={}",
                    auth_mode.as_str()
                );
            } else {
                println!(
                    "connect placeholder provider={provider} auth={}",
                    auth_mode.as_str()
                );
            }
            Ok(())
        }
        CliCommand::Start {
            dry_run,
            confirm: _,
        } => {
            if dry_run {
                println!("DRY-RUN start");
            } else {
                println!("start placeholder");
            }
            Ok(())
        }
        CliCommand::Tui {
            dry_run,
            confirm: _,
        } => {
            if dry_run {
                println!("DRY-RUN tui");
            } else {
                println!("tui placeholder");
            }
            Ok(())
        }
        CliCommand::Inbox { command } => match command {
            InboxCommand::Add {
                title,
                dry_run,
                confirm: _,
            } => {
                if dry_run {
                    println!("DRY-RUN inbox add title={title}");
                    print_inbox_normalized_fields(&title);
                } else {
                    println!("inbox add placeholder title={title}");
                    print_inbox_normalized_fields(&title);
                }
                Ok(())
            }
            InboxCommand::List { dry_run: _ } => {
                println!("inbox list placeholder (empty)");
                Ok(())
            }
        },
        CliCommand::Gateway { command } => match command {
            GatewayCommand::Add {
                source,
                dry_run,
                confirm: _,
            } => {
                if dry_run {
                    println!("DRY-RUN gateway add source={}", source.as_str());
                } else {
                    println!("gateway add placeholder source={}", source.as_str());
                }
                Ok(())
            }
        },
        CliCommand::Verify { dry_run } => {
            if dry_run {
                println!("DRY-RUN verify");
                Ok(())
            } else {
                Err(anyhow!(
                    "native non-dry-run verify is not implemented; use scripts/odin/odin verify or --dry-run"
                ))
            }
        }
        CliCommand::Skill { command } => handle_skill_command(command),
        CliCommand::Migrate { command } => match command {
            MigrateSubcommand::Export {
                source_root,
                odin_dir,
                out_dir,
            } => {
                let source_root = match source_root {
                    Some(p) => p,
                    None => {
                        eprintln!("missing required flag: --source-root");
                        process::exit(1);
                    }
                };
                odin_migration::run(odin_migration::MigrationCommand::Export {
                    source_root,
                    odin_dir,
                    out_dir,
                })
            }
            MigrateSubcommand::Validate { bundle } => {
                let bundle_dir = match bundle {
                    Some(p) => p,
                    None => {
                        eprintln!("missing required flag: --bundle");
                        process::exit(1);
                    }
                };
                odin_migration::run(odin_migration::MigrationCommand::Validate { bundle_dir })
            }
            MigrateSubcommand::Import => {
                odin_migration::run(odin_migration::MigrationCommand::Import)
            }
            MigrateSubcommand::Unknown(args) => {
                let name = args
                    .first()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                eprintln!("unknown migrate subcommand: {name}");
                process::exit(1);
            }
        },
    }
}

fn run_legacy_runtime(cfg: CliConfig) -> anyhow::Result<()> {
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

fn main() -> anyhow::Result<()> {
    let raw_args: Vec<String> = env::args().collect();
    if let Some(outcome) = try_handle_governance_command(&raw_args[1..]) {
        match outcome.body {
            GovernanceBody::Json(body) => {
                let payload = serde_json::to_string_pretty(&body)
                    .context("failed to format governance output")?;
                println!("{payload}");
            }
            GovernanceBody::Text(body) => println!("{body}"),
        }
        process::exit(outcome.exit_code);
    }

    match Cli::try_parse_from(raw_args.clone()) {
        Ok(cli) => {
            let cfg = CliConfig {
                config_path: cli.config_path.clone(),
                legacy_root: cli.legacy_root.clone(),
                legacy_odin_dir: cli.legacy_odin_dir.clone(),
                plugins_root: cli.plugins_root.clone(),
                task_file: cli.task_file.clone(),
                run_once: cli.run_once,
            };

            if let Some(command) = cli.command {
                handle_bootstrap_command(command)?;
                return Ok(());
            }

            run_legacy_runtime(cfg)
        }
        Err(err) => {
            if !parse_error_targets_native_contract(&raw_args[1..]) {
                let cfg = parse_legacy_cli_config(&raw_args[1..]);
                return run_legacy_runtime(cfg);
            }
            err.exit()
        }
    }
}
