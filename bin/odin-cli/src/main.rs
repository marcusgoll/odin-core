use std::env;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use anyhow::Context;
use odin_audit::NoopAuditSink;
use odin_compat_bash::{
    BashBackendStateAdapter, BashFailoverAdapter, BashTaskIngressAdapter, LegacyScriptPaths,
};
use odin_core_runtime::{
    BackendState, DryRunExecutor, ExternalProcessPluginRunner, OrchestratorRuntime, TaskIngress,
};
use odin_migration::{run as run_migration_command, MigrationCommand};
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

const MIGRATE_HELP: &str = "\
Usage: odin-cli migrate <COMMAND>

Commands:
  export    Export legacy data into migration artifacts
  validate  Validate migration artifacts
  import    Import migration artifacts into odin-core
";

const MIGRATE_EXPORT_HELP: &str = "\
Usage: odin-cli migrate export

Export legacy data into migration artifacts.
";

const MIGRATE_VALIDATE_HELP: &str = "\
Usage: odin-cli migrate validate

Validate migration artifacts.
";

const MIGRATE_IMPORT_HELP: &str = "\
Usage: odin-cli migrate import

Import migration artifacts into odin-core.
";

fn handle_migrate_surface() -> anyhow::Result<bool> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("migrate") {
        return Ok(false);
    }

    match args.get(1).map(String::as_str) {
        None | Some("--help") | Some("-h") => {
            println!("{MIGRATE_HELP}");
            Ok(true)
        }
        Some("export") => match args.get(2).map(String::as_str) {
            Some("--help") | Some("-h") => {
                println!("{MIGRATE_EXPORT_HELP}");
                Ok(true)
            }
            None | Some(_) => {
                run_migration_command(MigrationCommand::Export)?;
                Ok(true)
            }
        },
        Some("validate") => match args.get(2).map(String::as_str) {
            Some("--help") | Some("-h") => {
                println!("{MIGRATE_VALIDATE_HELP}");
                Ok(true)
            }
            None | Some(_) => {
                run_migration_command(MigrationCommand::Validate)?;
                Ok(true)
            }
        },
        Some("import") => match args.get(2).map(String::as_str) {
            Some("--help") | Some("-h") => {
                println!("{MIGRATE_IMPORT_HELP}");
                Ok(true)
            }
            None | Some(_) => {
                run_migration_command(MigrationCommand::Import)?;
                Ok(true)
            }
        },
        Some(other) => anyhow::bail!("unknown migrate subcommand: {other}"),
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

fn main() -> anyhow::Result<()> {
    if handle_migrate_surface()? {
        return Ok(());
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
