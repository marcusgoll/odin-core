#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

CANONICAL_SKILL="examples/skills/sass/v0.1/run_tests.skill.xml"
BROKEN_SKILL="examples/skills/sass/v0.1/broken-missing-target.skill.xml"
BROKEN_STDERR_FILE="$(mktemp)"
trap 'rm -f "${BROKEN_STDERR_FILE}"' EXIT

if ! test -f "${CANONICAL_SKILL}"; then
  echo "[sass-governance] ERROR missing canonical fixture: ${CANONICAL_SKILL}" >&2
  exit 1
fi

if ! test -f "${BROKEN_SKILL}"; then
  echo "[sass-governance] ERROR missing broken fixture: ${BROKEN_SKILL}" >&2
  exit 1
fi

echo "[sass-governance] RUN canonical skill validation"
cargo run -p odin-cli -- skill validate "${CANONICAL_SKILL}"

echo "[sass-governance] RUN broken skill validation (must fail)"
set +e
cargo run -p odin-cli -- skill validate "${BROKEN_SKILL}" 2>"${BROKEN_STDERR_FILE}"
BROKEN_STATUS=$?
set -e

if [[ ${BROKEN_STATUS} -eq 0 ]]; then
  echo "[sass-governance] ERROR broken skill unexpectedly passed validation: ${BROKEN_SKILL}" >&2
  exit 1
fi

if ! grep -Fq "validation failed" "${BROKEN_STDERR_FILE}"; then
  echo "[sass-governance] ERROR broken skill failed for an unexpected reason (missing marker: validation failed)" >&2
  cat "${BROKEN_STDERR_FILE}" >&2
  exit 1
fi

if ! grep -Fq "unknown target" "${BROKEN_STDERR_FILE}"; then
  echo "[sass-governance] ERROR broken skill failed for an unexpected reason (missing marker: unknown target)" >&2
  cat "${BROKEN_STDERR_FILE}" >&2
  exit 1
fi

echo "[sass-governance] PASS smoke checks"
