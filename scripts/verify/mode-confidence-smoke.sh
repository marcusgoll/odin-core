#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"
TMP_DIR="$(mktemp -d /tmp/odin-mode-confidence.XXXXXX)"
STATE_PATH="${TMP_DIR}/bootstrap-state.json"
CLI_STATE_PATH="${TMP_DIR}/cli-bootstrap-state.json"
CLI_GUARDRAILS_PATH="${TMP_DIR}/guardrails.yaml"
READONLY_STATE_DIR="${TMP_DIR}/readonly-state-dir"

cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

cat >"${CLI_GUARDRAILS_PATH}" <<'EOF'
denylist: []
confirm_required:
  - integration
EOF

mkdir -p "${READONLY_STATE_DIR}"
chmod 500 "${READONLY_STATE_DIR}"

assert_eq() {
  local label="$1"
  local expected="$2"
  local actual="$3"
  if [[ "${expected}" != "${actual}" ]]; then
    echo "[mode-confidence] ERROR ${label}: expected='${expected}' actual='${actual}'" >&2
    exit 1
  fi
  echo "[mode-confidence] PASS ${label} => ${actual}"
}

assert_blocked() {
  local label="$1"
  shift
  if "$@"; then
    echo "[mode-confidence] ERROR expected blocked: ${label}" >&2
    exit 1
  fi
  echo "[mode-confidence] PASS blocked ${label}"
}

source "${ROOT_DIR}/scripts/odin/lib/mode_state.sh"

export ODIN_MODE_STATE_PATH="${STATE_PATH}"
odin_mode_state_init

assert_eq "initial mode" "BOOTSTRAP" "$(odin_mode_state_get mode)"
assert_eq "initial confidence" "10" "$(odin_mode_state_get confidence)"
assert_blocked "initial can_operate" odin_mode_state_can_operate
assert_blocked "set OPERATE before checkpoints" odin_mode_state_set_mode OPERATE

before_confidence="$(odin_mode_state_get confidence)"
odin_mode_state_record_event "provider.connected.verified"
after_confidence="$(odin_mode_state_get confidence)"
if (( after_confidence <= before_confidence )); then
  echo "[mode-confidence] ERROR confidence did not increase after verified checkpoint" >&2
  exit 1
fi
echo "[mode-confidence] PASS confidence increase ${before_confidence} -> ${after_confidence}"

odin_mode_state_record_event "tui.opened.verified"
odin_mode_state_record_event "inbox.first_item.verified"
odin_mode_state_record_event "task.split.verified"
odin_mode_state_record_event "delegation.completed.verified"

threshold_confidence="$(odin_mode_state_get confidence)"
if (( threshold_confidence < 60 )); then
  echo "[mode-confidence] ERROR expected confidence >= 60, got ${threshold_confidence}" >&2
  exit 1
fi
echo "[mode-confidence] PASS threshold reached confidence=${threshold_confidence}"

assert_blocked "OPERATE without guardrails + task cycle" odin_mode_state_can_operate
assert_blocked "set OPERATE without guardrails + task cycle" odin_mode_state_set_mode OPERATE

odin_mode_state_record_event "guardrails.acknowledged.verified"
assert_blocked "OPERATE without task cycle" odin_mode_state_can_operate

odin_mode_state_record_event "task.cycle.verified"
odin_mode_state_can_operate
odin_mode_state_set_mode OPERATE
assert_eq "operate mode enabled" "OPERATE" "$(odin_mode_state_get mode)"

odin_mode_state_record_event "verify.failed"
assert_eq "recovery fallback on verify failure" "RECOVERY" "$(odin_mode_state_get mode)"

persisted_mode="$(
  bash -c '
    set -euo pipefail
    source "$1"
    export ODIN_MODE_STATE_PATH="$2"
    odin_mode_state_init
    odin_mode_state_get mode
  ' _ "${ROOT_DIR}/scripts/odin/lib/mode_state.sh" "${STATE_PATH}"
)"
assert_eq "state persists on disk" "RECOVERY" "${persisted_mode}"

run_cli() {
  env \
    ODIN_MODE_STATE_PATH="${CLI_STATE_PATH}" \
    ODIN_GUARDRAILS_PATH="${CLI_GUARDRAILS_PATH}" \
    ODIN_GUARDRAILS_ACK=yes \
    scripts/odin/odin "$@"
}

echo "[mode-confidence] RUN cli path connect+tui+inbox+verify"
low_confidence_state_path="${TMP_DIR}/low-confidence-state.json"
low_verify_out_file="$(mktemp "${TMP_DIR}/low-verify.out.XXXXXX")"
low_verify_err_file="$(mktemp "${TMP_DIR}/low-verify.err.XXXXXX")"
set +e
env \
  ODIN_MODE_STATE_PATH="${low_confidence_state_path}" \
  ODIN_GUARDRAILS_PATH="${CLI_GUARDRAILS_PATH}" \
  ODIN_GUARDRAILS_ACK=yes \
  scripts/odin/odin verify >"${low_verify_out_file}" 2>"${low_verify_err_file}"
low_verify_rc=$?
set -e
if [[ "${low_verify_rc}" -ne 2 ]]; then
  echo "[mode-confidence] ERROR expected low-confidence verify rc=2, got rc=${low_verify_rc}" >&2
  cat "${low_verify_out_file}" >&2
  cat "${low_verify_err_file}" >&2
  exit 1
fi
if ! grep -Fq "BLOCKED mode transition to OPERATE" "${low_verify_err_file}"; then
  echo "[mode-confidence] ERROR expected blocked mode transition message on low-confidence verify" >&2
  cat "${low_verify_out_file}" >&2
  cat "${low_verify_err_file}" >&2
  exit 1
fi
echo "[mode-confidence] PASS low-confidence verify blocked rc=${low_verify_rc}"

run_cli connect claude oauth --confirm >/dev/null
run_cli tui >/dev/null
run_cli inbox add "cli task" >/dev/null

export ODIN_MODE_STATE_PATH="${CLI_STATE_PATH}"
assert_blocked "cli can_operate before verify task cycle" odin_mode_state_can_operate
run_cli verify >/dev/null

assert_eq "cli mode transitions to OPERATE" "OPERATE" "$(odin_mode_state_get mode)"
assert_eq "cli task cycle marked" "true" "$(odin_mode_state_get task_cycle_verified)"
cli_confidence="$(odin_mode_state_get confidence)"
if (( cli_confidence < 60 )); then
  echo "[mode-confidence] ERROR expected cli confidence >= 60, got ${cli_confidence}" >&2
  exit 1
fi
echo "[mode-confidence] PASS cli confidence=${cli_confidence}"

echo "[mode-confidence] RUN explicit nested custom ODIN_MODE_STATE_PATH"
nested_state_path="${TMP_DIR}/nested/custom/path/state.json"
rm -rf "${TMP_DIR}/nested"
env \
  ODIN_MODE_STATE_PATH="${nested_state_path}" \
  ODIN_GUARDRAILS_PATH="${CLI_GUARDRAILS_PATH}" \
  ODIN_GUARDRAILS_ACK=yes \
  scripts/odin/odin connect claude oauth --confirm >/dev/null
if [[ ! -f "${nested_state_path}" ]]; then
  echo "[mode-confidence] ERROR expected nested custom state file to be created: ${nested_state_path}" >&2
  exit 1
fi
export ODIN_MODE_STATE_PATH="${nested_state_path}"
nested_confidence="$(odin_mode_state_get confidence)"
if (( nested_confidence <= 10 )); then
  echo "[mode-confidence] ERROR expected nested custom state confidence increase, got ${nested_confidence}" >&2
  exit 1
fi
echo "[mode-confidence] PASS nested custom path created confidence=${nested_confidence}"

echo "[mode-confidence] RUN state write failure propagation"
readonly_state_path="${READONLY_STATE_DIR}/state.json"
readonly_err_file="$(mktemp "${TMP_DIR}/readonly.err.XXXXXX")"
readonly_out_file="$(mktemp "${TMP_DIR}/readonly.out.XXXXXX")"
set +e
env \
  ODIN_MODE_STATE_PATH="${readonly_state_path}" \
  ODIN_GUARDRAILS_PATH="${CLI_GUARDRAILS_PATH}" \
  ODIN_GUARDRAILS_ACK=yes \
  scripts/odin/odin connect claude oauth --confirm >"${readonly_out_file}" 2>"${readonly_err_file}"
readonly_rc=$?
set -e
if [[ "${readonly_rc}" -eq 0 ]]; then
  echo "[mode-confidence] ERROR expected non-zero rc when state cannot be written" >&2
  cat "${readonly_out_file}" >&2
  cat "${readonly_err_file}" >&2
  exit 1
fi
if ! grep -Eiq "mode state|state.*failed|failed.*state" "${readonly_err_file}"; then
  echo "[mode-confidence] ERROR expected state failure message in stderr" >&2
  cat "${readonly_out_file}" >&2
  cat "${readonly_err_file}" >&2
  exit 1
fi
echo "[mode-confidence] PASS state failure surfaced rc=${readonly_rc}"

echo "[mode-confidence] RUN help command should not initialize mode state"
help_state_path="${TMP_DIR}/help-state.json"
env ODIN_MODE_STATE_PATH="${help_state_path}" scripts/odin/odin help >/dev/null
if [[ -f "${help_state_path}" ]]; then
  echo "[mode-confidence] ERROR help unexpectedly initialized mode state file" >&2
  exit 1
fi
echo "[mode-confidence] PASS help does not initialize mode state"

echo "[mode-confidence] COMPLETE"
