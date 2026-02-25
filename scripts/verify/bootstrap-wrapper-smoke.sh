#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"
TMP_DIR="$(mktemp -d /tmp/odin-bootstrap-wrapper.XXXXXX)"
MISSING_GUARDRAILS_PATH="${TMP_DIR}/missing-guardrails.yaml"

cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

run() {
  echo "[bootstrap-wrapper] RUN $*"
  "$@"
  echo "[bootstrap-wrapper] PASS $*"
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
    echo "[bootstrap-wrapper] ERROR expected rc=${expected_rc} for ${label}, got rc=${rc}" >&2
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

  echo "[bootstrap-wrapper] PASS ${label} rc=${rc}"
}

expect_blocked() {
  local label="$1"
  local expected_err="$2"
  shift 2

  expect_failure "blocked ${label}" 2 "${expected_err}" "$@"
}

run scripts/odin/odin help
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin connect claude oauth --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin start --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin tui --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin inbox add "test task" --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin inbox list
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin inbox list --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin gateway add cli --dry-run
run env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin verify --dry-run

expect_failure \
  "invalid connect extra positional" \
  64 \
  "unexpected argument" \
  scripts/odin/odin connect claude oauth extra --dry-run
expect_failure \
  "invalid start extra positional" \
  64 \
  "unexpected argument" \
  scripts/odin/odin start extra --dry-run
expect_failure \
  "invalid tui extra positional" \
  64 \
  "unexpected argument" \
  scripts/odin/odin tui extra --dry-run
expect_failure \
  "invalid verify extra positional" \
  64 \
  "unexpected argument" \
  scripts/odin/odin verify extra --dry-run
expect_failure \
  "invalid gateway extra positional" \
  64 \
  "unexpected argument" \
  scripts/odin/odin gateway add cli extra --dry-run
expect_failure \
  "invalid inbox list extra positional" \
  64 \
  "unexpected argument" \
  scripts/odin/odin inbox list extra

expect_blocked \
  "wrapper connect without --dry-run when guardrails missing" \
  "BLOCKED connect" \
  env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin connect claude oauth
expect_blocked \
  "wrapper start without --dry-run when guardrails missing" \
  "BLOCKED start" \
  env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin start
expect_blocked \
  "wrapper tui without --dry-run when guardrails missing" \
  "BLOCKED tui" \
  env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin tui
expect_blocked \
  "wrapper inbox add without --dry-run when guardrails missing" \
  "BLOCKED inbox add" \
  env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin inbox add "test task"
expect_blocked \
  "wrapper gateway add without --dry-run when guardrails missing" \
  "BLOCKED gateway add" \
  env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin gateway add cli

expect_blocked \
  "library connect without errexit when guardrails missing" \
  "BLOCKED connect" \
  bash -c '
    set -u -o pipefail
    set +e
    source "$1"
    ODIN_GUARDRAILS_PATH="$2"
    odin_bootstrap_cmd_connect claude oauth
    rc=$?
    exit "${rc}"
  ' _ "${ROOT_DIR}/scripts/odin/lib/bootstrap.sh" "${MISSING_GUARDRAILS_PATH}"
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
expect_blocked \
  "library tui without errexit when guardrails missing" \
  "BLOCKED tui" \
  bash -c '
    set -u -o pipefail
    set +e
    source "$1"
    ODIN_GUARDRAILS_PATH="$2"
    odin_bootstrap_cmd_tui
    rc=$?
    exit "${rc}"
  ' _ "${ROOT_DIR}/scripts/odin/lib/bootstrap.sh" "${MISSING_GUARDRAILS_PATH}"
expect_blocked \
  "library inbox add without errexit when guardrails missing" \
  "BLOCKED inbox add" \
  bash -c '
    set -u -o pipefail
    set +e
    source "$1"
    ODIN_GUARDRAILS_PATH="$2"
    odin_bootstrap_cmd_inbox_add "test task"
    rc=$?
    exit "${rc}"
  ' _ "${ROOT_DIR}/scripts/odin/lib/bootstrap.sh" "${MISSING_GUARDRAILS_PATH}"
expect_blocked \
  "library gateway add without errexit when guardrails missing" \
  "BLOCKED gateway add" \
  bash -c '
    set -u -o pipefail
    set +e
    source "$1"
    ODIN_GUARDRAILS_PATH="$2"
    odin_bootstrap_cmd_gateway_add cli
    rc=$?
    exit "${rc}"
  ' _ "${ROOT_DIR}/scripts/odin/lib/bootstrap.sh" "${MISSING_GUARDRAILS_PATH}"

echo "[bootstrap-wrapper] COMPLETE"
