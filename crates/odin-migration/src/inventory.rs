use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

const COUNTED_SECTIONS: [&str; 4] = ["skills", "learnings", "checkpoints", "events"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InventoryCounts {
    pub skills: usize,
    pub learnings: usize,
    pub checkpoints: usize,
    pub events: usize,
}

impl InventoryCounts {
    fn to_stable_json(self) -> String {
        format!(
            "{{\n  \"skills\": {},\n  \"learnings\": {},\n  \"checkpoints\": {},\n  \"events\": {}\n}}\n",
            self.skills, self.learnings, self.checkpoints, self.events
        )
    }
}

pub fn write_inventory_snapshot(
    input_dir: &Path,
    output_path: &Path,
) -> anyhow::Result<InventoryCounts> {
    validate_input_directory(input_dir)?;
    reject_output_inside_counted_sections(input_dir, output_path)?;

    let counts = InventoryCounts {
        skills: count_section_files(input_dir, "skills")?,
        learnings: count_section_files(input_dir, "learnings")?,
        checkpoints: count_section_files(input_dir, "checkpoints")?,
        events: count_section_files(input_dir, "events")?,
    };

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create inventory output parent directory {}",
                    parent.display()
                )
            })?;
        }
    }

    fs::write(output_path, counts.to_stable_json()).with_context(|| {
        format!(
            "failed to write inventory snapshot to {}",
            output_path.display()
        )
    })?;

    Ok(counts)
}

fn validate_input_directory(input_dir: &Path) -> anyhow::Result<()> {
    if !input_dir.exists() {
        anyhow::bail!(
            "inventory input directory does not exist: {}",
            input_dir.display()
        );
    }
    if !input_dir.is_dir() {
        anyhow::bail!(
            "inventory input path is not a directory: {}",
            input_dir.display()
        );
    }
    Ok(())
}

fn reject_output_inside_counted_sections(
    input_dir: &Path,
    output_path: &Path,
) -> anyhow::Result<()> {
    let input_root = fs::canonicalize(input_dir).with_context(|| {
        format!(
            "failed to canonicalize input directory {}",
            input_dir.display()
        )
    })?;
    let output_abs = canonicalize_path_allow_missing(output_path)?;

    for section in COUNTED_SECTIONS {
        if output_abs.starts_with(input_root.join(section)) {
            anyhow::bail!(
                "inventory output path cannot be inside counted section `{section}` under input root: {}",
                output_path.display()
            );
        }
    }

    Ok(())
}

fn canonicalize_path_allow_missing(path: &Path) -> anyhow::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to read current working directory")?
            .join(path)
    };

    let mut existing = absolute.as_path();
    let mut missing_tail: Vec<std::ffi::OsString> = Vec::new();

    while !existing.exists() {
        let component = existing.file_name().ok_or_else(|| {
            anyhow::anyhow!(
                "failed to resolve output path ancestor for {}",
                path.display()
            )
        })?;
        missing_tail.push(component.to_os_string());
        existing = existing.parent().ok_or_else(|| {
            anyhow::anyhow!(
                "failed to resolve output path ancestor for {}",
                path.display()
            )
        })?;
    }

    let mut resolved = fs::canonicalize(existing).with_context(|| {
        format!(
            "failed to canonicalize output path ancestor {}",
            existing.display()
        )
    })?;
    for component in missing_tail.iter().rev() {
        resolved.push(component);
    }

    Ok(normalize_path(&resolved))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn count_section_files(input_dir: &Path, section_name: &str) -> anyhow::Result<usize> {
    let section_path = input_dir.join(section_name);
    count_regular_files_recursive(&section_path)
}

fn count_regular_files_recursive(root: &Path) -> anyhow::Result<usize> {
    if !root.exists() {
        return Ok(0);
    }

    let mut count = 0;
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir)
            .with_context(|| format!("failed to read inventory directory {}", dir.display()))?;
        for entry in entries {
            let entry = entry.with_context(|| {
                format!(
                    "failed to read entry in inventory directory {}",
                    dir.display()
                )
            })?;
            let file_type = entry.file_type().with_context(|| {
                format!(
                    "failed to determine entry type for {}",
                    entry.path().display()
                )
            })?;
            if file_type.is_file() {
                count += 1;
            } else if file_type.is_dir() {
                stack.push(entry.path());
            }
        }
    }

    Ok(count)
}
