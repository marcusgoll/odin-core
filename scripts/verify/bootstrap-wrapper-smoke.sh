#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"
MISSING_GUARDRAILS_PATH="$(mktemp -u /tmp/odin-missing-guardrails.XXXXXX.yaml)"

run() {
  echo "[bootstrap-wrapper] RUN $*"
  "$@"
  echo "[bootstrap-wrapper] PASS $*"
}

expect_blocked() {
  local label="$1"
  local expected_err="$2"
  shift 2

  local out_file
  local err_file
  out_file="$(mktemp /tmp/odin-bootstrap-wrapper-out.XXXXXX)"
  err_file="$(mktemp /tmp/odin-bootstrap-wrapper-err.XXXXXX)"

  set +e
  "$@" >"${out_file}" 2>"${err_file}"
  local rc=$?
  set -e

  if [[ "${rc}" -eq 0 ]]; then
    echo "[bootstrap-wrapper] ERROR expected non-zero for ${label}" >&2
    cat "${out_file}" >&2
    cat "${err_file}" >&2
    exit 1
  fi

  if [[ -n "${expected_err}" ]] && ! grep -q "${expected_err}" "${err_file}"; then
    echo "[bootstrap-wrapper] ERROR expected stderr pattern '${expected_err}' for ${label}" >&2
    cat "${out_file}" >&2
    cat "${err_file}" >&2
    exit 1
  fi

  echo "[bootstrap-wrapper] PASS blocked ${label} rc=${rc}"
}

run scripts/odin/odin help
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin connect claude oauth --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin start --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin tui --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin inbox add "test task" --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin inbox list
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin gateway add cli --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin verify --dry-run

expect_blocked \
  "wrapper start without --dry-run when guardrails missing" \
  "BLOCKED start" \
  env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin start

expect_blocked \
  "library start without errexit when guardrails missing" \
  "BLOCKED start" \
  bash -c '
    set -u -o pipefail
    set +e
    source "$1"
    ODIN_GUARDRAILS_PATH="$2"
    odin_bootstrap_cmd_start
    rc=$?
    exit "${rc}"
  ' _ "${ROOT_DIR}/scripts/odin/lib/bootstrap.sh" "${MISSING_GUARDRAILS_PATH}"

echo "[bootstrap-wrapper] COMPLETE"
