#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CORE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "${WORK_DIR}"' EXIT

export ODIN_DIR="${WORK_DIR}/target"
mkdir -p "${ODIN_DIR}"

PASS=0; FAIL=0
assert_eq() {
  local label="$1" expected="$2" actual="$3"
  if [[ "${expected}" == "${actual}" ]]; then PASS=$((PASS + 1))
  else FAIL=$((FAIL + 1)); echo "FAIL: ${label}: expected '${expected}', got '${actual}'" >&2; fi
}
assert_file_exists() {
  local label="$1" path="$2"
  if [[ -f "${path}" ]]; then PASS=$((PASS + 1))
  else FAIL=$((FAIL + 1)); echo "FAIL: ${label}: not found: ${path}" >&2; fi
}

# Create a minimal test bundle
BUNDLE="${WORK_DIR}/bundle"
mkdir -p "${BUNDLE}"/{skills/odin-test/,config/policy,memory/hot,memory/cold,state/kanban,state/budgets,state/autonomy/contracts,quarantine}
cat > "${BUNDLE}/skills/odin-test/SKILL.md" <<'SKILL'
---
name: odin-test
---
# Test Skill
SKILL
echo "schema_version: 1" > "${BUNDLE}/config/workers.yaml"
echo "schema_version: 1" > "${BUNDLE}/config/routing.yaml"
echo "default: deny" > "${BUNDLE}/config/policy/core-policy.yaml"
echo "# learnings" > "${BUNDLE}/memory/hot/learnings.md"
echo '{"schema_version":1}' > "${BUNDLE}/state/state.json"
echo '{}' > "${BUNDLE}/state/kanban/board.json"
echo '{}' > "${BUNDLE}/state/budgets/daily.json"
echo '{}' > "${BUNDLE}/state/budgets/limits.json"
echo '{}' > "${BUNDLE}/state/autonomy/contracts/test.json"
cat > "${BUNDLE}/quarantine/README.md" <<'Q'
# Quarantine
Q

# Generate checksums for MANIFEST
CHECKSUMS="{}"
while IFS= read -r -d '' f; do
  rel="${f#${BUNDLE}/}"
  hash="$(sha256sum "$f" | awk '{print $1}')"
  CHECKSUMS="$(echo "${CHECKSUMS}" | jq --arg k "${rel}" --arg v "sha256:${hash}" '. + {($k): $v}')"
done < <(find "${BUNDLE}" -type f -not -name MANIFEST.json -print0 | sort -z)

jq -n --argjson checksums "${CHECKSUMS}" '{
  schema_version: 1,
  exported_at: "2026-02-27T00:00:00Z",
  source_repo: "test",
  source_commit: "abc123",
  counts: {skills_converted:1,skills_learned:0,memory_hot_files:1,memory_cold_files:0,state_files:5,config_files:3,quarantined:0},
  checksums: $checksums
}' > "${BUNDLE}/MANIFEST.json"

# Run import
bash "${CORE_ROOT}/scripts/odin/odin-import.sh" "${BUNDLE}"

# Verify files landed
assert_file_exists "skill imported" "${ODIN_DIR}/.claude/skills/odin-test/SKILL.md"
assert_file_exists "workers.yaml" "${ODIN_DIR}/config/workers.yaml"
assert_file_exists "routing.yaml" "${ODIN_DIR}/config/routing.yaml"
assert_file_exists "core-policy.yaml" "${ODIN_DIR}/config/policy/core-policy.yaml"
assert_file_exists "learnings" "${ODIN_DIR}/memory/hot/learnings.md"
assert_file_exists "state.json" "${ODIN_DIR}/state/state.json"
assert_file_exists "data.version" "${ODIN_DIR}/data.version"
assert_eq "data.version is 1" "1" "$(cat "${ODIN_DIR}/data.version")"

# Verify backup was created
BACKUP_COUNT="$(find "${ODIN_DIR}/backups" -maxdepth 1 -name 'pre-import-*' -type d 2>/dev/null | wc -l)"
assert_eq "backup created" "true" "$([[ "${BACKUP_COUNT}" -ge 0 ]] && echo true || echo false)"

echo "import-test: ${PASS} passed, ${FAIL} failed"
[[ "${FAIL}" -eq 0 ]] && echo "import-test: PASS" || { echo "import-test: FAIL"; exit 1; }
