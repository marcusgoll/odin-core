use std::time::Duration;

use predicates::str::contains;

fn assert_dry_run_contract(args: &[&str], expected_fragment: &str) {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("odin-cli");
    cmd.args(args).timeout(Duration::from_secs(3));

    cmd.assert().success().stdout(contains(expected_fragment));
}

#[test]
fn connect_dry_run_contract() {
    assert_dry_run_contract(
        &["connect", "claude", "oauth", "--dry-run"],
        "DRY-RUN connect provider=claude auth=oauth",
    );
}

#[test]
fn start_dry_run_contract() {
    assert_dry_run_contract(&["start", "--dry-run"], "DRY-RUN start");
}

#[test]
fn tui_dry_run_contract() {
    assert_dry_run_contract(&["tui", "--dry-run"], "DRY-RUN tui");
}

#[test]
fn inbox_add_dry_run_contract() {
    assert_dry_run_contract(
        &["inbox", "add", "bootstrap task", "--dry-run"],
        "DRY-RUN inbox add title=bootstrap task",
    );
}

#[test]
fn inbox_add_dry_run_includes_normalized_fields() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("odin-cli");
    cmd.args(["inbox", "add", "bootstrap task", "--dry-run"])
        .timeout(Duration::from_secs(3));

    cmd.assert()
        .success()
        .stdout(contains("normalized inbox item"))
        .stdout(contains("title=bootstrap task"))
        .stdout(contains("raw_text=bootstrap task"))
        .stdout(contains("source=cli"))
        .stdout(contains("timestamp="));
}

#[test]
fn verify_dry_run_contract() {
    assert_dry_run_contract(&["verify", "--dry-run"], "DRY-RUN verify");
}

#[test]
fn gateway_add_dry_run_contract() {
    assert_dry_run_contract(
        &["gateway", "add", "cli", "--dry-run"],
        "DRY-RUN gateway add source=cli",
    );
}

#[test]
fn inbox_list_dry_run_contract() {
    assert_dry_run_contract(
        &["inbox", "list", "--dry-run"],
        "inbox list placeholder (empty)",
    );
}

#[test]
fn verify_non_dry_run_is_fail_safe() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("odin-cli");
    cmd.args(["verify"]).timeout(Duration::from_secs(3));

    cmd.assert()
        .failure()
        .stderr(contains("native non-dry-run verify is not implemented"));
}

#[test]
fn unknown_args_fallback_to_legacy_parser() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("odin-cli");
    cmd.args(["--run-once", "--legacy-unknown-flag"])
        .timeout(Duration::from_secs(3));

    cmd.assert()
        .success()
        .stdout(contains("bootstrap outcome:"));
}
