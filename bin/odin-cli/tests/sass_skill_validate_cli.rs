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
