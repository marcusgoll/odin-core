//! Plugin installation and loading contracts.

use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use odin_plugin_protocol::PluginManifest;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginSource {
    LocalPath(PathBuf),
    GitRef(String),
    Artifact(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallRequest {
    pub source: PluginSource,
    pub expected_checksum_sha256: Option<String>,
    pub require_signature: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallResult {
    pub manifest: PluginManifest,
    pub install_path: PathBuf,
}

#[derive(Debug, Error)]
pub enum PluginManagerError {
    #[error("unsupported source: {0}")]
    UnsupportedSource(String),
    #[error("manifest missing: {0}")]
    ManifestMissing(String),
    #[error("manifest parse failed: {0}")]
    ManifestParse(String),
    #[error("checksum mismatch")]
    ChecksumMismatch,
    #[error("signature required but not present")]
    SignatureMissing,
    #[error("unsupported signature method: {0}")]
    SignatureMethodUnsupported(String),
    #[error("signature verification failed: {0}")]
    SignatureVerificationFailed(String),
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
    #[error("command failed: {0}")]
    CommandFailed(String),
    #[error("io error: {0}")]
    Io(String),
}

pub trait PluginManager: Send + Sync {
    fn install(&self, req: &InstallRequest) -> Result<InstallResult, PluginManagerError>;
    fn load_manifest(&self, path: &Path) -> Result<PluginManifest, PluginManagerError>;
}

#[derive(Clone, Debug)]
pub struct FilesystemPluginManager {
    installs_root: PathBuf,
}

impl Default for FilesystemPluginManager {
    fn default() -> Self {
        Self {
            installs_root: std::env::temp_dir().join("odin-core-plugin-installs"),
        }
    }
}

impl FilesystemPluginManager {
    pub fn new(installs_root: impl Into<PathBuf>) -> Self {
        Self {
            installs_root: installs_root.into(),
        }
    }

    fn prepare_install_dir(&self, prefix: &str) -> Result<PathBuf, PluginManagerError> {
        fs::create_dir_all(&self.installs_root)
            .map_err(|e| PluginManagerError::Io(e.to_string()))?;

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = self
            .installs_root
            .join(format!("{}-{}-{}", prefix, std::process::id(), ts));
        fs::create_dir_all(&dir).map_err(|e| PluginManagerError::Io(e.to_string()))?;
        Ok(dir)
    }

    fn install_from_local_path(
        &self,
        path: &Path,
        req: &InstallRequest,
    ) -> Result<InstallResult, PluginManagerError> {
        let manifest_dir = self.find_manifest_dir(path)?;
        let manifest_path = manifest_dir.join("odin.plugin.yaml");
        let manifest = self.load_manifest(&manifest_dir)?;

        if manifest.schema_version != 1 {
            return Err(PluginManagerError::InvalidManifest(
                "only schema_version=1 is supported".to_string(),
            ));
        }

        if let Some(expected) = &req.expected_checksum_sha256 {
            let actual = &manifest.distribution.integrity.checksum_sha256;
            if !expected.eq_ignore_ascii_case(actual) {
                return Err(PluginManagerError::ChecksumMismatch);
            }
        }

        self.verify_signature(
            &manifest_dir,
            &manifest_path,
            &manifest,
            req.require_signature,
        )?;

        Ok(InstallResult {
            manifest,
            install_path: manifest_dir,
        })
    }

    fn install_from_git_ref(
        &self,
        spec: &str,
        req: &InstallRequest,
    ) -> Result<InstallResult, PluginManagerError> {
        let (repo, git_ref) = parse_git_ref(spec);
        let checkout_dir = self.prepare_install_dir("git-plugin")?;

        run_command(
            Command::new("git")
                .arg("clone")
                .arg(&repo)
                .arg(&checkout_dir),
            "git clone",
        )?;

        if git_ref != "HEAD" {
            run_command(
                Command::new("git")
                    .arg("-C")
                    .arg(&checkout_dir)
                    .arg("checkout")
                    .arg(git_ref),
                "git checkout",
            )?;
        }

        self.install_from_local_path(&checkout_dir, req)
    }

    fn install_from_artifact(
        &self,
        spec: &str,
        req: &InstallRequest,
    ) -> Result<InstallResult, PluginManagerError> {
        if spec.starts_with("http://") || spec.starts_with("https://") {
            let download_dir = self.prepare_install_dir("artifact-download")?;
            let archive = download_dir.join("plugin.tar.gz");

            run_command(
                Command::new("curl")
                    .arg("-fsSL")
                    .arg(spec)
                    .arg("-o")
                    .arg(&archive),
                "artifact download",
            )?;

            return self.install_from_artifact(&archive.display().to_string(), req);
        }

        let path = PathBuf::from(spec);

        if path.is_dir() {
            return self.install_from_local_path(&path, req);
        }

        if !path.exists() {
            return Err(PluginManagerError::UnsupportedSource(format!(
                "artifact path not found: {}",
                path.display()
            )));
        }

        if path.file_name().and_then(|n| n.to_str()) == Some("odin.plugin.yaml") {
            let base = path.parent().unwrap_or_else(|| Path::new("."));
            return self.install_from_local_path(base, req);
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_lowercase();

        if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            let archive_checksum = sha256_file(&path)?;
            if let Some(expected) = &req.expected_checksum_sha256 {
                if !expected.eq_ignore_ascii_case(&archive_checksum) {
                    return Err(PluginManagerError::ChecksumMismatch);
                }
            }

            let extract_dir = self.prepare_install_dir("artifact-plugin")?;
            run_command(
                Command::new("tar")
                    .arg("-xzf")
                    .arg(&path)
                    .arg("-C")
                    .arg(&extract_dir),
                "artifact extract",
            )?;

            let mut local_req = req.clone();
            local_req.expected_checksum_sha256 = None;
            return self.install_from_local_path(&extract_dir, &local_req);
        }

        Err(PluginManagerError::UnsupportedSource(format!(
            "unsupported artifact format: {}",
            path.display()
        )))
    }

    fn find_manifest_dir(&self, path: &Path) -> Result<PathBuf, PluginManagerError> {
        if path.join("odin.plugin.yaml").exists() {
            return Ok(path.to_path_buf());
        }

        if !path.is_dir() {
            return Err(PluginManagerError::ManifestMissing(
                path.join("odin.plugin.yaml").display().to_string(),
            ));
        }

        for entry in fs::read_dir(path).map_err(|e| PluginManagerError::Io(e.to_string()))? {
            let entry = entry.map_err(|e| PluginManagerError::Io(e.to_string()))?;
            let candidate = entry.path();
            if candidate.is_dir() && candidate.join("odin.plugin.yaml").exists() {
                return Ok(candidate);
            }
        }

        Err(PluginManagerError::ManifestMissing(
            path.join("odin.plugin.yaml").display().to_string(),
        ))
    }

    fn verify_signature(
        &self,
        manifest_dir: &Path,
        manifest_path: &Path,
        manifest: &PluginManifest,
        require_signature: bool,
    ) -> Result<(), PluginManagerError> {
        let manifest_requires = manifest
            .signing
            .as_ref()
            .and_then(|s| s.required)
            .unwrap_or(false);
        if !(require_signature || manifest_requires) {
            return Ok(());
        }

        let signing = manifest
            .signing
            .as_ref()
            .ok_or(PluginManagerError::SignatureMissing)?;

        let method = signing
            .method
            .as_deref()
            .unwrap_or("none")
            .trim()
            .to_lowercase();

        let signature = signing
            .signature
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or(PluginManagerError::SignatureMissing)?;

        let signature_path = resolve_path(manifest_dir, signature);
        if !signature_path.exists() {
            return Err(PluginManagerError::SignatureMissing);
        }

        match method.as_str() {
            "none" => Err(PluginManagerError::SignatureMissing),
            "minisign" => {
                let cert_value = signing
                    .certificate
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or(PluginManagerError::SignatureMissing)?;

                let public_key = materialize_public_key(manifest_dir, cert_value)?;
                run_command(
                    Command::new("minisign")
                        .arg("-Vm")
                        .arg(manifest_path)
                        .arg("-x")
                        .arg(&signature_path)
                        .arg("-P")
                        .arg(public_key),
                    "minisign verify",
                )
                .map_err(|e| PluginManagerError::SignatureVerificationFailed(e.to_string()))
            }
            "sigstore" => {
                let cert_path = signing
                    .certificate
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or(PluginManagerError::SignatureMissing)?;
                let cert_path = resolve_path(manifest_dir, cert_path);
                if !cert_path.exists() {
                    return Err(PluginManagerError::SignatureMissing);
                }

                run_command(
                    Command::new("cosign")
                        .arg("verify-blob")
                        .arg("--key")
                        .arg(&cert_path)
                        .arg("--signature")
                        .arg(&signature_path)
                        .arg(manifest_path),
                    "sigstore verify",
                )
                .map_err(|e| PluginManagerError::SignatureVerificationFailed(e.to_string()))
            }
            other => Err(PluginManagerError::SignatureMethodUnsupported(
                other.to_string(),
            )),
        }
    }
}

impl PluginManager for FilesystemPluginManager {
    fn install(&self, req: &InstallRequest) -> Result<InstallResult, PluginManagerError> {
        match &req.source {
            PluginSource::LocalPath(path) => self.install_from_local_path(path, req),
            PluginSource::GitRef(spec) => self.install_from_git_ref(spec, req),
            PluginSource::Artifact(spec) => self.install_from_artifact(spec, req),
        }
    }

    fn load_manifest(&self, path: &Path) -> Result<PluginManifest, PluginManagerError> {
        let manifest_dir = self.find_manifest_dir(path)?;
        let manifest_path = manifest_dir.join("odin.plugin.yaml");
        if !manifest_path.exists() {
            return Err(PluginManagerError::ManifestMissing(
                manifest_path.display().to_string(),
            ));
        }

        let raw = fs::read_to_string(&manifest_path)
            .map_err(|e| PluginManagerError::ManifestParse(e.to_string()))?;

        serde_yml::from_str::<PluginManifest>(&raw)
            .map_err(|e| PluginManagerError::ManifestParse(e.to_string()))
    }
}

fn resolve_path(base: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

fn materialize_public_key(manifest_dir: &Path, value: &str) -> Result<String, PluginManagerError> {
    let candidate = resolve_path(manifest_dir, value);
    if candidate.exists() {
        let text =
            fs::read_to_string(candidate).map_err(|e| PluginManagerError::Io(e.to_string()))?;
        if let Some(key_line) = text
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with('R') && line.len() >= 8)
        {
            return Ok(key_line.to_string());
        }
        return Ok(text.trim().to_string());
    }
    Ok(value.to_string())
}

fn parse_git_ref(spec: &str) -> (String, String) {
    if let Some((repo, r)) = spec.rsplit_once('#') {
        let repo = repo.trim();
        let r = r.trim();
        if !repo.is_empty() && !r.is_empty() {
            return (repo.to_string(), r.to_string());
        }
    }
    (spec.to_string(), "HEAD".to_string())
}

fn run_command(command: &mut Command, label: &str) -> Result<(), PluginManagerError> {
    let output = command
        .output()
        .map_err(|e| PluginManagerError::CommandFailed(format!("{}: {}", label, e)))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr)
        .replace('\n', " ")
        .trim()
        .to_string();

    Err(PluginManagerError::CommandFailed(format!(
        "{} failed (exit={}): {}",
        label, output.status, stderr
    )))
}

fn sha256_file(path: &Path) -> Result<String, PluginManagerError> {
    let mut file = File::open(path).map_err(|e| PluginManagerError::Io(e.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 8192];

    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| PluginManagerError::Io(e.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    let digest = hasher.finalize();
    Ok(format!("{:x}", digest))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use super::{
        sha256_file, FilesystemPluginManager, InstallRequest, PluginManager, PluginSource,
    };

    fn write_manifest_with_signing(
        dir: &Path,
        checksum: &str,
        method: &str,
        signature: &str,
        certificate: &str,
        required: bool,
    ) {
        let content = format!(
            "schema_version: 1\nplugin:\n  name: example.safe-github\n  version: 0.1.0\n  runtime: external-process\n  compatibility:\n    core_version: \">=0.1.0 <0.2.0\"\n  entrypoint:\n    command: ./bin/plugin\n  capabilities:\n    - id: repo.read\n      scope: [project]\ndistribution:\n  source:\n    type: local-path\n    ref: .\n  integrity:\n    checksum_sha256: \"{}\"\nsigning:\n  required: {}\n  method: {}\n  signature: {}\n  certificate: {}\n",
            checksum,
            if required { "true" } else { "false" },
            serde_yml::to_string(method).expect("method yaml").trim(),
            serde_yml::to_string(signature).expect("signature yaml").trim(),
            serde_yml::to_string(certificate)
                .expect("certificate yaml")
                .trim(),
        );
        fs::write(dir.join("odin.plugin.yaml"), content).expect("write manifest");
    }

    fn write_manifest(dir: &Path, checksum: &str) {
        write_manifest_with_signing(dir, checksum, "none", "", "", false);
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "odin-core-plugin-test-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    fn command_available(name: &str) -> bool {
        Command::new("sh")
            .arg("-lc")
            .arg(format!("command -v {} >/dev/null 2>&1", name))
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn run_command_checked(cmd: &mut Command, label: &str) {
        let output = cmd
            .output()
            .unwrap_or_else(|e| panic!("{label} spawn failed: {e}"));
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("{label} failed ({}): {}", output.status, stderr);
        }
    }

    fn read_minisign_public_key(path: &Path) -> String {
        let text = fs::read_to_string(path).expect("read minisign pub");
        text.lines()
            .map(str::trim)
            .find(|line| line.starts_with('R') && line.len() >= 8)
            .expect("minisign key line")
            .to_string()
    }

    #[test]
    fn local_install_parses_manifest() {
        let root = temp_dir("local-ok");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("mkdir");
        write_manifest(
            &root,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        let manager = FilesystemPluginManager::default();
        let result = manager.install(&InstallRequest {
            source: PluginSource::LocalPath(root.clone()),
            expected_checksum_sha256: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
            require_signature: false,
        });

        assert!(result.is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn local_install_rejects_mismatched_checksum() {
        let root = temp_dir("local-bad-checksum");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("mkdir");
        write_manifest(
            &root,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        );

        let manager = FilesystemPluginManager::default();
        let result = manager.install(&InstallRequest {
            source: PluginSource::LocalPath(root.clone()),
            expected_checksum_sha256: Some(
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            ),
            require_signature: false,
        });

        assert!(result.is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn local_install_rejects_when_signature_required_but_missing() {
        let root = temp_dir("local-signature-required");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("mkdir");
        write_manifest(
            &root,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        let manager = FilesystemPluginManager::default();
        let result = manager.install(&InstallRequest {
            source: PluginSource::LocalPath(root.clone()),
            expected_checksum_sha256: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
            require_signature: true,
        });

        assert!(result.is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn local_install_rejects_unsupported_signature_method() {
        let root = temp_dir("local-unsupported-signature-method");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("mkdir");
        fs::write(root.join("sig.bin"), b"sig").expect("write sig");

        write_manifest_with_signing(
            &root,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "unknown",
            "sig.bin",
            "cert.pem",
            true,
        );

        let manager = FilesystemPluginManager::default();
        let result = manager.install(&InstallRequest {
            source: PluginSource::LocalPath(root.clone()),
            expected_checksum_sha256: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
            require_signature: false,
        });

        assert!(result.is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[ignore] // requires minisign CLI tool
    fn local_install_accepts_valid_minisign_signature_when_required() {
        let root = temp_dir("local-minisign-ok");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("mkdir");

        let secret = root.join("minisign.key");
        let public = root.join("minisign.pub");
        run_command_checked(
            Command::new("minisign")
                .arg("-G")
                .arg("-W")
                .arg("-s")
                .arg(&secret)
                .arg("-p")
                .arg(&public),
            "minisign keygen",
        );

        let public_key = read_minisign_public_key(&public);
        write_manifest_with_signing(
            &root,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "minisign",
            "odin.plugin.minisig",
            &public_key,
            true,
        );

        run_command_checked(
            Command::new("minisign")
                .arg("-Sm")
                .arg(root.join("odin.plugin.yaml"))
                .arg("-s")
                .arg(&secret)
                .arg("-x")
                .arg(root.join("odin.plugin.minisig")),
            "minisign sign",
        );

        let manager = FilesystemPluginManager::default();
        let result = manager.install(&InstallRequest {
            source: PluginSource::LocalPath(root.clone()),
            expected_checksum_sha256: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
            require_signature: true,
        });

        assert!(result.is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[ignore] // requires cosign CLI tool
    fn local_install_accepts_valid_sigstore_signature_when_required() {
        let root = temp_dir("local-sigstore-ok");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("mkdir");

        let prefix = root.join("cosign-test");
        run_command_checked(
            Command::new("cosign")
                .env("COSIGN_PASSWORD", "")
                .arg("generate-key-pair")
                .arg("--output-key-prefix")
                .arg(&prefix),
            "cosign keygen",
        );

        write_manifest_with_signing(
            &root,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "sigstore",
            "odin.plugin.sig",
            "cosign-test.pub",
            true,
        );

        run_command_checked(
            Command::new("cosign")
                .env("COSIGN_PASSWORD", "")
                .arg("sign-blob")
                .arg("--key")
                .arg(root.join("cosign-test.key"))
                .arg("--output-signature")
                .arg(root.join("odin.plugin.sig"))
                .arg(root.join("odin.plugin.yaml")),
            "cosign sign-blob",
        );

        let manager = FilesystemPluginManager::default();
        let result = manager.install(&InstallRequest {
            source: PluginSource::LocalPath(root.clone()),
            expected_checksum_sha256: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
            require_signature: true,
        });

        assert!(result.is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn git_ref_install_from_local_repo() {
        let repo_root = temp_dir("git-repo");
        let _ = fs::remove_dir_all(&repo_root);
        fs::create_dir_all(&repo_root).expect("mkdir");
        write_manifest(
            &repo_root,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        Command::new("git")
            .arg("init")
            .arg(&repo_root)
            .output()
            .expect("git init");
        Command::new("git")
            .arg("-C")
            .arg(&repo_root)
            .arg("config")
            .arg("user.email")
            .arg("test@example.com")
            .output()
            .expect("git config email");
        Command::new("git")
            .arg("-C")
            .arg(&repo_root)
            .arg("config")
            .arg("user.name")
            .arg("Test")
            .output()
            .expect("git config name");
        Command::new("git")
            .arg("-C")
            .arg(&repo_root)
            .arg("add")
            .arg(".")
            .output()
            .expect("git add");
        Command::new("git")
            .arg("-C")
            .arg(&repo_root)
            .arg("commit")
            .arg("-m")
            .arg("init")
            .output()
            .expect("git commit");

        let manager = FilesystemPluginManager::default();
        let source = format!("{}#HEAD", repo_root.display());
        let result = manager.install(&InstallRequest {
            source: PluginSource::GitRef(source),
            expected_checksum_sha256: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
            require_signature: false,
        });

        assert!(result.is_ok());
        let _ = fs::remove_dir_all(repo_root);
    }

    #[test]
    fn artifact_install_from_targz() {
        let plugin_dir = temp_dir("artifact-plugin");
        let archive_dir = temp_dir("artifact-archive");
        let _ = fs::remove_dir_all(&plugin_dir);
        let _ = fs::remove_dir_all(&archive_dir);
        fs::create_dir_all(&plugin_dir).expect("mkdir plugin");
        fs::create_dir_all(&archive_dir).expect("mkdir archive");

        write_manifest(
            &plugin_dir,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        let archive = archive_dir.join("plugin.tar.gz");
        Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&plugin_dir)
            .arg(".")
            .output()
            .expect("tar create");

        let archive_checksum = sha256_file(&archive).expect("archive checksum");

        let manager = FilesystemPluginManager::default();
        let result = manager.install(&InstallRequest {
            source: PluginSource::Artifact(archive.display().to_string()),
            expected_checksum_sha256: Some(archive_checksum),
            require_signature: false,
        });

        assert!(result.is_ok());
        let _ = fs::remove_dir_all(plugin_dir);
        let _ = fs::remove_dir_all(archive_dir);
    }

    #[test]
    fn artifact_install_from_targz_with_nested_root() {
        let archive_root = temp_dir("artifact-nested-root");
        let nested_plugin_dir = archive_root.join("plugin");
        let archive_dir = temp_dir("artifact-nested-archive");
        let _ = fs::remove_dir_all(&archive_root);
        let _ = fs::remove_dir_all(&archive_dir);
        fs::create_dir_all(&nested_plugin_dir).expect("mkdir nested plugin");
        fs::create_dir_all(&archive_dir).expect("mkdir archive");

        let archive = archive_dir.join("plugin-nested.tar.gz");
        write_manifest(
            &nested_plugin_dir,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );
        Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&archive_root)
            .arg(".")
            .output()
            .expect("tar create nested");

        let checksum = sha256_file(&archive).expect("checksum nested");

        let manager = FilesystemPluginManager::default();
        let result = manager.install(&InstallRequest {
            source: PluginSource::Artifact(archive.display().to_string()),
            expected_checksum_sha256: Some(checksum),
            require_signature: false,
        });

        assert!(result.is_ok());
        let _ = fs::remove_dir_all(archive_root);
        let _ = fs::remove_dir_all(archive_dir);
    }
}
