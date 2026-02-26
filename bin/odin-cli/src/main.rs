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
  inventory Create migration inventory snapshot
";

const MIGRATE_EXPORT_HELP: &str = "\
Usage: odin-cli migrate export --source-root <dir> --odin-dir <dir> --out <dir>

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

const MIGRATE_INVENTORY_HELP: &str = "\
Usage: odin-cli migrate inventory --input <dir> --output <path>

Create inventory snapshot from migration fixture data.
";

fn parse_export_flags(args: &[String]) -> anyhow::Result<(PathBuf, PathBuf, PathBuf)> {
    let mut source_root: Option<PathBuf> = None;
    let mut odin_dir: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--source-root" => {
                let Some(value) = args.get(index + 1) else {
                    anyhow::bail!("missing value for --source-root");
                };
                source_root = Some(PathBuf::from(value));
                index += 2;
            }
            "--odin-dir" => {
                let Some(value) = args.get(index + 1) else {
                    anyhow::bail!("missing value for --odin-dir");
                };
                odin_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--out" => {
                let Some(value) = args.get(index + 1) else {
                    anyhow::bail!("missing value for --out");
                };
                out_dir = Some(PathBuf::from(value));
                index += 2;
            }
            other => anyhow::bail!("unknown migrate export argument: {other}"),
        }
    }

    let source_root =
        source_root.ok_or_else(|| anyhow::anyhow!("missing required flag: --source-root"))?;
    let odin_dir = odin_dir.ok_or_else(|| anyhow::anyhow!("missing required flag: --odin-dir"))?;
    let out_dir = out_dir.ok_or_else(|| anyhow::anyhow!("missing required flag: --out"))?;

    Ok((source_root, odin_dir, out_dir))
}

fn parse_inventory_flags(args: &[String]) -> anyhow::Result<(PathBuf, PathBuf)> {
    let mut input_dir: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--input" => {
                let Some(value) = args.get(index + 1) else {
                    anyhow::bail!("missing value for --input");
                };
                input_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--output" => {
                let Some(value) = args.get(index + 1) else {
                    anyhow::bail!("missing value for --output");
                };
                output_path = Some(PathBuf::from(value));
                index += 2;
            }
            other => anyhow::bail!("unknown migrate inventory argument: {other}"),
        }
    }

    let input_dir = input_dir.ok_or_else(|| anyhow::anyhow!("missing required flag: --input"))?;
    let output_path =
        output_path.ok_or_else(|| anyhow::anyhow!("missing required flag: --output"))?;
    Ok((input_dir, output_path))
}

fn reject_trailing_migrate_args(command: &str, args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        return Ok(());
    }

    anyhow::bail!(
        "unexpected argument(s) for migrate {command}: {}",
        args.join(" ")
    );
}

fn handle_migrate_args(args: &[String]) -> anyhow::Result<bool> {
    if args.first().map(String::as_str) != Some("migrate") {
        return Ok(false);
    }

    match args.get(1).map(String::as_str) {
        None | Some("--help") | Some("-h") => {
            println!("{MIGRATE_HELP}");
            Ok(true)
        }
        Some("export") => {
            let sub_args = &args[2..];
            if matches!(
                sub_args.first().map(String::as_str),
                Some("--help") | Some("-h")
            ) {
                if sub_args.len() != 1 {
                    reject_trailing_migrate_args("export", &sub_args[1..])?;
                }
                println!("{MIGRATE_EXPORT_HELP}");
            } else {
                let (source_root, odin_dir, out_dir) = parse_export_flags(sub_args)?;
                run_migration_command(MigrationCommand::Export {
                    source_root,
                    odin_dir,
                    out_dir,
                })?;
            }
            Ok(true)
        }
        Some("validate") => {
            let sub_args = &args[2..];
            if matches!(
                sub_args.first().map(String::as_str),
                Some("--help") | Some("-h")
            ) {
                if sub_args.len() != 1 {
                    reject_trailing_migrate_args("validate", &sub_args[1..])?;
                }
                println!("{MIGRATE_VALIDATE_HELP}");
            } else {
                reject_trailing_migrate_args("validate", sub_args)?;
                run_migration_command(MigrationCommand::Validate)?;
            }
            Ok(true)
        }
        Some("import") => {
            let sub_args = &args[2..];
            if matches!(
                sub_args.first().map(String::as_str),
                Some("--help") | Some("-h")
            ) {
                if sub_args.len() != 1 {
                    reject_trailing_migrate_args("import", &sub_args[1..])?;
                }
                println!("{MIGRATE_IMPORT_HELP}");
            } else {
                reject_trailing_migrate_args("import", sub_args)?;
                run_migration_command(MigrationCommand::Import)?;
            }
            Ok(true)
        }
        Some("inventory") => {
            let sub_args = &args[2..];
            if matches!(
                sub_args.first().map(String::as_str),
                Some("--help") | Some("-h")
            ) {
                if sub_args.len() != 1 {
                    reject_trailing_migrate_args("inventory", &sub_args[1..])?;
                }
                println!("{MIGRATE_INVENTORY_HELP}");
            } else {
                let (input_dir, output_path) = parse_inventory_flags(&args[2..])?;
                run_migration_command(MigrationCommand::Inventory {
                    input_dir,
                    output_path,
                })?;
            }
            Ok(true)
        }
        Some(other) => anyhow::bail!("unknown migrate subcommand: {other}"),
    }
}

fn handle_migrate_surface() -> anyhow::Result<bool> {
    let args: Vec<String> = env::args().skip(1).collect();
    handle_migrate_args(&args)
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

#[cfg(test)]
mod tests {
    use super::handle_migrate_args;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| part.to_string()).collect()
    }

    #[test]
    fn migrate_validate_import_reject_unexpected_trailing_args() {
        for command in ["validate", "import"] {
            let result = handle_migrate_args(&args(&["migrate", command, "extra"]));
            let err = result.expect_err("trailing args should fail");
            assert!(
                err.to_string().contains(&format!(
                    "unexpected argument(s) for migrate {command}: extra"
                )),
                "unexpected error for command {command}: {err:#}"
            );
        }
    }

    #[test]
    fn migrate_export_rejects_missing_required_flags() {
        let result =
            handle_migrate_args(&args(&["migrate", "export", "--source-root", "/tmp/src"]));
        let err = result.expect_err("missing export flags should fail");
        assert!(
            err.to_string()
                .contains("missing required flag: --odin-dir"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn migrate_export_without_required_flags_fails() {
        let result = handle_migrate_args(&args(&["migrate", "export"]));
        let err = result.expect_err("missing export flags should fail");
        assert!(
            err.to_string()
                .contains("missing required flag: --source-root"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn migrate_export_rejects_unknown_flag() {
        let result = handle_migrate_args(&args(&[
            "migrate",
            "export",
            "--source-root",
            "/tmp/src",
            "--odin-dir",
            "/tmp/odin",
            "--out",
            "/tmp/out",
            "--bogus",
        ]));
        let err = result.expect_err("unknown export flags should fail");
        assert!(
            err.to_string()
                .contains("unknown migrate export argument: --bogus"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn migrate_export_help_rejects_trailing_args() {
        let result = handle_migrate_args(&args(&["migrate", "export", "--help", "extra"]));
        let err = result.expect_err("help with trailing args should fail");
        assert!(
            err.to_string()
                .contains("unexpected argument(s) for migrate export: extra"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn migrate_inventory_rejects_missing_required_flags() {
        let result = handle_migrate_args(&args(&["migrate", "inventory", "--input", "/tmp/in"]));
        let err = result.expect_err("missing --output should fail");
        assert!(
            err.to_string().contains("missing required flag: --output"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn migrate_inventory_rejects_invalid_flag() {
        let result = handle_migrate_args(&args(&["migrate", "inventory", "--bogus", "value"]));
        let err = result.expect_err("unknown flags should fail");
        assert!(
            err.to_string()
                .contains("unknown migrate inventory argument: --bogus"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn migrate_inventory_help_rejects_trailing_args() {
        let result = handle_migrate_args(&args(&["migrate", "inventory", "--help", "extra"]));
        let err = result.expect_err("help with trailing args should fail");
        assert!(
            err.to_string()
                .contains("unexpected argument(s) for migrate inventory: extra"),
            "unexpected error: {err:#}"
        );
    }
}
