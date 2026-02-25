#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

run_test() {
  local test_name="$1"
  echo "[plugin-matrix] RUN ${test_name}"
  cargo test -p odin-plugin-manager "${test_name}" -- --nocapture
  echo "[plugin-matrix] PASS ${test_name}"
}

# Local path install
run_test local_install_parses_manifest

# Git ref install
run_test git_ref_install_from_local_repo

# Artifact install + checksum pinning
run_test artifact_install_from_targz
run_test artifact_install_from_targz_with_nested_root

# Signed artifact policy paths (negative + positive)
run_test local_install_rejects_when_signature_required_but_missing
run_test local_install_accepts_valid_minisign_signature_when_required
run_test local_install_accepts_valid_sigstore_signature_when_required

echo "[plugin-matrix] COMPLETE"
