#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

SCHEMA_PATH="schemas/skill-sass.v0.1.schema.json"

echo "[sass-schema] RUN contract checks"

if [[ ! -f "${SCHEMA_PATH}" ]]; then
  echo "[sass-schema] ERROR missing schema: ${SCHEMA_PATH}" >&2
  exit 66
fi

for required_key in '"$schema"' '"title"' '"required"'; do
  if ! grep -Fq "${required_key}" "${SCHEMA_PATH}"; then
    echo "[sass-schema] ERROR missing key ${required_key} in ${SCHEMA_PATH}" >&2
    exit 65
  fi
done

echo "[sass-schema] PASS contract checks"
