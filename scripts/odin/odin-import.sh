#!/usr/bin/env bash
# odin-import.sh — Import a migration bundle into $ODIN_DIR
set -euo pipefail

ODIN_DIR="${ODIN_DIR:-/var/odin}"

import_log() { printf '[odin-import] %s\n' "$*" >&2; }

sha256_file() { sha256sum "$1" | awk '{print $1}'; }

main() {
  local bundle="$1"
  local force_flag="${2:-}"

  [[ -d "${bundle}" ]] || { import_log "error: bundle not found: ${bundle}"; exit 1; }
  [[ -f "${bundle}/MANIFEST.json" ]] || { import_log "error: MANIFEST.json not found"; exit 1; }

  local manifest
  manifest="$(cat "${bundle}/MANIFEST.json")"
  local schema_version
  schema_version="$(echo "${manifest}" | jq -r '.schema_version')"
  [[ "${schema_version}" == "1" ]] || { import_log "error: unsupported schema_version: ${schema_version}"; exit 1; }

  # 1. Verify all checksums in bundle before importing
  import_log "verifying bundle checksums..."
  local failures=0
  while IFS= read -r relpath; do
    [[ -z "${relpath}" ]] && continue
    local expected actual
    expected="$(echo "${manifest}" | jq -r --arg k "${relpath}" '.checksums[$k]')"
    expected="${expected#sha256:}"
    if [[ -f "${bundle}/${relpath}" ]]; then
      actual="$(sha256_file "${bundle}/${relpath}")"
      if [[ "${expected}" != "${actual}" ]]; then
        import_log "CHECKSUM FAIL: ${relpath}"
        failures=$((failures + 1))
      fi
    else
      import_log "MISSING: ${relpath}"
      failures=$((failures + 1))
    fi
  done <<< "$(echo "${manifest}" | jq -r '.checksums | keys[]')"

  if [[ "${failures}" -gt 0 ]]; then
    import_log "aborting: ${failures} checksum failures in bundle"
    exit 1
  fi
  import_log "all checksums verified"

  # 2. Check for existing data
  if [[ -f "${ODIN_DIR}/data.version" ]]; then
    if [[ "${force_flag}" != "--force" && "${force_flag}" != "--merge" ]]; then
      import_log "error: ${ODIN_DIR} already has data (data.version exists). Use --force or --merge."
      exit 1
    fi
  fi

  # 3. Backup existing data
  local backup_dir="${ODIN_DIR}/backups/pre-import-$(date +%Y%m%dT%H%M%S)"
  mkdir -p "${backup_dir}"
  for d in .claude/skills config memory state; do
    [[ -d "${ODIN_DIR}/${d}" ]] && cp -a "${ODIN_DIR}/${d}" "${backup_dir}/" 2>/dev/null || true
  done
  [[ -f "${ODIN_DIR}/data.version" ]] && cp "${ODIN_DIR}/data.version" "${backup_dir}/"
  import_log "backup written to ${backup_dir}"

  # 4. Copy skills
  if [[ -d "${bundle}/skills" ]]; then
    mkdir -p "${ODIN_DIR}/.claude/skills"
    cp -a "${bundle}/skills/." "${ODIN_DIR}/.claude/skills/"
    import_log "imported skills → ${ODIN_DIR}/.claude/skills/"
  fi

  # 5. Copy config
  if [[ -d "${bundle}/config" ]]; then
    mkdir -p "${ODIN_DIR}/config/policy"
    cp -a "${bundle}/config/." "${ODIN_DIR}/config/"
    import_log "imported config → ${ODIN_DIR}/config/"
  fi

  # 6. Copy memory
  if [[ -d "${bundle}/memory" ]]; then
    mkdir -p "${ODIN_DIR}/memory/hot" "${ODIN_DIR}/memory/cold"
    cp -a "${bundle}/memory/." "${ODIN_DIR}/memory/"
    import_log "imported memory → ${ODIN_DIR}/memory/"
  fi

  # 7. Copy state
  if [[ -d "${bundle}/state" ]]; then
    mkdir -p "${ODIN_DIR}/state/kanban" "${ODIN_DIR}/state/budgets" "${ODIN_DIR}/state/autonomy/contracts"
    cp -a "${bundle}/state/." "${ODIN_DIR}/state/"
    import_log "imported state → ${ODIN_DIR}/state/"
  fi

  # 8. Write data.version
  echo "1" > "${ODIN_DIR}/data.version"
  import_log "wrote data.version = 1"

  # 9. Post-import verification
  import_log "verifying import..."
  local verify_failures=0
  while IFS= read -r relpath; do
    [[ -z "${relpath}" ]] && continue
    local expected target_path

    # Map bundle paths to ODIN_DIR paths
    case "${relpath}" in
      skills/*)  target_path="${ODIN_DIR}/.claude/${relpath}" ;;
      config/*)  target_path="${ODIN_DIR}/${relpath}" ;;
      memory/*)  target_path="${ODIN_DIR}/${relpath}" ;;
      state/*)   target_path="${ODIN_DIR}/${relpath}" ;;
      *)         continue ;;  # skip quarantine, MANIFEST
    esac

    expected="$(echo "${manifest}" | jq -r --arg k "${relpath}" '.checksums[$k]')"
    expected="${expected#sha256:}"
    if [[ -f "${target_path}" ]]; then
      local actual
      actual="$(sha256_file "${target_path}")"
      if [[ "${expected}" != "${actual}" ]]; then
        import_log "POST-IMPORT MISMATCH: ${relpath} → ${target_path}"
        verify_failures=$((verify_failures + 1))
      fi
    else
      import_log "POST-IMPORT MISSING: ${target_path}"
      verify_failures=$((verify_failures + 1))
    fi
  done <<< "$(echo "${manifest}" | jq -r '.checksums | keys[]')"

  if [[ "${verify_failures}" -gt 0 ]]; then
    import_log "IMPORT FAILED: ${verify_failures} verification failures. Restoring backup..."
    # Restore from backup
    for d in .claude/skills config memory state; do
      [[ -d "${backup_dir}/${d}" ]] && cp -a "${backup_dir}/${d}" "${ODIN_DIR}/"
    done
    [[ -f "${backup_dir}/data.version" ]] && cp "${backup_dir}/data.version" "${ODIN_DIR}/"
    import_log "restored from backup. Import aborted."
    exit 1
  fi

  # 10. Summary
  local skill_count config_count memory_count state_count
  skill_count="$(find "${ODIN_DIR}/.claude/skills" -name SKILL.md 2>/dev/null | wc -l)"
  config_count="$(find "${ODIN_DIR}/config" -type f 2>/dev/null | wc -l)"
  memory_count="$(find "${ODIN_DIR}/memory" -type f 2>/dev/null | wc -l)"
  state_count="$(find "${ODIN_DIR}/state" -type f 2>/dev/null | wc -l)"
  import_log "import complete: ${skill_count} skills, ${config_count} configs, ${memory_count} memory files, ${state_count} state files"
}

main "$@"
