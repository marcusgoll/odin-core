use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

struct CliOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

fn fixture_path(file_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/skills/sass/v0.1")
        .join(file_name)
}

fn run_cli(args: &[&str]) -> CliOutput {
    let mut child = Command::new(env!("CARGO_BIN_EXE_odin-cli"))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn odin-cli");

    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("failed to poll odin-cli") {
            let mut stdout = String::new();
            let mut stderr = String::new();

            if let Some(mut out) = child.stdout.take() {
                out.read_to_string(&mut stdout)
                    .expect("failed to read odin-cli stdout");
            }
            if let Some(mut err) = child.stderr.take() {
                err.read_to_string(&mut stderr)
                    .expect("failed to read odin-cli stderr");
            }

            return CliOutput {
                status,
                stdout,
                stderr,
            };
        }

        if start.elapsed() >= Duration::from_secs(5) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("odin-cli timed out while running args: {args:?}");
        }

        thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn skill_validate_handles_valid_and_invalid_sass_files() {
    let valid_path = fixture_path("run_tests.skill.xml");
    let valid_path_str = valid_path.to_str().expect("valid path must be utf-8");

    let valid = run_cli(&["skill", "validate", valid_path_str]);
    assert!(
        valid.status.success(),
        "expected success for valid skill file\nstdout:\n{}\nstderr:\n{}",
        valid.stdout,
        valid.stderr
    );
    assert!(
        valid.stdout.contains("validation ok"),
        "expected success message in stdout\nstdout:\n{}",
        valid.stdout
    );

    let invalid_path = fixture_path("broken-missing-wakeup.skill.xml");
    let invalid_path_str = invalid_path.to_str().expect("invalid path must be utf-8");

    let invalid = run_cli(&["skill", "validate", invalid_path_str]);
    assert!(
        !invalid.status.success(),
        "expected failure for missing wake_up file\nstdout:\n{}\nstderr:\n{}",
        invalid.stdout,
        invalid.stderr
    );
    assert!(
        invalid.stderr.contains("wake_up"),
        "expected stderr to mention wake_up\nstderr:\n{}",
        invalid.stderr
    );
}

#[test]
fn skill_validate_accepts_canonical_sass_examples() {
    for file_name in ["resolve_project.skill.xml", "interpret_results.skill.xml"] {
        let path = fixture_path(file_name);
        let path_str = path.to_str().expect("fixture path must be utf-8");

        let result = run_cli(&["skill", "validate", path_str]);
        assert!(
            result.status.success(),
            "expected success for canonical fixture {file_name}\nstdout:\n{}\nstderr:\n{}",
            result.stdout,
            result.stderr
        );
        assert!(
            result.stdout.contains("validation ok"),
            "expected validation ok output for canonical fixture {file_name}\nstdout:\n{}",
            result.stdout
        );
    }
}

#[test]
fn skill_validate_fails_when_non_end_states_missing_on_failure() {
    let invalid_path = fixture_path("broken-missing-on-failure.skill.xml");
    let invalid_path_str = invalid_path.to_str().expect("invalid path must be utf-8");

    let invalid = run_cli(&["skill", "validate", invalid_path_str]);
    assert!(
        !invalid.status.success(),
        "expected failure for missing on_failure file\nstdout:\n{}\nstderr:\n{}",
        invalid.stdout,
        invalid.stderr
    );
    assert!(
        invalid.stderr.contains("missing on_failure"),
        "expected stderr to mention missing on_failure\nstderr:\n{}",
        invalid.stderr
    );
}

#[test]
fn skill_validate_fails_when_decision_transitions_missing_guards() {
    let invalid_path = fixture_path("broken-decision-missing-guards.skill.xml");
    let invalid_path_str = invalid_path.to_str().expect("invalid path must be utf-8");

    let invalid = run_cli(&["skill", "validate", invalid_path_str]);
    assert!(
        !invalid.status.success(),
        "expected failure for decision state missing guards\nstdout:\n{}\nstderr:\n{}",
        invalid.stdout,
        invalid.stderr
    );
    assert!(
        invalid
            .stderr
            .contains("decision transitions without guards"),
        "expected stderr to mention decision guard error\nstderr:\n{}",
        invalid.stderr
    );
}

#[test]
fn skill_validate_fails_when_no_end_state_exists() {
    let invalid_path = fixture_path("broken-missing-end-state.skill.xml");
    let invalid_path_str = invalid_path.to_str().expect("invalid path must be utf-8");

    let invalid = run_cli(&["skill", "validate", invalid_path_str]);
    assert!(
        !invalid.status.success(),
        "expected failure for missing end state\nstdout:\n{}\nstderr:\n{}",
        invalid.stdout,
        invalid.stderr
    );
    assert!(
        invalid
            .stderr
            .contains("at least one end state is required"),
        "expected stderr to mention missing end-state rule\nstderr:\n{}",
        invalid.stderr
    );
}

#[test]
fn skill_validate_fails_when_transition_target_does_not_exist() {
    let invalid_path = fixture_path("broken-missing-target.skill.xml");
    let invalid_path_str = invalid_path.to_str().expect("invalid path must be utf-8");

    let invalid = run_cli(&["skill", "validate", invalid_path_str]);
    assert!(
        !invalid.status.success(),
        "expected failure for unknown transition target\nstdout:\n{}\nstderr:\n{}",
        invalid.stdout,
        invalid.stderr
    );
    assert!(
        invalid.stderr.contains("transitions to unknown target"),
        "expected stderr to mention unknown transition target\nstderr:\n{}",
        invalid.stderr
    );
}
