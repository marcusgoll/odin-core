//! Bash compatibility adapters for existing Odin scripts.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use odin_core_runtime::{
    BackendState, FailoverController, RuntimeError, RuntimeResult, TaskIngress,
};

#[derive(Clone, Debug)]
pub struct LegacyScriptPaths {
    pub odin_inbox_write: PathBuf,
    pub backend_state_lib: PathBuf,
    pub orchestrator_failover_lib: PathBuf,
}

impl LegacyScriptPaths {
    pub fn from_legacy_root(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        Self {
            odin_inbox_write: root.join("scripts/odin/odin-inbox-write.sh"),
            backend_state_lib: root.join("scripts/odin/lib/backend-state.sh"),
            orchestrator_failover_lib: root.join("scripts/odin/lib/orchestrator-failover.sh"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BashTaskIngressAdapter {
    script_path: PathBuf,
}

impl BashTaskIngressAdapter {
    pub fn new(script_path: impl Into<PathBuf>) -> Self {
        Self {
            script_path: script_path.into(),
        }
    }

    pub fn from_paths(paths: &LegacyScriptPaths) -> Self {
        Self::new(paths.odin_inbox_write.clone())
    }

    pub fn script_path(&self) -> &Path {
        &self.script_path
    }
}

impl TaskIngress for BashTaskIngressAdapter {
    fn write_task_payload(&self, payload: &str) -> RuntimeResult<()> {
        if payload.trim().is_empty() {
            return Err(RuntimeError::InvalidInput(
                "task payload cannot be empty".to_string(),
            ));
        }

        let mut child = Command::new("bash")
            .arg(&self.script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| RuntimeError::Execution(format!("failed to spawn adapter: {e}")))?;

        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(payload.as_bytes()).map_err(|e| {
                RuntimeError::Execution(format!("failed to write task payload: {e}"))
            })?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| RuntimeError::Execution(format!("adapter wait failed: {e}")))?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr)
            .replace('\n', " ")
            .trim()
            .to_string();
        Err(RuntimeError::Execution(format!(
            "legacy inbox writer failed (exit={}): {}",
            output.status, stderr
        )))
    }
}

#[derive(Clone, Debug)]
pub struct BashBackendStateAdapter {
    backend_state_lib: PathBuf,
    odin_dir: PathBuf,
}

impl BashBackendStateAdapter {
    pub fn new(backend_state_lib: impl Into<PathBuf>, odin_dir: impl Into<PathBuf>) -> Self {
        Self {
            backend_state_lib: backend_state_lib.into(),
            odin_dir: odin_dir.into(),
        }
    }

    pub fn from_paths(paths: &LegacyScriptPaths, odin_dir: impl Into<PathBuf>) -> Self {
        Self::new(paths.backend_state_lib.clone(), odin_dir)
    }

    pub fn backend_state_lib(&self) -> &Path {
        &self.backend_state_lib
    }

    fn state_file(&self) -> PathBuf {
        self.odin_dir.join("state.json")
    }

    fn routing_file(&self) -> PathBuf {
        self.odin_dir.join("routing.json")
    }

    fn run_backend_command(
        &self,
        script: &str,
        target: Option<&str>,
        reason: Option<&str>,
    ) -> RuntimeResult<String> {
        let mut cmd = Command::new("bash");
        cmd.arg("-lc")
            .arg(script)
            .env("BACKEND_STATE_LIB", &self.backend_state_lib)
            .env("ODIN_DIR", &self.odin_dir)
            .env("STATE_FILE", self.state_file())
            .env("ROUTING_FILE", self.routing_file())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(t) = target {
            cmd.env("TARGET", t);
        }
        if let Some(r) = reason {
            cmd.env("REASON", r);
        }

        let output = cmd
            .output()
            .map_err(|e| RuntimeError::Execution(format!("backend adapter failed: {e}")))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Ok(stdout);
        }

        let stderr = String::from_utf8_lossy(&output.stderr)
            .replace('\n', " ")
            .trim()
            .to_string();
        Err(RuntimeError::Execution(format!(
            "backend adapter command failed (exit={}): {}",
            output.status, stderr
        )))
    }
}

#[derive(Clone, Debug)]
pub struct BashFailoverAdapter {
    orchestrator_failover_lib: PathBuf,
    odin_dir: PathBuf,
}

impl BashFailoverAdapter {
    pub fn new(
        orchestrator_failover_lib: impl Into<PathBuf>,
        odin_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            orchestrator_failover_lib: orchestrator_failover_lib.into(),
            odin_dir: odin_dir.into(),
        }
    }

    pub fn from_paths(paths: &LegacyScriptPaths, odin_dir: impl Into<PathBuf>) -> Self {
        Self::new(paths.orchestrator_failover_lib.clone(), odin_dir)
    }

    pub fn failover_lib(&self) -> &Path {
        &self.orchestrator_failover_lib
    }

    fn state_file(&self) -> PathBuf {
        self.odin_dir.join("state.json")
    }

    fn routing_file(&self) -> PathBuf {
        self.odin_dir.join("routing.json")
    }
}

impl BackendState for BashBackendStateAdapter {
    fn get_active_backend(&self) -> RuntimeResult<String> {
        self.run_backend_command(
            "set -euo pipefail; source \"$BACKEND_STATE_LIB\"; get_orchestrator_backend",
            None,
            None,
        )
    }

    fn set_active_backend(&self, target: &str, reason: &str) -> RuntimeResult<()> {
        if target.trim().is_empty() {
            return Err(RuntimeError::InvalidInput(
                "target backend cannot be empty".to_string(),
            ));
        }

        self.run_backend_command(
            "set -euo pipefail; source \"$BACKEND_STATE_LIB\"; set_orchestrator_backend \"$TARGET\" \"$REASON\"",
            Some(target),
            Some(reason),
        )
        .map(|_| ())
    }
}

impl FailoverController for BashFailoverAdapter {
    fn attempt_failover(&self, active_backend: Option<&str>) -> RuntimeResult<()> {
        let mut cmd = Command::new("bash");
        cmd.arg("-lc")
            .arg(
                "set -euo pipefail; source \"$ORCHESTRATOR_FAILOVER_LIB\"; \
                 attempt_orchestrator_backend_failover \"${ACTIVE_BACKEND:-}\"",
            )
            .env("ORCHESTRATOR_FAILOVER_LIB", &self.orchestrator_failover_lib)
            .env("ODIN_DIR", &self.odin_dir)
            .env("STATE_FILE", self.state_file())
            .env("ROUTING_FILE", self.routing_file())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        if let Some(active) = active_backend {
            cmd.env("ACTIVE_BACKEND", active);
        }

        let output = cmd
            .output()
            .map_err(|e| RuntimeError::Execution(format!("failover adapter failed: {e}")))?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr)
            .replace('\n', " ")
            .trim()
            .to_string();
        Err(RuntimeError::Execution(format!(
            "failover adapter command failed (exit={}): {}",
            output.status, stderr
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::{BashBackendStateAdapter, BashFailoverAdapter, BashTaskIngressAdapter};
    use odin_core_runtime::{BackendState, FailoverController, TaskIngress};

    #[test]
    fn empty_payload_rejected() {
        let adapter = BashTaskIngressAdapter::new("/does/not/matter.sh");
        let result = adapter.write_task_payload("");
        assert!(result.is_err());
    }

    #[test]
    fn missing_script_returns_execution_error() {
        let adapter = BashTaskIngressAdapter::new("/tmp/odin-this-script-does-not-exist.sh");
        let result = adapter.write_task_payload("{}");
        assert!(result.is_err());
    }

    #[test]
    fn missing_backend_lib_returns_error() {
        let adapter = BashBackendStateAdapter::new(
            "/tmp/odin-backend-state-missing.sh",
            "/tmp/odin-state-missing",
        );
        let result = adapter.get_active_backend();
        assert!(result.is_err());
    }

    #[test]
    fn missing_failover_lib_returns_error() {
        let adapter = BashFailoverAdapter::new(
            "/tmp/odin-orchestrator-failover-missing.sh",
            "/tmp/odin-state-missing",
        );
        let result = adapter.attempt_failover(None);
        assert!(result.is_err());
    }
}
