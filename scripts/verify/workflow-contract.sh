#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

WORKFLOW_FILE=".github/workflows/ci.yml"

if [[ ! -f "${WORKFLOW_FILE}" ]]; then
  echo "[workflow-contract] ERROR missing ${WORKFLOW_FILE}" >&2
  exit 1
fi

contract_validation_block="$(
  awk '
    /^  contract-validation:$/ { in_block=1; print; next }
    in_block && /^  [a-zA-Z0-9_-]+:$/ { in_block=0 }
    in_block { print }
  ' "${WORKFLOW_FILE}"
)"

if [[ -z "${contract_validation_block}" ]]; then
  echo "[workflow-contract] ERROR missing contract-validation job in ${WORKFLOW_FILE}" >&2
  exit 1
fi

required_step_names=(
  'Workflow contract lint'
  'SASS skills contract lint'
)

for step_name in "${required_step_names[@]}"; do
  if ! grep -Fq -- "- name: ${step_name}" <<< "${contract_validation_block}"; then
    echo "[workflow-contract] ERROR missing required step name in contract-validation job: ${step_name}" >&2
    exit 1
  fi
done

required_step_runs=(
  'run: bash scripts/verify/workflow-contract.sh'
  'run: bash scripts/verify/skills-contract.sh'
)

for run_cmd in "${required_step_runs[@]}"; do
  if ! grep -Fq -- "${run_cmd}" <<< "${contract_validation_block}"; then
    echo "[workflow-contract] ERROR missing required run command in contract-validation job: ${run_cmd}" >&2
    exit 1
  fi
done

echo "[workflow-contract] PASS CI contains workflow and SASS contract gates"
echo "[workflow-contract] COMPLETE"
