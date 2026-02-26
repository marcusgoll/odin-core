#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

CANONICAL_SKILL="examples/skills/sass/v0.1/run_tests.skill.xml"
BROKEN_SKILL="examples/skills/sass/v0.1/broken-missing-target.skill.xml"

echo "[sass-governance] RUN canonical skill validation"
cargo run -p odin-cli -- skill validate "${CANONICAL_SKILL}"

echo "[sass-governance] RUN broken skill validation (must fail)"
if cargo run -p odin-cli -- skill validate "${BROKEN_SKILL}"; then
  echo "[sass-governance] ERROR broken skill unexpectedly passed validation: ${BROKEN_SKILL}" >&2
  exit 1
fi

echo "[sass-governance] PASS smoke checks"
