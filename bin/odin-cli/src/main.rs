use std::env;
use std::fs;
use std::path::PathBuf;
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
use odin_plugin_protocol::{ActionRequest, CapabilityRequest, RiskTier};
use odin_policy_engine::StaticPolicyEngine;

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
            "connect" | "start" | "tui" | "inbox" | "gateway" | "verify"
        );
    }

    if let Some(token) = raw_args.get(idx).map(String::as_str) {
        return matches!(
            token,
            "connect" | "start" | "tui" | "inbox" | "gateway" | "verify"
        );
    }

    false
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
