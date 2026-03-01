use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use anyhow::Context;
use sha2::{Digest, Sha256};

use crate::export::SECTION_MAPPINGS;

const MANIFEST_FILENAME: &str = "manifest.json";
const CHECKSUMS_FILENAME: &str = "checksums.sha256";

pub fn verify_bundle(bundle_dir: &Path) -> anyhow::Result<()> {
    ensure_bundle_root(bundle_dir)?;
    ensure_required_structure(bundle_dir)?;
    verify_checksums(bundle_dir)?;
    Ok(())
}

fn ensure_bundle_root(bundle_dir: &Path) -> anyhow::Result<()> {
    if !bundle_dir.exists() {
        anyhow::bail!(
            "bundle directory does not exist: {}. Re-run migrate export to create it.",
            bundle_dir.display()
        );
    }

    if !bundle_dir.is_dir() {
        anyhow::bail!(
            "bundle path is not a directory: {}. Pass --bundle <bundle-dir>.",
            bundle_dir.display()
        );
    }

    Ok(())
}

fn ensure_required_structure(bundle_dir: &Path) -> anyhow::Result<()> {
    let manifest_path = bundle_dir.join(MANIFEST_FILENAME);
    if !manifest_path.is_file() {
        anyhow::bail!(
            "missing required bundle file: {} (expected at {}). Re-run migrate export.",
            MANIFEST_FILENAME,
            manifest_path.display()
        );
    }

    let checksums_path = bundle_dir.join(CHECKSUMS_FILENAME);
    if !checksums_path.is_file() {
        anyhow::bail!(
            "missing required bundle file: {} (expected at {}). Re-run migrate export.",
            CHECKSUMS_FILENAME,
            checksums_path.display()
        );
    }

    for mapping in SECTION_MAPPINGS {
        let path = bundle_dir.join(mapping.name);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                anyhow::bail!(
                    "missing required bundle directory: {} (expected at {}). Re-run migrate export.",
                    mapping.name,
                    path.display()
                );
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "failed to inspect required bundle directory metadata at {}",
                        path.display()
                    )
                });
            }
        };

        if metadata.file_type().is_symlink() {
            anyhow::bail!(
                "required bundle directory must not be a symlink: {}",
                path.display()
            );
        }

        if !metadata.is_dir() {
            anyhow::bail!(
                "missing required bundle directory: {} (expected at {}). Re-run migrate export.",
                mapping.name,
                path.display()
            );
        }
    }

    Ok(())
}

fn verify_checksums(bundle_dir: &Path) -> anyhow::Result<()> {
    let checksum_entries = read_checksum_entries(bundle_dir)?;
    if !checksum_entries.contains_key(MANIFEST_FILENAME) {
        anyhow::bail!(
            "checksums.sha256 is missing required manifest entry: {}",
            MANIFEST_FILENAME
        );
    }

    let payload_files = collect_payload_files(bundle_dir)?;
    let expected_paths: BTreeSet<String> = checksum_entries.keys().cloned().collect();

    let missing_from_checksums: Vec<String> =
        payload_files.difference(&expected_paths).cloned().collect();
    if !missing_from_checksums.is_empty() {
        anyhow::bail!(
            "checksums.sha256 is missing entries for bundle file(s): {}",
            missing_from_checksums.join(", ")
        );
    }

    let unexpected_in_checksums: Vec<String> =
        expected_paths.difference(&payload_files).cloned().collect();
    if !unexpected_in_checksums.is_empty() {
        anyhow::bail!(
            "checksums.sha256 contains path(s) that are not manifest/copied files: {}",
            unexpected_in_checksums.join(", ")
        );
    }

    for path in payload_files {
        let expected = checksum_entries
            .get(&path)
            .expect("checksum map should include path after set comparison");
        let absolute = bundle_dir.join(&path);
        let bytes = fs::read(&absolute).with_context(|| {
            format!(
                "failed to read bundle file for checksum: {}",
                absolute.display()
            )
        })?;
        let actual = checksum_hex(&bytes);

        if actual != *expected {
            anyhow::bail!(
                "checksum mismatch for bundle file {path}: expected {expected}, got {actual}. Bundle may be tampered; re-run migrate export."
            );
        }
    }

    Ok(())
}

fn read_checksum_entries(bundle_dir: &Path) -> anyhow::Result<BTreeMap<String, String>> {
    let checksums_path = bundle_dir.join(CHECKSUMS_FILENAME);
    let raw = fs::read_to_string(&checksums_path)
        .with_context(|| format!("failed to read checksums file {}", checksums_path.display()))?;

    let mut entries = BTreeMap::new();

    for (line_idx, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let line_no = line_idx + 1;
        let (digest, raw_path) = line.split_once("  ").ok_or_else(|| {
            anyhow::anyhow!(
                "invalid checksums.sha256 line {line_no}: expected '<sha256><space><space><relative-path>'"
            )
        })?;

        if !looks_like_sha256_hex(digest) {
            anyhow::bail!("invalid checksum digest on line {line_no}: expected 64 hex chars");
        }

        let normalized_path = normalize_checksum_path(raw_path, line_no)?;

        if entries
            .insert(normalized_path.clone(), digest.to_ascii_lowercase())
            .is_some()
        {
            anyhow::bail!(
                "duplicate path in checksums.sha256 on line {line_no}: {normalized_path}"
            );
        }
    }

    if entries.is_empty() {
        anyhow::bail!("checksums.sha256 is empty");
    }

    Ok(entries)
}

fn looks_like_sha256_hex(candidate: &str) -> bool {
    candidate.len() == 64 && candidate.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn normalize_checksum_path(raw_path: &str, line_no: usize) -> anyhow::Result<String> {
    if raw_path.is_empty() {
        anyhow::bail!("invalid checksums.sha256 line {line_no}: missing path");
    }

    let path = Path::new(raw_path);
    if path.is_absolute() {
        anyhow::bail!(
            "invalid checksums.sha256 line {line_no}: path must be relative, got {raw_path}"
        );
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => normalized.push(segment),
            Component::CurDir => {
                anyhow::bail!(
                    "invalid checksums.sha256 line {line_no}: path cannot contain '.' segments"
                )
            }
            Component::ParentDir => {
                anyhow::bail!(
                    "invalid checksums.sha256 line {line_no}: path cannot contain '..' segments"
                )
            }
            Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!(
                    "invalid checksums.sha256 line {line_no}: path must be relative, got {raw_path}"
                )
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        anyhow::bail!("invalid checksums.sha256 line {line_no}: missing path");
    }

    Ok(normalize_relative_path(&normalized))
}

fn collect_payload_files(bundle_dir: &Path) -> anyhow::Result<BTreeSet<String>> {
    let mut files = BTreeSet::new();
    files.insert(MANIFEST_FILENAME.to_string());

    for mapping in SECTION_MAPPINGS {
        collect_payload_files_recursive(bundle_dir, &bundle_dir.join(mapping.name), &mut files)?;
    }

    Ok(files)
}

fn collect_payload_files_recursive(
    bundle_root: &Path,
    current: &Path,
    out: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    let mut entries = fs::read_dir(current)
        .with_context(|| format!("failed to read bundle directory {}", current.display()))?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("failed to read entries in {}", current.display()))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to determine entry type for {}", path.display()))?;

        if file_type.is_dir() {
            collect_payload_files_recursive(bundle_root, &path, out)?;
            continue;
        }

        if file_type.is_file() {
            let relative = path.strip_prefix(bundle_root).with_context(|| {
                format!(
                    "failed to compute relative bundle path for {}",
                    path.display()
                )
            })?;
            out.insert(normalize_relative_path(relative));
            continue;
        }

        anyhow::bail!(
            "unsupported bundle entry type at {}: only regular files and directories are allowed",
            path.display()
        );
    }

    Ok(())
}

fn normalize_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn checksum_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}
