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
fn verify_dry_run_contract() {
    assert_dry_run_contract(&["verify", "--dry-run"], "DRY-RUN verify");
}
