use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn run_cli(args: &[&str]) -> Result<Output, String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_odin-cli"))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to spawn odin-cli: {err}"))?;

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("odin-cli timed out for args: {:?}", args));
        }

        match child.try_wait() {
            Ok(Some(_status)) => {
                return child
                    .wait_with_output()
                    .map_err(|err| format!("failed to collect output: {err}"));
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(err) => return Err(format!("failed while waiting for odin-cli: {err}")),
        }
    }
}

fn stdout_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn migrate_help_surface_is_available() {
    let output = run_cli(&["migrate", "--help"]).expect("odin-cli should return promptly");
    assert!(output.status.success(), "stdout:\n{}", stdout_text(&output));

    let stdout = stdout_text(&output);
    assert!(stdout.contains("Usage: odin-cli migrate <COMMAND>"));
    assert!(stdout.contains("export"));
    assert!(stdout.contains("validate"));
    assert!(stdout.contains("import"));
}

#[test]
fn migrate_export_help_surface_is_available() {
    let output =
        run_cli(&["migrate", "export", "--help"]).expect("odin-cli should return promptly");
    assert!(output.status.success(), "stdout:\n{}", stdout_text(&output));

    let stdout = stdout_text(&output);
    assert!(stdout.contains("Usage: odin-cli migrate export"));
}

#[test]
fn migrate_validate_help_surface_is_available() {
    let output =
        run_cli(&["migrate", "validate", "--help"]).expect("odin-cli should return promptly");
    assert!(output.status.success(), "stdout:\n{}", stdout_text(&output));

    let stdout = stdout_text(&output);
    assert!(stdout.contains("Usage: odin-cli migrate validate"));
}

#[test]
fn migrate_import_help_surface_is_available() {
    let output =
        run_cli(&["migrate", "import", "--help"]).expect("odin-cli should return promptly");
    assert!(output.status.success(), "stdout:\n{}", stdout_text(&output));

    let stdout = stdout_text(&output);
    assert!(stdout.contains("Usage: odin-cli migrate import"));
}

#[test]
fn migrate_export_without_required_flags_exits_non_zero() {
    let output = run_cli(&["migrate", "export"]).expect("odin-cli should return promptly");
    assert!(
        !output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout_text(&output),
        stderr_text(&output)
    );

    let stderr = stderr_text(&output);
    assert!(stderr.contains("missing required flag: --source-root"));
}

#[test]
fn migrate_validate_without_bundle_flag_exits_non_zero() {
    let output = run_cli(&["migrate", "validate"]).expect("odin-cli should return promptly");
    assert!(
        !output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout_text(&output),
        stderr_text(&output)
    );

    let stderr = stderr_text(&output);
    assert!(stderr.contains("missing required flag: --bundle"));
}

#[test]
fn migrate_import_without_extra_args_delegates_to_stub() {
    let output = run_cli(&["migrate", "import"]).expect("odin-cli should return promptly");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout_text(&output),
        stderr_text(&output)
    );

    let stdout = stdout_text(&output);
    assert!(stdout.contains("migrate import is not implemented yet"));
}

#[test]
fn migrate_unknown_subcommand_exits_non_zero_with_clear_error() {
    let output = run_cli(&["migrate", "unknown"]).expect("odin-cli should return promptly");
    assert!(
        !output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout_text(&output),
        stderr_text(&output)
    );

    let stderr = stderr_text(&output);
    assert!(stderr.contains("unknown migrate subcommand: unknown"));
}
