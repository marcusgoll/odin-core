use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::checksum;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RootSelector {
    SourceRoot,
    OdinDir,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SectionMapping {
    name: &'static str,
    source: RootSelector,
}

const SECTION_MAPPINGS: [SectionMapping; 8] = [
    SectionMapping {
        name: "skills",
        source: RootSelector::SourceRoot,
    },
    SectionMapping {
        name: "learnings",
        source: RootSelector::SourceRoot,
    },
    SectionMapping {
        name: "runtime",
        source: RootSelector::OdinDir,
    },
    SectionMapping {
        name: "checkpoints",
        source: RootSelector::OdinDir,
    },
    SectionMapping {
        name: "events",
        source: RootSelector::OdinDir,
    },
    SectionMapping {
        name: "opaque",
        source: RootSelector::SourceRoot,
    },
    SectionMapping {
        name: "quarantine",
        source: RootSelector::SourceRoot,
    },
    SectionMapping {
        name: "meta",
        source: RootSelector::OdinDir,
    },
];

pub fn write_bundle(source_root: &Path, odin_dir: &Path, out_dir: &Path) -> anyhow::Result<()> {
    validate_input_directory("source root", source_root)?;
    validate_input_directory("odin dir", odin_dir)?;
    reject_output_equal_input_roots(source_root, odin_dir, out_dir)?;
    reject_output_inside_mapped_source_sections(source_root, odin_dir, out_dir)?;
    prepare_clean_output_dir(out_dir)?;

    let mut written_files = Vec::new();

    for mapping in SECTION_MAPPINGS {
        let out_section_dir = out_dir.join(mapping.name);
        fs::create_dir_all(&out_section_dir).with_context(|| {
            format!(
                "failed to create export section directory {}",
                out_section_dir.display()
            )
        })?;

        let source_section_dir = match mapping.source {
            RootSelector::SourceRoot => source_root.join(mapping.name),
            RootSelector::OdinDir => odin_dir.join(mapping.name),
        };

        let mut section_files =
            copy_section_files(&source_section_dir, &out_section_dir, mapping.name)?;
        written_files.append(&mut section_files);
    }

    if written_files.is_empty() {
        anyhow::bail!("export produced no mapped files from source roots");
    }

    let manifest_path = out_dir.join("manifest.json");
    fs::write(&manifest_path, manifest_json()).with_context(|| {
        format!(
            "failed to write export manifest to {}",
            manifest_path.display()
        )
    })?;

    written_files.push(PathBuf::from("manifest.json"));

    let checksums_path = out_dir.join("checksums.sha256");
    checksum::write_checksums_file(out_dir, &written_files, &checksums_path)?;

    Ok(())
}

fn validate_input_directory(label: &str, path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        anyhow::bail!("export {label} does not exist: {}", path.display());
    }
    if !path.is_dir() {
        anyhow::bail!("export {label} is not a directory: {}", path.display());
    }
    Ok(())
}

fn reject_output_inside_mapped_source_sections(
    source_root: &Path,
    odin_dir: &Path,
    out_dir: &Path,
) -> anyhow::Result<()> {
    let out_abs = canonicalize_path_allow_missing(out_dir)?;

    for mapping in SECTION_MAPPINGS {
        let section_path = match mapping.source {
            RootSelector::SourceRoot => source_root.join(mapping.name),
            RootSelector::OdinDir => odin_dir.join(mapping.name),
        };
        let section_abs = canonicalize_path_allow_missing(&section_path)?;

        if out_abs.starts_with(&section_abs) {
            anyhow::bail!(
                "export output path cannot be inside mapped source section `{}`: {}",
                mapping.name,
                out_dir.display()
            );
        }
    }

    Ok(())
}

fn reject_output_equal_input_roots(
    source_root: &Path,
    odin_dir: &Path,
    out_dir: &Path,
) -> anyhow::Result<()> {
    let out_abs = canonicalize_path_allow_missing(out_dir)?;
    let source_abs = normalize_path(&fs::canonicalize(source_root).with_context(|| {
        format!(
            "failed to canonicalize source root {}",
            source_root.display()
        )
    })?);
    let odin_abs = normalize_path(
        &fs::canonicalize(odin_dir)
            .with_context(|| format!("failed to canonicalize odin dir {}", odin_dir.display()))?,
    );

    if out_abs == source_abs || out_abs == odin_abs {
        anyhow::bail!(
            "export output path cannot equal export input root: {}",
            out_dir.display()
        );
    }

    Ok(())
}

fn prepare_clean_output_dir(out_dir: &Path) -> anyhow::Result<()> {
    if out_dir.exists() {
        if !out_dir.is_dir() {
            anyhow::bail!(
                "export output path exists and is not a directory: {}",
                out_dir.display()
            );
        }
        fs::remove_dir_all(out_dir).with_context(|| {
            format!(
                "failed to remove existing export output directory {}",
                out_dir.display()
            )
        })?;
    }

    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create export bundle root {}", out_dir.display()))?;
    Ok(())
}

fn copy_section_files(
    source_section_dir: &Path,
    out_section_dir: &Path,
    section_name: &str,
) -> anyhow::Result<Vec<PathBuf>> {
    if !source_section_dir.exists() {
        return Ok(Vec::new());
    }
    if !source_section_dir.is_dir() {
        anyhow::bail!(
            "export source section is not a directory: {}",
            source_section_dir.display()
        );
    }

    let relative_files = collect_relative_files(source_section_dir)?;
    let mut written_files = Vec::with_capacity(relative_files.len());

    for relative_file in relative_files {
        let source_file = source_section_dir.join(&relative_file);
        let destination_file = out_section_dir.join(&relative_file);

        if let Some(parent) = destination_file.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create export output directory for {}",
                    destination_file.display()
                )
            })?;
        }

        fs::copy(&source_file, &destination_file).with_context(|| {
            format!(
                "failed to copy export file {} -> {}",
                source_file.display(),
                destination_file.display()
            )
        })?;

        written_files.push(PathBuf::from(section_name).join(relative_file));
    }

    Ok(written_files)
}

fn collect_relative_files(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_relative_files_recursive(root, root, &mut files)?;
    files.sort_unstable();
    Ok(files)
}

fn collect_relative_files_recursive(
    root: &Path,
    current: &Path,
    files: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    let mut entries = fs::read_dir(current)
        .with_context(|| format!("failed to read export directory {}", current.display()))?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("failed to read entries in {}", current.display()))?;

    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to determine entry type for {}", path.display()))?;

        if file_type.is_dir() {
            collect_relative_files_recursive(root, &path, files)?;
        } else if file_type.is_file() {
            let relative = path.strip_prefix(root).with_context(|| {
                format!("failed to compute relative path for {}", path.display())
            })?;
            files.push(relative.to_path_buf());
        }
    }

    Ok(())
}

fn manifest_json() -> &'static str {
    "{\n  \"schema_version\": 1,\n  \"user_data_model_version\": 1,\n  \"skills\": {},\n  \"learnings\": {},\n  \"runtime\": {},\n  \"checkpoints\": {},\n  \"events\": {},\n  \"opaque\": {},\n  \"quarantine\": {},\n  \"meta\": {}\n}\n"
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
            "failed to canonicalize path ancestor {}",
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
