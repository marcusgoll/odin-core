use odin_migration::{run, MigrationCommand};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp fixture dir");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn create_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir for fixture file");
    }
    fs::write(path, contents).expect("write fixture file");
}

#[test]
fn validate_bundle_rejects_checksum_tamper() {
    let fixture = TempDir::new("odin-migration-validate-checksum-tamper");
    let source_root = fixture.path.join("source-root");
    let odin_dir = fixture.path.join("odin-dir");
    let bundle_dir = fixture.path.join("bundle");

    create_file(&source_root.join("skills/skill-a.json"), "original");
    create_file(&odin_dir.join("runtime/state.json"), "runtime");

    run(MigrationCommand::Export {
        source_root,
        odin_dir,
        out_dir: bundle_dir.clone(),
    })
    .expect("export should succeed");

    create_file(&bundle_dir.join("skills/skill-a.json"), "tampered");

    let result = run(MigrationCommand::Validate {
        bundle_dir: bundle_dir.clone(),
    });
    let err = result.expect_err("tampered bundle should fail validation");

    assert!(
        err.to_string()
            .contains("checksum mismatch for bundle file skills/skill-a.json"),
        "unexpected error: {err:#}"
    );
}

#[test]
fn validate_bundle_accepts_fresh_export_bundle() {
    let fixture = TempDir::new("odin-migration-validate-success");
    let source_root = fixture.path.join("source-root");
    let odin_dir = fixture.path.join("odin-dir");
    let bundle_dir = fixture.path.join("bundle");

    create_file(&source_root.join("skills/skill-a.json"), "original");
    create_file(&source_root.join("learnings/learn-a.json"), "learning");
    create_file(&odin_dir.join("runtime/state.json"), "runtime");

    run(MigrationCommand::Export {
        source_root,
        odin_dir,
        out_dir: bundle_dir.clone(),
    })
    .expect("export should succeed");

    run(MigrationCommand::Validate { bundle_dir }).expect("fresh export should validate");
}

#[test]
fn validate_bundle_rejects_missing_required_directory() {
    let fixture = TempDir::new("odin-migration-validate-structure");
    let source_root = fixture.path.join("source-root");
    let odin_dir = fixture.path.join("odin-dir");
    let bundle_dir = fixture.path.join("bundle");

    create_file(&source_root.join("skills/skill-a.json"), "original");
    create_file(&odin_dir.join("runtime/state.json"), "runtime");

    run(MigrationCommand::Export {
        source_root,
        odin_dir,
        out_dir: bundle_dir.clone(),
    })
    .expect("export should succeed");

    fs::remove_dir_all(bundle_dir.join("events")).expect("remove required section directory");

    let result = run(MigrationCommand::Validate {
        bundle_dir: bundle_dir.clone(),
    });
    let err = result.expect_err("missing required section should fail");

    assert!(
        err.to_string()
            .contains("missing required bundle directory: events"),
        "unexpected error: {err:#}"
    );
}

#[test]
fn validate_bundle_rejects_missing_manifest_checksum_entry() {
    let fixture = TempDir::new("odin-migration-validate-manifest-checksum");
    let source_root = fixture.path.join("source-root");
    let odin_dir = fixture.path.join("odin-dir");
    let bundle_dir = fixture.path.join("bundle");

    create_file(&source_root.join("skills/skill-a.json"), "original");
    create_file(&odin_dir.join("runtime/state.json"), "runtime");

    run(MigrationCommand::Export {
        source_root,
        odin_dir,
        out_dir: bundle_dir.clone(),
    })
    .expect("export should succeed");

    let checksums_path = bundle_dir.join("checksums.sha256");
    let filtered_checksums = fs::read_to_string(&checksums_path)
        .expect("read checksums")
        .lines()
        .filter(|line| !line.ends_with("  manifest.json"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&checksums_path, format!("{filtered_checksums}\n")).expect("rewrite checksums");

    let result = run(MigrationCommand::Validate {
        bundle_dir: bundle_dir.clone(),
    });
    let err = result.expect_err("missing manifest checksum should fail");

    assert!(
        err.to_string()
            .contains("checksums.sha256 is missing required manifest entry"),
        "unexpected error: {err:#}"
    );
}

#[cfg(unix)]
#[test]
fn validate_bundle_rejects_symlinked_required_directory() {
    let fixture = TempDir::new("odin-migration-validate-symlinked-required-dir");
    let source_root = fixture.path.join("source-root");
    let odin_dir = fixture.path.join("odin-dir");
    let bundle_dir = fixture.path.join("bundle");

    create_file(&source_root.join("skills/skill-a.json"), "original");
    create_file(&odin_dir.join("runtime/state.json"), "runtime");

    run(MigrationCommand::Export {
        source_root,
        odin_dir,
        out_dir: bundle_dir.clone(),
    })
    .expect("export should succeed");

    let events_dir = bundle_dir.join("events");
    fs::remove_dir_all(&events_dir).expect("remove required section directory");
    let outside_dir = fixture.path.join("outside-events");
    fs::create_dir_all(&outside_dir).expect("create external directory");
    std::os::unix::fs::symlink(&outside_dir, &events_dir)
        .expect("create symlinked required directory");

    let result = run(MigrationCommand::Validate {
        bundle_dir: bundle_dir.clone(),
    });
    let err = result.expect_err("symlinked required dir should fail validation");

    assert!(
        err.to_string()
            .contains("required bundle directory must not be a symlink"),
        "unexpected error: {err:#}"
    );
}
