#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"
TMP_DIR="$(mktemp -d /tmp/odin-docs-command-smoke.XXXXXX)"
MISSING_GUARDRAILS_PATH="${TMP_DIR}/missing-guardrails.yaml"

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

run() {
  echo "[docs-command] RUN $*"
  "$@"
  echo "[docs-command] PASS $*"
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

run scripts/odin/odin help
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin connect claude oauth --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin start --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin tui --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin inbox add "bootstrap task" --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin inbox list
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin gateway add cli --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin gateway add slack --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin gateway add telegram --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin verify --dry-run

expect_failure \
  "guardrails missing blocks mutating start" \
  2 \
  "BLOCKED start: guardrails file not found" \
  env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin start

run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin start --dry-run

echo "[docs-command] COMPLETE"
