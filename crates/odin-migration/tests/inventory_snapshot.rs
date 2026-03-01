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

fn create_file(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir for fixture file");
    }
    fs::write(path, "fixture").expect("write fixture file");
}

#[test]
fn inventory_snapshot_is_deterministic_and_counts_fixture_sections() {
    let fixture = TempDir::new("odin-migration-inventory");

    create_file(&fixture.path.join("skills/skill-a.json"));
    create_file(&fixture.path.join("skills/nested/skill-b.json"));
    create_file(&fixture.path.join("skills/nested/deeper/skill-c.json"));

    create_file(&fixture.path.join("learnings/learning-a.json"));
    create_file(&fixture.path.join("learnings/module/learning-b.json"));

    create_file(&fixture.path.join("checkpoints/checkpoint-a.json"));

    create_file(&fixture.path.join("events/event-a.json"));
    create_file(&fixture.path.join("events/event-b.json"));
    create_file(&fixture.path.join("events/archive/event-c.json"));
    create_file(&fixture.path.join("events/archive/deep/event-d.json"));

    create_file(&fixture.path.join("runtime/ignored.json"));

    let output_path = fixture.path.join("snapshots/inventory.json");
    run(MigrationCommand::Inventory {
        input_dir: fixture.path.clone(),
        output_path: output_path.clone(),
    })
    .expect("inventory command should succeed");

    let actual = fs::read_to_string(&output_path).expect("read inventory output");
    let expected = "{\n  \"skills\": 3,\n  \"learnings\": 2,\n  \"checkpoints\": 1,\n  \"events\": 4\n}\n";

    assert_eq!(actual, expected, "inventory snapshot must be stable");
}

#[test]
fn inventory_snapshot_rejects_missing_input_directory() {
    let fixture = TempDir::new("odin-migration-inventory");
    let missing_input = fixture.path.join("does-not-exist");
    let output_path = fixture.path.join("snapshots/inventory.json");

    let result = run(MigrationCommand::Inventory {
        input_dir: missing_input.clone(),
        output_path,
    });

    let err = result.expect_err("missing input directory should fail");
    assert!(
        err.to_string().contains("inventory input directory does not exist"),
        "unexpected error: {err:#}"
    );
}

#[test]
fn inventory_snapshot_rejects_input_path_that_is_not_a_directory() {
    let fixture = TempDir::new("odin-migration-inventory");
    let input_file = fixture.path.join("input-file.txt");
    create_file(&input_file);
    let output_path = fixture.path.join("snapshots/inventory.json");

    let result = run(MigrationCommand::Inventory {
        input_dir: input_file.clone(),
        output_path,
    });

    let err = result.expect_err("non-directory input path should fail");
    assert!(
        err.to_string()
            .contains("inventory input path is not a directory"),
        "unexpected error: {err:#}"
    );
}

#[test]
fn inventory_snapshot_rejects_output_inside_counted_section() {
    let fixture = TempDir::new("odin-migration-inventory");
    create_file(&fixture.path.join("skills/existing.json"));

    let output_path = fixture.path.join("skills/inventory.json");
    let result = run(MigrationCommand::Inventory {
        input_dir: fixture.path.clone(),
        output_path: output_path.clone(),
    });

    let err = result.expect_err("output path inside counted section should fail");
    assert!(
        err.to_string()
            .contains("inventory output path cannot be inside counted section"),
        "unexpected error: {err:#}"
    );
    assert!(
        !output_path.exists(),
        "inventory output should not be written on validation failure"
    );
}

#[cfg(unix)]
#[test]
fn inventory_snapshot_rejects_output_symlink_alias_into_counted_section() {
    use std::os::unix::fs::symlink;

    let fixture = TempDir::new("odin-migration-inventory");
    create_file(&fixture.path.join("skills/existing.json"));

    let alias_dir = fixture.path.join("skills-alias");
    symlink(fixture.path.join("skills"), &alias_dir).expect("create symlink alias to skills");

    let output_path = alias_dir.join("inventory.json");
    let result = run(MigrationCommand::Inventory {
        input_dir: fixture.path.clone(),
        output_path: output_path.clone(),
    });

    let err = result.expect_err("symlink alias output path should be rejected");
    assert!(
        err.to_string()
            .contains("inventory output path cannot be inside counted section"),
        "unexpected error: {err:#}"
    );
    assert!(
        !output_path.exists(),
        "inventory output should not be written through symlink alias"
    );
}
