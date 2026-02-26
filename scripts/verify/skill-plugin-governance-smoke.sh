#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

run_test() {
  local description="$1"
  local package_name="$2"
  local test_name="$3"

  echo "[governance-smoke] RUN ${description}"
  cargo test -p "${package_name}" "${test_name}" -- --exact --nocapture
  echo "[governance-smoke] PASS ${description}"
}

check_doc() {
  local doc_path="$1"
  if [[ ! -s "${doc_path}" ]]; then
    echo "[governance-smoke] FAIL missing required doc: ${doc_path}" >&2
    exit 1
  fi
  echo "[governance-smoke] PASS required doc present: ${doc_path}"
}

run_test \
  "untrusted skill install without ack -> blocked" \
  "odin-cli" \
  "governance_install_requires_ack_for_untrusted"

run_test \
  "stagehand enable without domains/workspaces -> blocked" \
  "odin-cli" \
  "governance_enable_plugin_stagehand_requires_explicit_domains_and_workspaces"

run_test \
  "capability not in manifest -> blocked" \
  "odin-core-runtime" \
  "denies_capability_not_in_manifest"

run_test \
  "allowed manifest capability -> executed" \
  "odin-core-runtime" \
  "emits_manifest_validated_and_capability_used_events_on_success"

check_doc "docs/skill-system.md"
check_doc "docs/stagehand-safety.md"

echo "[governance-smoke] COMPLETE"
