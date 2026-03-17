use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
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
use odin_governance::import::{self, Ack, SkillImportCandidate};
use odin_governance::plugins::{self, Action, PermissionDecision};
use odin_governance::skills::{self, SkillRegistryLoadError};
use odin_plugin_protocol::{
    ActionRequest, CapabilityRequest, RiskTier, SkillRecord, SkillRegistry, TrustLevel,
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
    /// Adaptive capacity scheduling engine
    Capacity {
        #[command(subcommand)]
        command: CapacityCommand,
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

#[derive(Clone, Debug, Subcommand)]
enum CapacityCommand {
    /// Run a scheduling cycle given current state
    Schedule {
        /// Path to queue state JSON (queue_depths, held_depth, deferred_depth)
        #[arg(long)]
        queue_state: PathBuf,
        /// Path to agent state JSON (agent_count)
        #[arg(long)]
        agent_state: PathBuf,
        /// Path to infra state JSON (swap_pct, cpu_pct, memory_mb)
        #[arg(long)]
        infra_state: PathBuf,
        /// Path to cost state JSON (spend_today_usd, spend_ceiling_usd)
        #[arg(long)]
        cost_state: PathBuf,
        /// Path to capacity config YAML (optional, uses defaults if absent)
        #[arg(long = "capacity-config")]
        capacity_config: Option<PathBuf>,
        /// Print decision without side effects
        #[arg(long)]
        dry_run: bool,
    },
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GovernanceScope {
    Global,
    Project,
    User,
}

impl GovernanceScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Project => "project",
            Self::User => "user",
        }
    }

    fn default_registry_path(self) -> &'static str {
        match self {
            Self::Global => "config/skills.global.yaml",
            Self::Project => "config/skills.project.yaml",
            Self::User => "config/skills.user.yaml",
        }
    }
}

fn parse_governance_scope(raw: &str) -> Option<GovernanceScope> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "global" => Some(GovernanceScope::Global),
        "project" => Some(GovernanceScope::Project),
        "user" => Some(GovernanceScope::User),
        _ => None,
    }
}

fn parse_trust_level(raw: &str) -> Option<TrustLevel> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "trusted" => Some(TrustLevel::Trusted),
        "caution" => Some(TrustLevel::Caution),
        "untrusted" => Some(TrustLevel::Untrusted),
        _ => None,
    }
}

fn trust_level_str(level: &TrustLevel) -> &'static str {
    match level {
        TrustLevel::Trusted => "trusted",
        TrustLevel::Caution => "caution",
        TrustLevel::Untrusted => "untrusted",
    }
}

fn split_csv_values(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn decision_json(name: &str, input: &str, decision: PermissionDecision) -> Value {
    match decision {
        PermissionDecision::Allow { reason_code } => json!({
            "name": name,
            "input": input,
            "decision": "allow",
            "reason": reason_code,
        }),
        PermissionDecision::Deny { reason_code } => json!({
            "name": name,
            "input": input,
            "decision": "deny",
            "reason": reason_code,
        }),
    }
}

fn load_registry_for_scope(
    scope: GovernanceScope,
    path: &Path,
) -> Result<SkillRegistry, SkillRegistryLoadError> {
    match scope {
        GovernanceScope::Global => skills::load_global_registry(path),
        GovernanceScope::Project => skills::load_project_registry(path),
        GovernanceScope::User => skills::load_user_registry(path),
    }
}

fn governance_error(command: &str, error_code: &str) -> (Value, i32) {
    (
        json!({
            "command": command,
            "status": "error",
            "error_code": error_code,
        }),
        1,
    )
}

fn governance_discover(args: &[String]) -> (Value, i32) {
    let mut scope = GovernanceScope::Project;
    let mut registry: Option<String> = None;
    let mut idx = 0usize;

    while idx < args.len() {
        let arg = args[idx].as_str();
        match arg {
            "--run-once" => {
                idx += 1;
                continue;
            }
            "--scope" => {
                let Some(value) = args.get(idx + 1) else {
                    return governance_error("discover", "missing_required_value");
                };
                if value.starts_with("--") {
                    return governance_error("discover", "missing_required_value");
                }
                let Some(parsed_scope) = parse_governance_scope(value) else {
                    return governance_error("discover", "invalid_scope");
                };
                scope = parsed_scope;
                idx += 2;
                continue;
            }
            "--registry" => {
                let Some(value) = args.get(idx + 1) else {
                    return governance_error("discover", "missing_required_value");
                };
                if value.starts_with("--") {
                    return governance_error("discover", "missing_required_value");
                }
                registry = Some(value.clone());
                idx += 2;
                continue;
            }
            _ => {}
        }

        if let Some(value) = arg.strip_prefix("--scope=") {
            if value.trim().is_empty() {
                return governance_error("discover", "missing_required_value");
            }
            let Some(parsed_scope) = parse_governance_scope(value) else {
                return governance_error("discover", "invalid_scope");
            };
            scope = parsed_scope;
            idx += 1;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--registry=") {
            if value.trim().is_empty() {
                return governance_error("discover", "missing_required_value");
            }
            registry = Some(value.to_string());
            idx += 1;
            continue;
        }

        idx += 1;
    }

    let registry_path = registry.unwrap_or_else(|| scope.default_registry_path().to_string());
    match load_registry_for_scope(scope, Path::new(&registry_path)) {
        Ok(registry_data) => {
            let candidates = registry_data
                .skills
                .iter()
                .map(|record| {
                    json!({
                        "name": record.name,
                        "trust_level": trust_level_str(&record.trust_level),
                        "source": record.source,
                    })
                })
                .collect::<Vec<_>>();

            (
                json!({
                    "command": "discover",
                    "status": "ok",
                    "scope": scope.as_str(),
                    "registry": registry_path,
                    "candidates": candidates,
                }),
                0,
            )
        }
        Err(err) => (
            json!({
                "command": "discover",
                "status": "failed",
                "scope": scope.as_str(),
                "registry": registry_path,
                "error_code": "registry_load_failed",
                "error": err.to_string(),
                "candidates": [],
            }),
            1,
        ),
    }
}

fn governance_install(args: &[String]) -> (Value, i32) {
    let mut name: Option<String> = None;
    let mut trust_level: Option<TrustLevel> = None;
    let mut ack = Ack::None;
    let mut idx = 0usize;

    while idx < args.len() {
        let arg = args[idx].as_str();
        match arg {
            "--run-once" => {
                idx += 1;
                continue;
            }
            "--ack" => {
                ack = Ack::Accepted;
                idx += 1;
                continue;
            }
            "--name" => {
                let Some(value) = args.get(idx + 1) else {
                    return governance_error("install", "missing_required_value");
                };
                if value.starts_with("--") {
                    return governance_error("install", "missing_required_value");
                }
                name = Some(value.clone());
                idx += 2;
                continue;
            }
            "--trust-level" => {
                let Some(value) = args.get(idx + 1) else {
                    return governance_error("install", "missing_required_value");
                };
                if value.starts_with("--") {
                    return governance_error("install", "missing_required_value");
                }
                let Some(parsed) = parse_trust_level(value) else {
                    return governance_error("install", "invalid_trust_level");
                };
                trust_level = Some(parsed);
                idx += 2;
                continue;
            }
            _ => {}
        }

        if let Some(value) = arg.strip_prefix("--name=") {
            if value.trim().is_empty() {
                return governance_error("install", "missing_required_value");
            }
            name = Some(value.to_string());
            idx += 1;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--trust-level=") {
            if value.trim().is_empty() {
                return governance_error("install", "missing_required_value");
            }
            let Some(parsed) = parse_trust_level(value) else {
                return governance_error("install", "invalid_trust_level");
            };
            trust_level = Some(parsed);
            idx += 1;
            continue;
        }

        idx += 1;
    }

    let Some(skill_name) = name else {
        return governance_error("install", "missing_required_value");
    };
    let Some(skill_trust) = trust_level else {
        return governance_error("install", "missing_required_value");
    };

    let candidate = SkillImportCandidate {
        record: SkillRecord {
            name: skill_name.clone(),
            trust_level: skill_trust.clone(),
            source: "manual:cli".to_string(),
            pinned_version: None,
            capabilities: Vec::new(),
        },
        scripts: Vec::new(),
        readme: None,
    };

    let plan = match import::evaluate_install(&candidate, ack) {
        Ok(plan) => plan,
        Err(err) => {
            return (
                json!({
                    "command": "install",
                    "status": "error",
                    "error_code": "invalid_candidate",
                    "error": err.to_string(),
                }),
                1,
            );
        }
    };

    match plan.status {
        import::InstallGateStatus::BlockedAckRequired => (
            json!({
                "command": "install",
                "status": "blocked",
                "error_code": "ack_required",
                "name": skill_name,
                "trust_level": trust_level_str(&skill_trust),
                "reasons": plan.reasons,
            }),
            1,
        ),
        import::InstallGateStatus::Allowed => (
            json!({
                "command": "install",
                "status": "ok",
                "name": skill_name,
                "trust_level": trust_level_str(&skill_trust),
                "reasons": plan.reasons,
                "finding_count": plan.findings.len(),
            }),
            0,
        ),
    }
}

fn governance_verify(args: &[String]) -> (Value, i32) {
    let mut scope = GovernanceScope::Project;
    let mut registry: Option<String> = None;
    let mut idx = 0usize;

    while idx < args.len() {
        let arg = args[idx].as_str();
        match arg {
            "--run-once" => {
                idx += 1;
                continue;
            }
            "--scope" => {
                let Some(value) = args.get(idx + 1) else {
                    return governance_error("verify", "missing_required_value");
                };
                if value.starts_with("--") {
                    return governance_error("verify", "missing_required_value");
                }
                let Some(parsed_scope) = parse_governance_scope(value) else {
                    return governance_error("verify", "invalid_scope");
                };
                scope = parsed_scope;
                idx += 2;
                continue;
            }
            "--registry" => {
                let Some(value) = args.get(idx + 1) else {
                    return governance_error("verify", "missing_required_value");
                };
                if value.starts_with("--") {
                    return governance_error("verify", "missing_required_value");
                }
                registry = Some(value.clone());
                idx += 2;
                continue;
            }
            _ => {}
        }

        if let Some(value) = arg.strip_prefix("--scope=") {
            if value.trim().is_empty() {
                return governance_error("verify", "missing_required_value");
            }
            let Some(parsed_scope) = parse_governance_scope(value) else {
                return governance_error("verify", "invalid_scope");
            };
            scope = parsed_scope;
            idx += 1;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--registry=") {
            if value.trim().is_empty() {
                return governance_error("verify", "missing_required_value");
            }
            registry = Some(value.to_string());
            idx += 1;
            continue;
        }

        idx += 1;
    }

    let registry_path = registry.unwrap_or_else(|| scope.default_registry_path().to_string());
    let mut checks = Vec::new();

    let registry_data = match load_registry_for_scope(scope, Path::new(&registry_path)) {
        Ok(data) => {
            checks.push(json!({
                "name": "registry_load",
                "status": "pass",
            }));
            data
        }
        Err(err) => {
            checks.push(json!({
                "name": "registry_load",
                "status": "fail",
                "detail": err.to_string(),
            }));
            return (
                json!({
                    "command": "verify",
                    "status": "failed",
                    "scope": scope.as_str(),
                    "registry": registry_path,
                    "checks": checks,
                }),
                1,
            );
        }
    };

    if registry_data.skills.is_empty() {
        checks.push(json!({
            "name": "registry_has_skills",
            "status": "fail",
            "detail": "no skills found",
        }));
    } else {
        checks.push(json!({
            "name": "registry_has_skills",
            "status": "pass",
            "count": registry_data.skills.len(),
        }));
    }

    let missing_pins = registry_data
        .skills
        .iter()
        .filter(|skill| {
            skill
                .pinned_version
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
        })
        .map(|skill| skill.name.clone())
        .collect::<Vec<_>>();
    if missing_pins.is_empty() {
        checks.push(json!({
            "name": "skills_pinned",
            "status": "pass",
        }));
    } else {
        checks.push(json!({
            "name": "skills_pinned",
            "status": "fail",
            "missing": missing_pins,
        }));
    }

    let has_failures = checks
        .iter()
        .any(|check| check.get("status").and_then(Value::as_str) == Some("fail"));
    let status = if has_failures { "failed" } else { "ok" };
    (
        json!({
            "command": "verify",
            "status": status,
            "scope": scope.as_str(),
            "registry": registry_path,
            "checks": checks,
        }),
        if has_failures { 1 } else { 0 },
    )
}

fn governance_enable_plugin(args: &[String]) -> (Value, i32) {
    let mut plugin: Option<String> = None;
    let mut domains: Vec<String> = Vec::new();
    let mut workspaces: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();
    let mut idx = 0usize;

    while idx < args.len() {
        let arg = args[idx].as_str();
        match arg {
            "--run-once" => {
                idx += 1;
                continue;
            }
            "--plugin" => {
                let Some(value) = args.get(idx + 1) else {
                    return governance_error("enable-plugin", "missing_required_value");
                };
                if value.starts_with("--") {
                    return governance_error("enable-plugin", "missing_required_value");
                }
                plugin = Some(value.clone());
                idx += 2;
                continue;
            }
            "--domains" => {
                let Some(value) = args.get(idx + 1) else {
                    return governance_error("enable-plugin", "missing_required_value");
                };
                if value.starts_with("--") {
                    return governance_error("enable-plugin", "missing_required_value");
                }
                domains = split_csv_values(value);
                idx += 2;
                continue;
            }
            "--workspaces" => {
                let Some(value) = args.get(idx + 1) else {
                    return governance_error("enable-plugin", "missing_required_value");
                };
                if value.starts_with("--") {
                    return governance_error("enable-plugin", "missing_required_value");
                }
                workspaces = split_csv_values(value);
                idx += 2;
                continue;
            }
            "--commands" => {
                let Some(value) = args.get(idx + 1) else {
                    return governance_error("enable-plugin", "missing_required_value");
                };
                if value.starts_with("--") {
                    return governance_error("enable-plugin", "missing_required_value");
                }
                commands = split_csv_values(value);
                idx += 2;
                continue;
            }
            _ => {}
        }

        if let Some(value) = arg.strip_prefix("--plugin=") {
            if value.trim().is_empty() {
                return governance_error("enable-plugin", "missing_required_value");
            }
            plugin = Some(value.to_string());
            idx += 1;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--domains=") {
            domains = split_csv_values(value);
            idx += 1;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--workspaces=") {
            workspaces = split_csv_values(value);
            idx += 1;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--commands=") {
            commands = split_csv_values(value);
            idx += 1;
            continue;
        }

        idx += 1;
    }

    let Some(plugin_name) = plugin else {
        return governance_error("enable-plugin", "missing_required_value");
    };
    if !plugin_name.eq_ignore_ascii_case("stagehand") {
        return governance_error("enable-plugin", "unsupported_plugin");
    }

    let mut reasons = Vec::new();
    if domains.is_empty() {
        reasons.push("domains_required".to_string());
    }
    if workspaces.is_empty() {
        reasons.push("workspaces_required".to_string());
    }
    if !reasons.is_empty() {
        return (
            json!({
                "command": "enable-plugin",
                "status": "blocked",
                "plugin": "stagehand",
                "reasons": reasons,
                "checks": [],
            }),
            1,
        );
    }

    let policy = plugins::stagehand_default_policy()
        .with_enabled(true)
        .with_domains(domains.iter().map(String::as_str))
        .with_workspaces(workspaces.iter().map(String::as_str))
        .with_commands([
            "ls", "cat", "grep", "sed", "awk", "find", "head", "tail", "wc",
        ]);

    let mut checks = Vec::new();
    let mut denied = false;

    for domain in &domains {
        let probe = if domain.starts_with("http://") || domain.starts_with("https://") {
            domain.clone()
        } else {
            format!("https://{domain}")
        };
        let check = decision_json(
            "domain_allowlist",
            domain,
            policy.evaluate(Action::ObserveUrl(probe)),
        );
        if check.get("decision").and_then(Value::as_str) == Some("deny") {
            denied = true;
        }
        checks.push(check);
    }

    for workspace in &workspaces {
        let check = decision_json(
            "workspace_allowlist",
            workspace,
            policy.evaluate(Action::ReadWorkspace(workspace.clone())),
        );
        if check.get("decision").and_then(Value::as_str) == Some("deny") {
            denied = true;
        }
        checks.push(check);
    }

    for command in &commands {
        let check = decision_json(
            "command_allowlist",
            command,
            policy.evaluate(Action::RunCommand(command.clone())),
        );
        if check.get("decision").and_then(Value::as_str) == Some("deny") {
            denied = true;
        }
        checks.push(check);
    }

    if denied {
        (
            json!({
                "command": "enable-plugin",
                "status": "blocked",
                "plugin": "stagehand",
                "checks": checks,
            }),
            1,
        )
    } else {
        (
            json!({
                "command": "enable-plugin",
                "status": "ok",
                "plugin": "stagehand",
                "checks": checks,
            }),
            0,
        )
    }
}

fn run_governance_command(args: &[String]) -> (Value, i32) {
    let mut idx = 0usize;
    while idx < args.len() && args[idx] == "--run-once" {
        idx += 1;
    }

    let Some(command) = args.get(idx).map(String::as_str) else {
        return governance_error("governance", "unknown_subcommand");
    };
    let command_args = &args[idx + 1..];

    match command {
        "discover" => governance_discover(command_args),
        "install" => governance_install(command_args),
        "verify" => governance_verify(command_args),
        "enable-plugin" => governance_enable_plugin(command_args),
        _ => governance_error("governance", "unknown_subcommand"),
    }
}

fn maybe_dispatch_governance(raw_args: &[String]) -> anyhow::Result<Option<i32>> {
    let Some(idx) = raw_args.iter().position(|arg| arg == "governance") else {
        return Ok(None);
    };

    let (payload, exit_code) = run_governance_command(&raw_args[idx + 1..]);
    let rendered = serde_json::to_string(&payload).context("failed to encode governance output")?;
    println!("{rendered}");
    Ok(Some(exit_code))
}

fn maybe_print_migrate_help_surface(raw_args: &[String]) -> bool {
    if raw_args == ["migrate", "--help"] || raw_args == ["migrate", "-h"] {
        println!("Orchestrator-to-core migration tools");
        println!();
        println!("Usage: odin-cli migrate <COMMAND>");
        println!();
        println!("Commands:");
        println!("  export    Export a migration bundle from the orchestrator");
        println!("  validate  Validate a migration bundle");
        println!("  import    Import a migration bundle into odin-core");
        println!("  help      Print this message or the help of the given subcommand(s)");
        return true;
    }
    false
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
        run_context: None,
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
                | "capacity"
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
                | "capacity"
        );
    }

    false
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
        .find(|node| node.has_tag_name("wake_up"))
        .and_then(|node| node.attribute("state").map(String::from));

    let states_elem = root.children().find(|node| node.has_tag_name("states"));
    let initial_state = states_elem
        .as_ref()
        .and_then(|node| node.attribute("initial_state").map(String::from));

    let mut states = Vec::new();
    if let Some(states_elem) = &states_elem {
        for state_node in states_elem
            .children()
            .filter(|node| node.has_tag_name("state"))
        {
            let id = state_node.attribute("id").unwrap_or_default().to_string();
            let is_end = state_node.attribute("end") == Some("true");
            let on_failure = state_node.attribute("on_failure").map(String::from);

            let transitions = state_node
                .children()
                .filter(|node| node.has_tag_name("transition"))
                .map(|transition| {
                    let target = transition
                        .attribute("target")
                        .unwrap_or_default()
                        .to_string();
                    let has_guard = transition
                        .children()
                        .any(|child| child.has_tag_name("guard"));
                    SassTransition { target, has_guard }
                })
                .collect::<Vec<_>>();

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

    let wake_up = match &skill.wake_up_state {
        Some(value) => value.clone(),
        None => {
            errors.push("wake_up element is required".to_string());
            return errors;
        }
    };

    match &skill.initial_state {
        Some(initial) => {
            if *initial != wake_up {
                errors.push(format!(
                    "wake_up state '{wake_up}' does not match initial_state '{initial}'"
                ));
            }
        }
        None => errors.push("states element with initial_state attribute is required".to_string()),
    }

    let state_ids: HashSet<&str> = skill.states.iter().map(|state| state.id.as_str()).collect();
    if !skill.states.iter().any(|state| state.is_end) {
        errors.push("at least one end state is required".to_string());
    }

    for state in &skill.states {
        if !state.is_end && state.on_failure.is_none() {
            errors.push(format!(
                "state '{}' missing on_failure (required for non-end states)",
                state.id
            ));
        }

        if let Some(ref target) = state.on_failure {
            if !state_ids.contains(target.as_str()) {
                errors.push(format!(
                    "state '{}' on_failure transitions to unknown target '{target}'",
                    state.id
                ));
            }
        }

        for transition in &state.transitions {
            if !state_ids.contains(transition.target.as_str()) {
                errors.push(format!(
                    "state '{}' transitions to unknown target '{}'",
                    state.id, transition.target
                ));
            }
        }

        if state.transitions.len() > 1
            && state
                .transitions
                .iter()
                .any(|transition| !transition.has_guard)
        {
            errors.push(format!(
                "state '{}' has decision transitions without guards",
                state.id
            ));
        }
    }

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
        for transition in &state.transitions {
            lines.push(format!("    {} --> {}", state.id, transition.target));
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
                for error in &errors {
                    eprintln!("validation failed: {error}");
                }
                process::exit(1);
            }
        }
        SkillCommand::Mermaid { file } => {
            let skill = parse_sass_skill(&file)?;
            let errors = validate_sass_skill(&skill);
            if !errors.is_empty() {
                for error in &errors {
                    eprintln!("validation failed: {error}");
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
        CliCommand::Capacity { command } => handle_capacity_command(command),
        CliCommand::Migrate { command } => match command {
            MigrateSubcommand::Export {
                source_root,
                odin_dir,
                out_dir,
            } => {
                let source_root = match source_root {
                    Some(path) => path,
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
                    Some(path) => path,
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
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_default();
                eprintln!("unknown migrate subcommand: {name}");
                process::exit(1);
            }
        },
    }
}

fn handle_capacity_command(command: CapacityCommand) -> anyhow::Result<()> {
    match command {
        CapacityCommand::Schedule {
            queue_state,
            agent_state,
            infra_state,
            cost_state,
            capacity_config,
            dry_run,
        } => {
            let queue_raw = fs::read_to_string(&queue_state)
                .with_context(|| format!("reading queue state: {}", queue_state.display()))?;
            let agent_raw = fs::read_to_string(&agent_state)
                .with_context(|| format!("reading agent state: {}", agent_state.display()))?;
            let infra_raw = fs::read_to_string(&infra_state)
                .with_context(|| format!("reading infra state: {}", infra_state.display()))?;
            let cost_raw = fs::read_to_string(&cost_state)
                .with_context(|| format!("reading cost state: {}", cost_state.display()))?;

            let queue_val: Value =
                serde_json::from_str(&queue_raw).context("parsing queue state JSON")?;
            let agent_val: Value =
                serde_json::from_str(&agent_raw).context("parsing agent state JSON")?;
            let infra_val: Value =
                serde_json::from_str(&infra_raw).context("parsing infra state JSON")?;
            let cost_val: Value =
                serde_json::from_str(&cost_raw).context("parsing cost state JSON")?;

            // Build ScheduleInput from state files
            let queue_depths: std::collections::HashMap<String, u32> =
                serde_json::from_value(queue_val["queue_depths"].clone()).unwrap_or_default();
            let held_depth = queue_val["held_depth"].as_u64().unwrap_or(0) as u32;
            let deferred_depth = queue_val["deferred_depth"].as_u64().unwrap_or(0) as u32;
            let agent_count = agent_val["agent_count"].as_u64().unwrap_or(0) as u32;
            let infra: odin_capacity::capacity::InfraState =
                serde_json::from_value(infra_val).context("parsing infra state")?;
            let spend_today_usd = cost_val["spend_today_usd"].as_f64().unwrap_or(0.0);
            let spend_ceiling_usd = cost_val["spend_ceiling_usd"].as_f64().unwrap_or(50.0);

            let input = odin_capacity::scheduler::ScheduleInput {
                queue_depths,
                held_depth,
                deferred_depth,
                agent_count,
                infra,
                spend_today_usd,
                spend_ceiling_usd,
            };

            // Load config or use defaults
            let (cap_config, cb_config, overflow_config) =
                if let Some(ref config_path) = capacity_config {
                    let config_raw = fs::read_to_string(config_path)
                        .with_context(|| format!("reading config: {}", config_path.display()))?;
                    let config_val: Value =
                        serde_yml::from_str(&config_raw).context("parsing capacity config YAML")?;

                    let cap: odin_capacity::capacity::CapacityConfig =
                        serde_json::from_value(config_val["capacity"].clone()).unwrap_or_default();
                    let cb: odin_capacity::circuit_breaker::CircuitBreakerConfig =
                        serde_json::from_value(config_val["circuit_breaker"].clone())
                            .unwrap_or_default();
                    let ov: odin_capacity::overflow::OverflowConfig =
                        serde_json::from_value(config_val["overflow"].clone()).unwrap_or_default();
                    (cap, cb, ov)
                } else {
                    (
                        odin_capacity::capacity::CapacityConfig::default(),
                        odin_capacity::circuit_breaker::CircuitBreakerConfig::default(),
                        odin_capacity::overflow::OverflowConfig::default(),
                    )
                };

            // Load role_priority from config YAML if present, else default
            let role_priority = if let Some(ref config_path) = capacity_config {
                let rp_raw = fs::read_to_string(config_path).ok();
                rp_raw.and_then(|raw| {
                    let yml_val: serde_yml::Value = serde_yml::from_str(&raw).ok()?;
                    yml_val["capacity"]["role_priority"]
                        .as_sequence()
                        .map(|seq| {
                            seq.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        })
                })
            } else {
                None
            }
            .unwrap_or_else(|| {
                vec![
                    "ops".into(),
                    "developer".into(),
                    "qa-lead".into(),
                    "strategist".into(),
                    "sm".into(),
                ]
            });

            let scheduler = odin_capacity::scheduler::Scheduler {
                capacity_config: cap_config,
                cb_config,
                overflow_config,
                role_priority,
            };

            let cost_controller = odin_capacity::cost_controller::CostController::new(
                odin_capacity::cost_controller::CostConfig {
                    daily_ceiling_usd: spend_ceiling_usd,
                    ..Default::default()
                },
            );

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            if dry_run {
                eprintln!("DRY-RUN capacity schedule");
            }

            let decision = scheduler.schedule(&input, &[], &cost_controller, now);
            let json =
                serde_json::to_string_pretty(&decision).context("serializing schedule decision")?;
            println!("{json}");
            Ok(())
        }
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
    if let Some(code) = maybe_dispatch_governance(&raw_args[1..])? {
        process::exit(code);
    }
    if maybe_print_migrate_help_surface(&raw_args[1..]) {
        return Ok(());
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
