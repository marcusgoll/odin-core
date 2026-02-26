use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn write_project_registry(temp_dir: &TempDir) -> PathBuf {
    let path = temp_dir.path().join("skills.project.yaml");
    let content = r#"
schema_version: 1
scope: project
skills:
  - name: brainstorming
    trust_level: trusted
    source: project:/skills/brainstorming
"#;
    fs::write(&path, content).expect("write registry");
    path
}

fn parse_stdout_json(output: &Output) -> Value {
    let stdout = String::from_utf8(output.stdout.clone()).expect("utf8 stdout");
    serde_json::from_str(&stdout).expect("stdout json")
}

#[test]
fn governance_discover_prints_candidates() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let registry_path = write_project_registry(&temp_dir);

    let output = Command::new(assert_cmd::cargo::cargo_bin!("odin-cli"))
        .args(["governance", "discover", "--scope", "project", "--registry"])
        .arg(&registry_path)
        .arg("--run-once")
        .output()
        .expect("run discover");

    assert!(output.status.success(), "discover command should succeed");

    let json = parse_stdout_json(&output);
    assert_eq!(json["command"], "discover");
    assert_eq!(json["status"], "ok");
    assert!(json["candidates"].is_array());
    assert!(json["candidates"].to_string().contains("brainstorming"));
}

#[test]
fn governance_install_requires_ack_for_untrusted() {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("odin-cli"))
        .args([
            "governance",
            "install",
            "--name",
            "suspicious-skill",
            "--trust-level",
            "untrusted",
            "--run-once",
        ])
        .output()
        .expect("run install");

    assert!(
        !output.status.success(),
        "install should be blocked without --ack"
    );

    let json = parse_stdout_json(&output);
    assert_eq!(json["command"], "install");
    assert_eq!(json["status"], "blocked");
    assert_eq!(json["error_code"], "ack_required");
}

#[test]
fn governance_enable_plugin_stagehand_requires_explicit_domains_and_workspaces() {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("odin-cli"))
        .args([
            "governance",
            "enable-plugin",
            "--plugin",
            "Stagehand",
            "--run-once",
        ])
        .output()
        .expect("run enable-plugin");

    assert!(
        !output.status.success(),
        "stagehand enable should be blocked without policy scope"
    );

    let json = parse_stdout_json(&output);
    assert_eq!(json["command"], "enable-plugin");
    assert_eq!(json["status"], "blocked");
    assert!(json["reasons"].to_string().contains("domains_required"));
    assert!(json["reasons"].to_string().contains("workspaces_required"));
}

#[test]
fn governance_verify_prints_pass_fail_checks() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let registry_path = write_project_registry(&temp_dir);

    let output = Command::new(assert_cmd::cargo::cargo_bin!("odin-cli"))
        .args(["governance", "verify", "--scope", "project", "--registry"])
        .arg(&registry_path)
        .arg("--run-once")
        .output()
        .expect("run verify");

    assert!(
        !output.status.success(),
        "verify should return non-zero if any check fails"
    );

    let json = parse_stdout_json(&output);
    assert_eq!(json["command"], "verify");
    assert!(json["checks"].is_array());

    let checks = json["checks"].as_array().expect("checks array");
    assert!(
        checks.iter().any(|check| check["status"] == "pass"),
        "expected at least one passing check"
    );
    assert!(
        checks.iter().any(|check| check["status"] == "fail"),
        "expected at least one failing check"
    );
}

#[test]
fn governance_verify_without_registry_uses_non_example_default_and_fails_when_missing() {
    let temp_dir = TempDir::new().expect("create temp dir");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("odin-cli"))
        .current_dir(temp_dir.path())
        .args(["governance", "verify", "--scope", "project", "--run-once"])
        .output()
        .expect("run verify without explicit registry");

    assert!(
        !output.status.success(),
        "verify should fail when default registry path is missing"
    );

    let json = parse_stdout_json(&output);
    assert_eq!(json["command"], "verify");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["registry"], "config/skills.project.yaml");

    let checks = json["checks"].as_array().expect("checks array");
    let registry_check = checks
        .iter()
        .find(|check| check["name"] == "registry_load")
        .expect("registry_load check");
    assert_eq!(registry_check["status"], "fail");
}

#[test]
fn governance_discover_invalid_scope_returns_json_error() {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("odin-cli"))
        .args(["governance", "discover", "--scope", "invalid", "--run-once"])
        .output()
        .expect("run discover invalid scope");

    assert!(!output.status.success(), "invalid scope should fail");

    let json = parse_stdout_json(&output);
    assert_eq!(json["command"], "discover");
    assert_eq!(json["status"], "error");
    assert_eq!(json["error_code"], "invalid_scope");
}

#[test]
fn governance_install_invalid_trust_level_returns_json_error() {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("odin-cli"))
        .args([
            "governance",
            "install",
            "--name",
            "test-skill",
            "--trust-level",
            "not-a-level",
            "--run-once",
        ])
        .output()
        .expect("run install invalid trust-level");

    assert!(!output.status.success(), "invalid trust-level should fail");

    let json = parse_stdout_json(&output);
    assert_eq!(json["command"], "install");
    assert_eq!(json["status"], "error");
    assert_eq!(json["error_code"], "invalid_trust_level");
}

#[test]
fn governance_discover_missing_required_value_returns_json_error() {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("odin-cli"))
        .args(["governance", "discover", "--scope", "--run-once"])
        .output()
        .expect("run discover missing value");

    assert!(
        !output.status.success(),
        "missing required option value should fail"
    );

    let json = parse_stdout_json(&output);
    assert_eq!(json["command"], "discover");
    assert_eq!(json["status"], "error");
    assert_eq!(json["error_code"], "missing_required_value");
}

#[test]
fn governance_dispatch_handles_global_flag_before_subcommand() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let registry_path = write_project_registry(&temp_dir);

    let output = Command::new(assert_cmd::cargo::cargo_bin!("odin-cli"))
        .args([
            "--run-once",
            "governance",
            "discover",
            "--scope",
            "project",
            "--registry",
        ])
        .arg(&registry_path)
        .output()
        .expect("run discover with leading global flag");

    assert!(
        output.status.success(),
        "discover via leading global flag should succeed"
    );

    let json = parse_stdout_json(&output);
    assert_eq!(json["command"], "discover");
    assert_eq!(json["status"], "ok");
}

#[test]
fn governance_dispatch_scans_past_unknown_leading_args() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let registry_path = write_project_registry(&temp_dir);

    let output = Command::new(assert_cmd::cargo::cargo_bin!("odin-cli"))
        .args([
            "--unknown",
            "foo",
            "governance",
            "verify",
            "--scope",
            "project",
            "--registry",
        ])
        .arg(&registry_path)
        .arg("--run-once")
        .output()
        .expect("run verify with unknown leading args");

    assert!(
        !output.status.success(),
        "verify should still run and fail checks for this registry"
    );

    let json = parse_stdout_json(&output);
    assert_eq!(json["command"], "verify");
    assert!(json["checks"].is_array());
}

#[test]
fn governance_enable_plugin_stagehand_allows_url_form_domain_probe() {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("odin-cli"))
        .args([
            "governance",
            "enable-plugin",
            "--plugin",
            "stagehand",
            "--domains",
            "https://example.com",
            "--workspaces",
            "/tmp",
            "--run-once",
        ])
        .output()
        .expect("run stagehand enable with url domain");

    assert!(
        output.status.success(),
        "stagehand enable should succeed with required policy inputs"
    );

    let json = parse_stdout_json(&output);
    assert_eq!(json["command"], "enable-plugin");
    assert_eq!(json["status"], "ok");

    let checks = json["checks"].as_array().expect("checks array");
    let domain_check = checks
        .iter()
        .find(|check| check["name"] == "domain_allowlist")
        .expect("domain check");
    assert_eq!(domain_check["decision"], "allow");
}

#[test]
fn governance_enable_plugin_stagehand_returns_blocked_when_policy_checks_deny() {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("odin-cli"))
        .args([
            "governance",
            "enable-plugin",
            "--plugin",
            "stagehand",
            "--domains",
            "/",
            "--workspaces",
            "/",
            "--run-once",
        ])
        .output()
        .expect("run stagehand enable with denied policy checks");

    assert!(
        !output.status.success(),
        "stagehand enable should fail when checks deny"
    );

    let json = parse_stdout_json(&output);
    assert_eq!(json["command"], "enable-plugin");
    assert_eq!(json["status"], "blocked");

    let checks = json["checks"].as_array().expect("checks array");
    assert!(
        checks.iter().any(|check| check["decision"] == "deny"),
        "expected at least one denied policy check"
    );
}

#[test]
fn governance_enable_plugin_stagehand_blocks_when_later_values_deny() {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("odin-cli"))
        .args([
            "governance",
            "enable-plugin",
            "--plugin",
            "stagehand",
            "--domains",
            "example.com,/",
            "--workspaces",
            "/tmp,/",
            "--run-once",
        ])
        .output()
        .expect("run stagehand enable with mixed valid/invalid values");

    assert!(
        !output.status.success(),
        "stagehand enable should fail when any later value is denied"
    );

    let json = parse_stdout_json(&output);
    assert_eq!(json["command"], "enable-plugin");
    assert_eq!(json["status"], "blocked");

    let checks = json["checks"].as_array().expect("checks array");
    assert!(
        checks.iter().any(|check| check["decision"] == "deny"),
        "expected at least one denied policy check"
    );
}

#[test]
fn governance_enable_plugin_stagehand_blocks_when_command_scope_denies() {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("odin-cli"))
        .args([
            "governance",
            "enable-plugin",
            "--plugin",
            "stagehand",
            "--domains",
            "example.com",
            "--workspaces",
            "/tmp",
            "--commands",
            "ls,cat file",
            "--run-once",
        ])
        .output()
        .expect("run stagehand enable with denied command scope");

    assert!(
        !output.status.success(),
        "stagehand enable should fail when any command check denies"
    );

    let json = parse_stdout_json(&output);
    assert_eq!(json["command"], "enable-plugin");
    assert_eq!(json["status"], "blocked");

    let checks = json["checks"].as_array().expect("checks array");
    let command_checks = checks
        .iter()
        .filter(|check| check["name"] == "command_allowlist")
        .collect::<Vec<_>>();
    assert_eq!(
        command_checks.len(),
        2,
        "expected one check per command value"
    );
    assert!(
        command_checks
            .iter()
            .any(|check| check["decision"] == "deny"),
        "expected a denied command check"
    );
}
