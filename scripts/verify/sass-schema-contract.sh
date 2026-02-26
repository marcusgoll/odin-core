#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

SCHEMA_PATH="${1:-schemas/skill-sass.v0.1.schema.json}"

echo "[sass-schema] RUN contract checks"

if [[ ! -f "${SCHEMA_PATH}" ]]; then
  echo "[sass-schema] ERROR missing schema: ${SCHEMA_PATH}" >&2
  exit 66
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "[sass-schema] ERROR jq is required but not installed" >&2
  exit 69
fi

check_jq() {
  local description="$1"
  local expr="$2"
  if ! jq -e "${expr}" "${SCHEMA_PATH}" >/dev/null; then
    echo "[sass-schema] ERROR ${description} (${SCHEMA_PATH})" >&2
    exit 65
  fi
}

check_jq 'missing or invalid top-level "$schema"' 'has("$schema") and (.["$schema"] | type == "string" and length > 0)'
check_jq 'missing or invalid top-level "title"' 'has("title") and (.title | type == "string" and length > 0)'
check_jq 'top-level "type" must be "object"' 'has("type") and .type == "object"'
check_jq 'missing or invalid top-level "required" array' 'has("required") and (.required | type == "array" and all(.[]; type == "string"))'
check_jq 'top-level required is missing one or more mandatory fields' '(["skill_id", "version", "permissions", "states", "initial_state"] - .required | length) == 0'

echo "[sass-schema] PASS contract checks"
