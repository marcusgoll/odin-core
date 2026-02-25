#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: compat-regression.sh --legacy-root <path>

Runs the pinned Odin compatibility regression matrix against a legacy Odin checkout.
EOF
}

LEGACY_ROOT=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --legacy-root)
      LEGACY_ROOT="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 64
      ;;
  esac
done

if [[ -z "${LEGACY_ROOT}" ]]; then
  echo "ERROR: --legacy-root is required" >&2
  usage >&2
  exit 64
fi

if [[ ! -d "${LEGACY_ROOT}" ]]; then
  echo "ERROR: legacy root not found: ${LEGACY_ROOT}" >&2
  exit 66
fi

run_step() {
  local label="$1"
  shift
  echo "[compat] RUN ${label}: $*"
  "$@"
  echo "[compat] PASS ${label}"
}

cd "${LEGACY_ROOT}"

run_step keepalive-syntax bash -n scripts/odin/keepalive.sh
run_step backend-state scripts/odin/tests/backend-state-test.sh
run_step backend-switch-events scripts/odin/tests/backend-switch-events-test.sh
run_step keepalive-failover scripts/odin/tests/keepalive-failover-test.sh
run_step keepalive-cooldown scripts/odin/tests/keepalive-cooldown-test.sh
run_step keepalive-antiflap scripts/odin/tests/keepalive-antiflap-test.sh
run_step service-launcher scripts/odin/tests/odin-service-launcher-test.sh
run_step spend-ledger scripts/odin/tests/spend-ledger-test.sh

echo "[compat] COMPLETE all regression checks passed"
