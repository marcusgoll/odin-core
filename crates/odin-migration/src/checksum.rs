use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use sha2::{Digest, Sha256};

pub fn write_checksums_file(
    bundle_root: &Path,
    relative_paths: &[PathBuf],
    output_path: &Path,
) -> anyhow::Result<()> {
    let mut entries = Vec::with_capacity(relative_paths.len());

    for relative_path in relative_paths {
        let absolute_path = bundle_root.join(relative_path);
        let contents = fs::read(&absolute_path).with_context(|| {
            format!(
                "failed to read file for checksum: {}",
                absolute_path.display()
            )
        })?;
        let digest = checksum_hex(&contents);
        let normalized_path = normalize_relative_path(relative_path);
        entries.push((normalized_path, digest));
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut output = String::new();
    for (path, digest) in entries {
        output.push_str(&digest);
        output.push_str("  ");
        output.push_str(&path);
        output.push('\n');
    }

    fs::write(output_path, output)
        .with_context(|| format!("failed to write checksums file {}", output_path.display()))?;

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
