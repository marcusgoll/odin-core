#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"
TMP_DIR="$(mktemp -d /tmp/odin-mode-confidence.XXXXXX)"
STATE_PATH="${TMP_DIR}/bootstrap-state.json"

cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

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

echo "[mode-confidence] COMPLETE"
