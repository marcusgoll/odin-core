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
            "stagehand",
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

    assert!(output.status.success(), "verify should succeed");

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
