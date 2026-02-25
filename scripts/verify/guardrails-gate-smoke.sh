#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"
TMP_DIR="$(mktemp -d /tmp/odin-guardrails-gate.XXXXXX)"
MISSING_GUARDRAILS_PATH="${TMP_DIR}/missing-guardrails.yaml"
GUARDRAILS_PATH="${TMP_DIR}/guardrails.yaml"
UNREADABLE_GUARDRAILS_PATH="${TMP_DIR}/guardrails-unreadable.yaml"
QUOTED_DENY_GUARDRAILS_PATH="${TMP_DIR}/guardrails-quoted-deny.yaml"
FLOW_DENY_GUARDRAILS_PATH="${TMP_DIR}/guardrails-flow-deny.yaml"
BOM_CONFIRM_GUARDRAILS_PATH="${TMP_DIR}/guardrails-bom-confirm.yaml"
QUOTED_KEY_DENY_GUARDRAILS_PATH="${TMP_DIR}/guardrails-quoted-key-deny.yaml"

cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

cat >"${GUARDRAILS_PATH}" <<'EOF'
denylist:
  - gateway.add
confirm_required:
  - integration
EOF

cat >"${UNREADABLE_GUARDRAILS_PATH}" <<'EOF'
denylist:
  - gateway.add
confirm_required:
  - integration
EOF
chmod 000 "${UNREADABLE_GUARDRAILS_PATH}"

cat >"${QUOTED_DENY_GUARDRAILS_PATH}" <<'EOF'
denylist:
  - "gateway.add"
confirm_required:
  - integration
EOF

cat >"${FLOW_DENY_GUARDRAILS_PATH}" <<'EOF'
denylist: [gateway.add, connect]
confirm_required:
  - integration
EOF

printf '\357\273\277confirm_required:\n  - integration\ndenylist:\n  - gateway.add\n' >"${BOM_CONFIRM_GUARDRAILS_PATH}"

cat >"${QUOTED_KEY_DENY_GUARDRAILS_PATH}" <<'EOF'
"denylist":
  - gateway.add
confirm_required:
  - integration
EOF

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
    echo "[guardrails-gate] ERROR expected rc=${expected_rc} for ${label}, got rc=${rc}" >&2
    cat "${out_file}" >&2
    cat "${err_file}" >&2
    exit 1
  fi

  if [[ -n "${expected_err}" ]] && ! grep -Fq "${expected_err}" "${err_file}"; then
    echo "[guardrails-gate] ERROR expected stderr pattern '${expected_err}' for ${label}" >&2
    cat "${out_file}" >&2
    cat "${err_file}" >&2
    exit 1
  fi

  echo "[guardrails-gate] PASS ${label} rc=${rc}"
}

expect_success() {
  local label="$1"
  local expected_out="$2"
  shift 2

  local out_file
  local err_file
  out_file="$(mktemp "${TMP_DIR}/out.XXXXXX")"
  err_file="$(mktemp "${TMP_DIR}/err.XXXXXX")"

  set +e
  "$@" >"${out_file}" 2>"${err_file}"
  local rc=$?
  set -e

  if [[ "${rc}" -ne 0 ]]; then
    echo "[guardrails-gate] ERROR expected rc=0 for ${label}, got rc=${rc}" >&2
    cat "${out_file}" >&2
    cat "${err_file}" >&2
    exit 1
  fi

  if [[ -n "${expected_out}" ]] && ! grep -Fq "${expected_out}" "${out_file}"; then
    echo "[guardrails-gate] ERROR expected stdout pattern '${expected_out}' for ${label}" >&2
    cat "${out_file}" >&2
    cat "${err_file}" >&2
    exit 1
  fi

  echo "[guardrails-gate] PASS ${label}"
}

expect_failure \
  "A missing guardrails + risky action blocks" \
  2 \
  "BLOCKED start: guardrails file not found" \
  scripts/odin/odin --guardrails "${MISSING_GUARDRAILS_PATH}" start

expect_failure \
  "B guardrails present but unacknowledged blocks" \
  2 \
  "acknowledgement required" \
  scripts/odin/odin --guardrails "${GUARDRAILS_PATH}" start

expect_success \
  "C guardrails present + acknowledged allows" \
  "start placeholder" \
  env ODIN_GUARDRAILS_ACK=yes scripts/odin/odin --guardrails "${GUARDRAILS_PATH}" start

expect_failure \
  "confirm-required integration blocks without --confirm" \
  2 \
  "requires --confirm" \
  env ODIN_GUARDRAILS_ACK=yes scripts/odin/odin --guardrails "${GUARDRAILS_PATH}" connect claude oauth

expect_success \
  "confirm-required integration allows with --confirm" \
  "connect placeholder provider=claude auth=oauth" \
  env ODIN_GUARDRAILS_ACK=yes scripts/odin/odin --guardrails "${GUARDRAILS_PATH}" connect claude oauth --confirm

expect_failure \
  "denylist blocks even with acknowledgement" \
  2 \
  "denylisted by guardrails" \
  env ODIN_GUARDRAILS_ACK=yes scripts/odin/odin --guardrails "${GUARDRAILS_PATH}" gateway add cli --confirm

expect_failure \
  "unreadable guardrails file blocks risky action" \
  2 \
  "guardrails policy unreadable or unsupported" \
  env ODIN_GUARDRAILS_ACK=yes scripts/odin/odin --guardrails "${UNREADABLE_GUARDRAILS_PATH}" start

expect_failure \
  "quoted denylist item blocks action" \
  2 \
  "denylisted by guardrails" \
  env ODIN_GUARDRAILS_ACK=yes scripts/odin/odin --guardrails "${QUOTED_DENY_GUARDRAILS_PATH}" gateway add cli --confirm

expect_failure \
  "flow-style denylist item blocks action" \
  2 \
  "denylisted by guardrails" \
  env ODIN_GUARDRAILS_ACK=yes scripts/odin/odin --guardrails "${FLOW_DENY_GUARDRAILS_PATH}" connect claude oauth --confirm

expect_failure \
  "BOM-prefixed confirm_required key enforces --confirm" \
  2 \
  "requires --confirm" \
  env ODIN_GUARDRAILS_ACK=yes scripts/odin/odin --guardrails "${BOM_CONFIRM_GUARDRAILS_PATH}" connect claude oauth

expect_failure \
  "quoted denylist key blocks action" \
  2 \
  "denylisted by guardrails" \
  env ODIN_GUARDRAILS_ACK=yes scripts/odin/odin --guardrails "${QUOTED_KEY_DENY_GUARDRAILS_PATH}" gateway add cli --confirm

expect_success \
  "default guardrails example is consumable with acknowledgement" \
  "start placeholder" \
  env ODIN_GUARDRAILS_ACK=yes scripts/odin/odin --guardrails config/guardrails.yaml.example start

echo "[guardrails-gate] COMPLETE"
