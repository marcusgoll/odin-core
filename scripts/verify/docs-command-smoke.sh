#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"
TMP_DIR="$(mktemp -d /tmp/odin-docs-command-smoke.XXXXXX)"
MISSING_GUARDRAILS_PATH="${TMP_DIR}/missing-guardrails.yaml"
MODE_STATE_PATH="${TMP_DIR}/bootstrap-state.json"

cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

require_file() {
  local path="$1"
  if [[ ! -f "${path}" ]]; then
    echo "[docs-command] ERROR missing required file: ${path}" >&2
    exit 1
  fi
  echo "[docs-command] PASS file exists: ${path}"
}

require_snippet() {
  local path="$1"
  local snippet="$2"
  if ! grep -Fq "${snippet}" "${path}"; then
    echo "[docs-command] ERROR missing snippet in ${path}: ${snippet}" >&2
    exit 1
  fi
  echo "[docs-command] PASS snippet in ${path}: ${snippet}"
}

run_wrapper() {
  env \
    ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" \
    ODIN_MODE_STATE_PATH="${MODE_STATE_PATH}" \
    "$@"
}

expect_success() {
  local label="$1"
  local expected_out="$2"
  shift 2

  local out_file
  local err_file
  out_file="$(mktemp "${TMP_DIR}/out.XXXXXX")"
  err_file="$(mktemp "${TMP_DIR}/err.XXXXXX")"

  echo "[docs-command] RUN ${label}"

  set +e
  "$@" >"${out_file}" 2>"${err_file}"
  local rc=$?
  set -e

  if [[ "${rc}" -ne 0 ]]; then
    echo "[docs-command] ERROR expected rc=0 for ${label}, got rc=${rc}" >&2
    cat "${out_file}" >&2
    cat "${err_file}" >&2
    exit 1
  fi

  if [[ -n "${expected_out}" ]] && ! grep -Fq "${expected_out}" "${out_file}"; then
    echo "[docs-command] ERROR expected stdout pattern '${expected_out}' for ${label}" >&2
    cat "${out_file}" >&2
    cat "${err_file}" >&2
    exit 1
  fi

  echo "[docs-command] PASS ${label}"
}

expect_failure() {
  local label="$1"
  local expected_rc="$2"
  local expected_err="$3"
  shift 3

  local out_file
  local err_file
  out_file="$(mktemp "${TMP_DIR}/out.XXXXXX")"
  err_file="$(mktemp "${TMP_DIR}/err.XXXXXX")"

  echo "[docs-command] RUN ${label}"

  set +e
  "$@" >"${out_file}" 2>"${err_file}"
  local rc=$?
  set -e

  if [[ "${rc}" -ne "${expected_rc}" ]]; then
    echo "[docs-command] ERROR expected rc=${expected_rc} for ${label}, got rc=${rc}" >&2
    cat "${out_file}" >&2
    cat "${err_file}" >&2
    exit 1
  fi

  if [[ -n "${expected_err}" ]] && ! grep -Fq "${expected_err}" "${err_file}"; then
    echo "[docs-command] ERROR expected stderr pattern '${expected_err}' for ${label}" >&2
    cat "${out_file}" >&2
    cat "${err_file}" >&2
    exit 1
  fi

  echo "[docs-command] PASS ${label} rc=${rc}"
}

require_file "docs/quickstart.md"
require_file "docs/integrations/n8n.md"
require_file "docs/integrations/slack.md"
require_file "docs/integrations/telegram.md"

require_snippet "README.md" "docs/quickstart.md"
require_snippet "README.md" "docs/integrations/n8n.md"
require_snippet "README.md" "docs/integrations/slack.md"
require_snippet "README.md" "docs/integrations/telegram.md"

require_snippet "docs/quickstart.md" "scripts/odin/odin help"
require_snippet "docs/quickstart.md" "scripts/odin/odin connect claude oauth --dry-run"
require_snippet "docs/quickstart.md" "scripts/odin/odin start --dry-run"
require_snippet "docs/quickstart.md" "scripts/odin/odin tui --dry-run"
require_snippet "docs/quickstart.md" "scripts/odin/odin inbox add \"bootstrap task\" --dry-run"
require_snippet "docs/quickstart.md" "scripts/odin/odin inbox list"
require_snippet "docs/quickstart.md" "scripts/odin/odin gateway add cli --dry-run"
require_snippet "docs/quickstart.md" "scripts/odin/odin verify --dry-run"
require_snippet "docs/quickstart.md" "bash scripts/verify/docs-command-smoke.sh"

require_snippet "docs/integrations/n8n.md" "scripts/odin/odin gateway add cli --dry-run"
require_snippet "docs/integrations/slack.md" "scripts/odin/odin gateway add slack --dry-run"
require_snippet "docs/integrations/telegram.md" "scripts/odin/odin gateway add telegram --dry-run"

expect_success \
  "help usage" \
  "Usage:" \
  run_wrapper scripts/odin/odin help
expect_success \
  "quickstart connect dry-run" \
  "DRY-RUN connect provider=claude auth=oauth" \
  run_wrapper scripts/odin/odin connect claude oauth --dry-run
expect_success \
  "quickstart start dry-run" \
  "DRY-RUN start" \
  run_wrapper scripts/odin/odin start --dry-run
expect_success \
  "quickstart tui dry-run" \
  "DRY-RUN tui" \
  run_wrapper scripts/odin/odin tui --dry-run
expect_success \
  "quickstart inbox add dry-run" \
  "DRY-RUN inbox add title=bootstrap task" \
  run_wrapper scripts/odin/odin inbox add "bootstrap task" --dry-run
expect_success \
  "quickstart inbox list" \
  "inbox list placeholder (empty)" \
  run_wrapper scripts/odin/odin inbox list
expect_success \
  "quickstart gateway cli dry-run" \
  "DRY-RUN gateway add source=cli" \
  run_wrapper scripts/odin/odin gateway add cli --dry-run
expect_success \
  "quickstart gateway slack dry-run" \
  "DRY-RUN gateway add source=slack" \
  run_wrapper scripts/odin/odin gateway add slack --dry-run
expect_success \
  "quickstart gateway telegram dry-run" \
  "DRY-RUN gateway add source=telegram" \
  run_wrapper scripts/odin/odin gateway add telegram --dry-run
expect_success \
  "quickstart verify dry-run" \
  "DRY-RUN verify" \
  run_wrapper scripts/odin/odin verify --dry-run
expect_success \
  "n8n inbox add dry-run" \
  "DRY-RUN inbox add title=n8n bootstrap task" \
  run_wrapper scripts/odin/odin inbox add "n8n bootstrap task" --dry-run

expect_failure \
  "guardrails missing blocks mutating start" \
  2 \
  "BLOCKED start: guardrails file not found" \
  run_wrapper scripts/odin/odin start

expect_success \
  "guardrails missing smallest fix is --dry-run" \
  "DRY-RUN start" \
  run_wrapper scripts/odin/odin start --dry-run

echo "[docs-command] COMPLETE"
