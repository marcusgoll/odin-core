#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"
MISSING_GUARDRAILS_PATH="/tmp/odin-ops-smoke-missing-guardrails.yaml"
PASS=0
FAIL=0

pass() {
  echo "[ops-cli] PASS $*"
  PASS=$((PASS + 1))
}

fail() {
  echo "[ops-cli] FAIL $*" >&2
  FAIL=$((FAIL + 1))
}

# --- help output includes new commands ---
help_output="$(scripts/odin/odin help 2>&1)"
for cmd in status stop agents logs send test restart; do
  if echo "${help_output}" | grep -q "${cmd}"; then
    pass "help mentions '${cmd}'"
  else
    fail "help missing '${cmd}'"
  fi
done

# --- status runs without crashing (rc 0 is fine) ---
set +e
env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin status >/dev/null 2>&1
rc=$?
set -e
if [[ "${rc}" -eq 0 ]]; then
  pass "status exits rc=0"
else
  fail "status exits rc=${rc} (expected 0)"
fi

# --- agents runs and returns rc 0 ---
set +e
env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin agents >/dev/null 2>&1
rc=$?
set -e
if [[ "${rc}" -eq 0 ]]; then
  pass "agents exits rc=0"
else
  fail "agents exits rc=${rc} (expected 0)"
fi

# --- unknown command exits 64 ---
set +e
env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin unknown-cmd-xyz >/dev/null 2>&1
rc=$?
set -e
if [[ "${rc}" -eq 64 ]]; then
  pass "unknown command exits rc=64"
else
  fail "unknown command exits rc=${rc} (expected 64)"
fi

# --- send rejects empty stdin ---
set +e
echo "" | env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin send >/dev/null 2>&1
rc=$?
set -e
if [[ "${rc}" -ne 0 ]]; then
  pass "send rejects empty stdin (rc=${rc})"
else
  fail "send accepted empty stdin (expected non-zero rc)"
fi

# --- send writes valid JSON to inbox ---
test_inbox="$(mktemp -d /tmp/odin-ops-smoke-inbox.XXXXXX)"
set +e
echo '{"task":"smoke-test"}' | env ODIN_DIR="${test_inbox}" scripts/odin/odin send >/dev/null 2>&1
rc=$?
set -e
if [[ "${rc}" -eq 0 ]] && ls "${test_inbox}/inbox/manual-"*.json >/dev/null 2>&1; then
  pass "send writes JSON to inbox"
else
  fail "send did not write to inbox (rc=${rc})"
fi
rm -rf "${test_inbox}"

# --- ops commands reject extra arguments ---
set +e
env ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" scripts/odin/odin status --verbose >/dev/null 2>&1
rc=$?
set -e
if [[ "${rc}" -eq 64 ]]; then
  pass "status rejects extra args (rc=64)"
else
  fail "status accepted extra args (rc=${rc}, expected 64)"
fi

# --- summary ---
echo ""
echo "[ops-cli] ${PASS} passed, ${FAIL} failed"
if [[ "${FAIL}" -gt 0 ]]; then
  exit 1
fi
echo "[ops-cli] COMPLETE"
