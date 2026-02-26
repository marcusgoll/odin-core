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
fn skill_mermaid_emits_state_diagram_from_sass_skill_xml() {
    let path = fixture_path("run_tests.skill.xml");
    let path_str = path.to_str().expect("fixture path must be utf-8");

    let result = run_cli(&["skill", "mermaid", path_str]);
    assert!(
        result.status.success(),
        "expected success for mermaid generation\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    let expected = [
        "stateDiagram-v2",
        "    %% wake_up: collect_context",
        "    [*] --> collect_context",
        "    collect_context --> run_test_suite",
        "    run_test_suite --> report_success",
        "    run_test_suite --> report_failure",
        "    report_success --> done",
        "    report_failure --> done",
    ]
    .join("\n");
    assert_eq!(
        result.stdout.trim_end_matches('\n'),
        expected,
        "unexpected mermaid output\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
}

#[test]
fn skill_mermaid_fails_for_semantically_invalid_skill_xml() {
    let path = fixture_path("broken-missing-target.skill.xml");
    let path_str = path.to_str().expect("fixture path must be utf-8");

    let result = run_cli(&["skill", "mermaid", path_str]);
    assert!(
        !result.status.success(),
        "expected mermaid generation to fail for invalid skill\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(
        result.stderr.contains("validation failed"),
        "expected validation failure prefix in stderr\nstderr:\n{}",
        result.stderr
    );
    assert!(
        result.stderr.contains("transitions to unknown target"),
        "expected unknown transition target validation error in stderr\nstderr:\n{}",
        result.stderr
    );
}
