use odin_migration::{run, MigrationCommand};
use serde_json::Value;
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
fn export_bundle_creates_required_bundle_root_structure() {
    let fixture = TempDir::new("odin-migration-export-structure");
    let source_root = fixture.path.join("source-root");
    let odin_dir = fixture.path.join("odin-dir");
    let out_dir = fixture.path.join("bundle");

    fs::create_dir_all(&source_root).expect("create source root");
    fs::create_dir_all(&odin_dir).expect("create odin dir");
    create_file(&source_root.join("skills/seed.json"), "seed");

    run(MigrationCommand::Export {
        source_root,
        odin_dir,
        out_dir: out_dir.clone(),
    })
    .expect("export should succeed");

    assert!(
        out_dir.join("manifest.json").is_file(),
        "manifest should exist"
    );
    assert!(
        out_dir.join("checksums.sha256").is_file(),
        "checksums should exist"
    );

    for required_dir in [
        "skills",
        "learnings",
        "runtime",
        "checkpoints",
        "events",
        "opaque",
        "quarantine",
        "meta",
    ] {
        let path = out_dir.join(required_dir);
        assert!(path.is_dir(), "required dir missing: {}", path.display());
    }
}

#[test]
fn export_bundle_emits_manifest_json() {
    let fixture = TempDir::new("odin-migration-export-manifest");
    let source_root = fixture.path.join("source-root");
    let odin_dir = fixture.path.join("odin-dir");
    let out_dir = fixture.path.join("bundle");

    create_file(&source_root.join("skills/skill-a.json"), "skill");
    create_file(&odin_dir.join("runtime/state.json"), "runtime");

    run(MigrationCommand::Export {
        source_root,
        odin_dir,
        out_dir: out_dir.clone(),
    })
    .expect("export should succeed");

    let manifest_raw = fs::read_to_string(out_dir.join("manifest.json")).expect("read manifest");
    let manifest: Value = serde_json::from_str(&manifest_raw).expect("manifest should be JSON");

    assert_eq!(manifest["schema_version"], 1);
    assert_eq!(manifest["user_data_model_version"], 1);

    for section in [
        "skills",
        "learnings",
        "runtime",
        "checkpoints",
        "events",
        "opaque",
        "quarantine",
        "meta",
    ] {
        assert!(
            manifest.get(section).is_some(),
            "manifest missing section key: {section}"
        );
    }
}

#[test]
fn export_bundle_writes_checksums_file() {
    let fixture = TempDir::new("odin-migration-export-checksums");
    let source_root = fixture.path.join("source-root");
    let odin_dir = fixture.path.join("odin-dir");
    let out_dir = fixture.path.join("bundle");

    create_file(&source_root.join("skills/skill-a.json"), "alpha");
    create_file(&odin_dir.join("runtime/state.json"), "beta");

    run(MigrationCommand::Export {
        source_root,
        odin_dir,
        out_dir: out_dir.clone(),
    })
    .expect("export should succeed");

    let checksums = fs::read_to_string(out_dir.join("checksums.sha256")).expect("read checksums");

    assert!(
        checksums.contains("  manifest.json"),
        "checksums should include manifest"
    );
    assert!(
        checksums.contains("  skills/skill-a.json"),
        "checksums should include copied source-root file"
    );
    assert!(
        checksums.contains("  runtime/state.json"),
        "checksums should include copied odin-dir file"
    );
}

#[test]
fn export_bundle_checksums_are_deterministically_ordered() {
    let fixture = TempDir::new("odin-migration-export-ordering");
    let source_root = fixture.path.join("source-root");
    let odin_dir = fixture.path.join("odin-dir");
    let out_a = fixture.path.join("bundle-a");
    let out_b = fixture.path.join("bundle-b");

    create_file(&source_root.join("skills/z-last.json"), "z");
    create_file(&source_root.join("skills/a-first.json"), "a");
    create_file(&source_root.join("learnings/m-mid.json"), "m");
    create_file(&odin_dir.join("runtime/r1.json"), "r1");
    create_file(&odin_dir.join("events/e1.json"), "e1");

    run(MigrationCommand::Export {
        source_root: source_root.clone(),
        odin_dir: odin_dir.clone(),
        out_dir: out_a.clone(),
    })
    .expect("first export should succeed");

    run(MigrationCommand::Export {
        source_root,
        odin_dir,
        out_dir: out_b.clone(),
    })
    .expect("second export should succeed");

    let checksums_a = fs::read_to_string(out_a.join("checksums.sha256")).expect("read checksums a");
    let checksums_b = fs::read_to_string(out_b.join("checksums.sha256")).expect("read checksums b");

    assert_eq!(
        checksums_a, checksums_b,
        "checksums output should be deterministic across runs"
    );

    let paths_in_order: Vec<&str> = checksums_a
        .lines()
        .filter_map(|line| line.split_once("  ").map(|(_, path)| path))
        .collect();

    let mut sorted = paths_in_order.clone();
    sorted.sort_unstable();

    assert_eq!(
        paths_in_order, sorted,
        "checksums entries must be in stable sorted path order"
    );
}

#[test]
fn export_bundle_rejects_output_inside_mapped_source_sections() {
    let fixture = TempDir::new("odin-migration-export-self-ingest");
    let source_root = fixture.path.join("source-root");
    let odin_dir = fixture.path.join("odin-dir");
    let out_dir = source_root.join("skills/bundle");

    create_file(&source_root.join("skills/seed.json"), "seed");
    fs::create_dir_all(&odin_dir).expect("create odin dir");

    let result = run(MigrationCommand::Export {
        source_root,
        odin_dir,
        out_dir: out_dir.clone(),
    });

    let err = result.expect_err("out dir inside mapped source section should fail");
    assert!(
        err.to_string()
            .contains("export output path cannot be inside mapped source section"),
        "unexpected error: {err:#}"
    );
    assert!(
        !out_dir.exists(),
        "failing export should not leave output directory behind"
    );
}

#[test]
fn export_bundle_removes_stale_files_when_output_directory_already_exists() {
    let fixture = TempDir::new("odin-migration-export-clean");
    let source_root = fixture.path.join("source-root");
    let odin_dir = fixture.path.join("odin-dir");
    let out_dir = fixture.path.join("bundle");

    create_file(&source_root.join("skills/skill-a.json"), "alpha");
    create_file(&odin_dir.join("runtime/state.json"), "runtime");

    run(MigrationCommand::Export {
        source_root: source_root.clone(),
        odin_dir: odin_dir.clone(),
        out_dir: out_dir.clone(),
    })
    .expect("first export should succeed");

    create_file(&out_dir.join("skills/stale.json"), "stale");
    assert!(out_dir.join("skills/stale.json").is_file());

    run(MigrationCommand::Export {
        source_root,
        odin_dir,
        out_dir: out_dir.clone(),
    })
    .expect("second export should succeed");

    assert!(
        !out_dir.join("skills/stale.json").exists(),
        "stale output files must be removed before writing a new bundle"
    );

    let checksums = fs::read_to_string(out_dir.join("checksums.sha256")).expect("read checksums");
    assert!(
        !checksums.contains("  skills/stale.json"),
        "stale file must not survive and be omitted from checksums"
    );
}

#[test]
fn export_bundle_checksums_file_uses_real_sha256_digest_format() {
    let fixture = TempDir::new("odin-migration-export-sha256");
    let source_root = fixture.path.join("source-root");
    let odin_dir = fixture.path.join("odin-dir");
    let out_dir = fixture.path.join("bundle");

    create_file(&source_root.join("skills/skill-a.txt"), "abc");
    fs::create_dir_all(&odin_dir).expect("create odin dir");

    run(MigrationCommand::Export {
        source_root,
        odin_dir,
        out_dir: out_dir.clone(),
    })
    .expect("export should succeed");

    let checksums = fs::read_to_string(out_dir.join("checksums.sha256")).expect("read checksums");
    let skill_line = checksums
        .lines()
        .find(|line| line.ends_with("  skills/skill-a.txt"))
        .expect("checksums should include skills/skill-a.txt");

    let (digest, _) = skill_line
        .split_once("  ")
        .expect("checksum lines should use 'digest<space><space>path' format");
    assert_eq!(
        digest, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "digest must match standard SHA-256 for input content"
    );
}

#[test]
fn export_bundle_rejects_output_equal_to_source_or_odin_root() {
    let fixture = TempDir::new("odin-migration-export-root-overlap");

    let source_root = fixture.path.join("source-root");
    let odin_dir = fixture.path.join("odin-dir");
    create_file(&source_root.join("skills/seed.json"), "seed");
    create_file(&odin_dir.join("runtime/state.json"), "state");

    let source_seed = source_root.join("skills/seed.json");
    let odin_seed = odin_dir.join("runtime/state.json");

    let result_source = run(MigrationCommand::Export {
        source_root: source_root.clone(),
        odin_dir: odin_dir.clone(),
        out_dir: source_root.clone(),
    });
    let err_source = result_source.expect_err("out == source_root should fail");
    assert!(
        err_source
            .to_string()
            .contains("export output path cannot equal export input root"),
        "unexpected error: {err_source:#}"
    );
    assert!(
        source_seed.is_file(),
        "source-root data must remain intact on rejected export"
    );

    let result_odin = run(MigrationCommand::Export {
        source_root,
        odin_dir: odin_dir.clone(),
        out_dir: odin_dir.clone(),
    });
    let err_odin = result_odin.expect_err("out == odin_dir should fail");
    assert!(
        err_odin
            .to_string()
            .contains("export output path cannot equal export input root"),
        "unexpected error: {err_odin:#}"
    );
    assert!(
        odin_seed.is_file(),
        "odin-dir data must remain intact on rejected export"
    );
}

#[test]
fn export_bundle_rejects_noop_when_no_mapped_files_are_copied() {
    let fixture = TempDir::new("odin-migration-export-noop");
    let source_root = fixture.path.join("source-root");
    let odin_dir = fixture.path.join("odin-dir");
    let out_dir = fixture.path.join("bundle");

    fs::create_dir_all(&source_root).expect("create source root");
    fs::create_dir_all(&odin_dir).expect("create odin dir");
    create_file(&source_root.join("unmapped/ignored.txt"), "ignore");
    create_file(&odin_dir.join("other/ignored.txt"), "ignore");

    let result = run(MigrationCommand::Export {
        source_root,
        odin_dir,
        out_dir: out_dir.clone(),
    });

    let err = result.expect_err("no-op export should fail");
    assert!(
        err.to_string()
            .contains("export produced no mapped files from source roots"),
        "unexpected error: {err:#}"
    );
    assert!(
        !out_dir.join("manifest.json").exists(),
        "manifest should not be written on no-op export rejection"
    );
    assert!(
        !out_dir.join("checksums.sha256").exists(),
        "checksums should not be written on no-op export rejection"
    );
}
