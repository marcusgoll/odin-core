use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;
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

fn parse_cli_config(args: &[String]) -> CliConfig {
    let mut cfg = CliConfig::default();
    let mut index = 0;

    while let Some(arg) = args.get(index) {
        match arg.as_str() {
            "--config" => {
                if let Some(path) = args.get(index + 1) {
                    cfg.config_path = path.to_string();
                    index += 1;
                }
            }
            "--legacy-root" => {
                if let Some(path) = args.get(index + 1) {
                    cfg.legacy_root = Some(PathBuf::from(path));
                    index += 1;
                }
            }
            "--legacy-odin-dir" => {
                if let Some(path) = args.get(index + 1) {
                    cfg.legacy_odin_dir = PathBuf::from(path);
                    index += 1;
                }
            }
            "--plugins-root" => {
                if let Some(path) = args.get(index + 1) {
                    cfg.plugins_root = PathBuf::from(path);
                    index += 1;
                }
            }
            "--task-file" => {
                if let Some(path) = args.get(index + 1) {
                    cfg.task_file = Some(PathBuf::from(path));
                    index += 1;
                }
            }
            "--run-once" => {
                cfg.run_once = true;
            }
            _ => {}
        }
        index += 1;
    }

    cfg
}

#[derive(Debug)]
struct SkillTransition {
    target: Option<String>,
    has_guard: bool,
}

#[derive(Debug)]
struct SkillState {
    id: String,
    end: bool,
    on_failure: Option<String>,
    transitions: Vec<SkillTransition>,
}

#[derive(Debug)]
struct ParsedSkill {
    wake_up_state: Option<String>,
    states: Vec<SkillState>,
}

fn run_skill_validate(path: &str) -> anyhow::Result<()> {
    let skill = load_skill(path)?;
    let mut errors = Vec::new();
    validate_skill(&skill, &mut errors);

    if !errors.is_empty() {
        return Err(anyhow!("validation failed:\n- {}", errors.join("\n- ")));
    }

    println!("validation ok");
    Ok(())
}

fn run_skill_mermaid(path: &str) -> anyhow::Result<()> {
    let skill = load_skill(path)?;
    let mut errors = Vec::new();
    validate_skill(&skill, &mut errors);

    if !errors.is_empty() {
        return Err(anyhow!("validation failed:\n- {}", errors.join("\n- ")));
    }

    println!("stateDiagram-v2");
    if let Some(wake_up_state) = &skill.wake_up_state {
        println!("    %% wake_up: {wake_up_state}");
        println!("    [*] --> {wake_up_state}");
    }

    for state in &skill.states {
        for transition in &state.transitions {
            if let Some(target) = &transition.target {
                println!("    {} --> {}", state.id, target);
            }
        }
    }

    Ok(())
}

fn load_skill(path: &str) -> anyhow::Result<ParsedSkill> {
    let xml = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    let document = roxmltree::Document::parse(&xml)
        .with_context(|| format!("failed to parse XML in {path}"))?;

    let mut errors = Vec::new();
    let parsed = parse_skill(&document, &mut errors);

    if !errors.is_empty() {
        return Err(anyhow!("validation failed:\n- {}", errors.join("\n- ")));
    }

    parsed.ok_or_else(|| anyhow!("validation failed:\n- unknown parse error"))
}

fn parse_skill(
    document: &roxmltree::Document<'_>,
    errors: &mut Vec<String>,
) -> Option<ParsedSkill> {
    let root = document.root_element();
    if root.tag_name().name() != "skill" {
        errors.push("root element must be <skill>".to_string());
        return None;
    }

    let wake_up_node = root
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "wake_up");
    let wake_up_state = wake_up_node
        .and_then(|node| node.attribute("state"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    if wake_up_node.is_none() {
        errors.push("missing wake_up state".to_string());
    } else if wake_up_state.is_none() {
        errors.push("wake_up state must not be empty".to_string());
    }

    let states_node = match root
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "states")
    {
        Some(node) => node,
        None => {
            errors.push("missing <states> block".to_string());
            return None;
        }
    };

    let mut states = Vec::new();

    for state_node in states_node
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "state")
    {
        let id = match state_node
            .attribute("id")
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(value) => value.to_string(),
            None => {
                errors.push("state is missing required id attribute".to_string());
                continue;
            }
        };

        let end = state_node
            .attribute("end")
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let on_failure = state_node
            .attribute("on_failure")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);

        let mut transitions = Vec::new();
        for transition_node in state_node
            .children()
            .filter(|node| node.is_element() && node.tag_name().name() == "transition")
        {
            let target = transition_node
                .attribute("target")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);

            if target.is_none() {
                errors.push(format!("state '{id}' has transition missing target"));
            }

            let guard_attr = transition_node
                .attribute("guard")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some();
            let guard_child = transition_node.children().any(|node| {
                node.is_element()
                    && node.tag_name().name() == "guard"
                    && (node
                        .attribute("expression")
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .is_some()
                        || node
                            .text()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .is_some())
            });

            transitions.push(SkillTransition {
                target,
                has_guard: guard_attr || guard_child,
            });
        }

        states.push(SkillState {
            id,
            end,
            on_failure,
            transitions,
        });
    }

    if states.is_empty() {
        errors.push("no states found under <states>".to_string());
    }

    Some(ParsedSkill {
        wake_up_state,
        states,
    })
}

fn validate_skill(skill: &ParsedSkill, errors: &mut Vec<String>) {
    let state_ids: HashSet<&str> = skill.states.iter().map(|state| state.id.as_str()).collect();

    if let Some(wake_up_state) = &skill.wake_up_state {
        if !state_ids.contains(wake_up_state.as_str()) {
            errors.push(format!("wake_up state '{wake_up_state}' does not exist"));
        }
    }

    let end_state_count = skill.states.iter().filter(|state| state.end).count();
    if end_state_count == 0 {
        errors.push("at least one end state is required".to_string());
    }

    for state in &skill.states {
        let mut unguarded_transitions = 0usize;

        for transition in &state.transitions {
            if let Some(target) = &transition.target {
                if !state_ids.contains(target.as_str()) {
                    errors.push(format!(
                        "state '{}' transitions to unknown target '{}'",
                        state.id, target
                    ));
                }
            }

            if !transition.has_guard {
                unguarded_transitions += 1;
            }
        }

        if state.transitions.len() > 1 && unguarded_transitions > 0 {
            errors.push(format!(
                "state '{}' has decision transitions without guards",
                state.id
            ));
        }
    }

    for state in skill.states.iter().filter(|state| !state.end) {
        match &state.on_failure {
            Some(target) if !state_ids.contains(target.as_str()) => {
                errors.push(format!(
                    "state '{}' has on_failure target '{}' that does not exist",
                    state.id, target
                ));
            }
            Some(_) => {}
            None => errors.push(format!(
                "non-end state '{}' is missing on_failure",
                state.id
            )),
        }
    }
}

fn maybe_run_skill_command(args: &[String]) -> Option<anyhow::Result<()>> {
    if args.first().map(String::as_str) != Some("skill") {
        return None;
    }

    if args.len() != 3 {
        return Some(Err(anyhow!(
            "usage: odin-cli skill <validate|mermaid> <path>"
        )));
    }

    let command = args[1].as_str();
    let path = &args[2];
    match command {
        "validate" => Some(run_skill_validate(path)),
        "mermaid" => Some(run_skill_mermaid(path)),
        _ => Some(Err(anyhow!(
            "unsupported skill command, expected: odin-cli skill <validate|mermaid> <path>"
        ))),
    }
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
    let args: Vec<String> = env::args().skip(1).collect();
    if let Some(result) = maybe_run_skill_command(&args) {
        return result;
    }

    let cfg = parse_cli_config(&args);
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
