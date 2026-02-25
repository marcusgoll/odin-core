#!/usr/bin/env bash

ODIN_BOOTSTRAP_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ODIN_BOOTSTRAP_ROOT_DIR="$(cd "${ODIN_BOOTSTRAP_LIB_DIR}/../../.." && pwd)"
ODIN_BOOTSTRAP_GUARDRAILS_DEFAULT="${ODIN_BOOTSTRAP_ROOT_DIR}/config/guardrails.yaml"
ODIN_BOOTSTRAP_GUARDRAILS_OVERRIDE=""

odin_bootstrap_usage() {
  cat <<'EOF'
Usage:
  odin [--guardrails <path>] help
  odin [--guardrails <path>] connect <provider> <oauth|api> [--dry-run] [--confirm]
  odin [--guardrails <path>] start [--dry-run] [--confirm]
  odin [--guardrails <path>] tui [--dry-run] [--confirm]
  odin [--guardrails <path>] inbox add "<task>" [--dry-run] [--confirm]
  odin [--guardrails <path>] inbox list
  odin [--guardrails <path>] gateway add <cli|slack|telegram> [--dry-run] [--confirm]
  odin [--guardrails <path>] verify [--dry-run]
EOF
}

odin_bootstrap_err() {
  echo "[odin] ERROR: $*" >&2
}

odin_bootstrap_info() {
  echo "[odin] $*"
}

odin_bootstrap_has_flag() {
  local needle="$1"
  shift
  local arg
  for arg in "$@"; do
    if [[ "${arg}" == "${needle}" ]]; then
      return 0
    fi
  done
  return 1
}

odin_bootstrap_has_dry_run() {
  odin_bootstrap_has_flag "--dry-run" "$@"
}

odin_bootstrap_has_confirm() {
  odin_bootstrap_has_flag "--confirm" "$@"
}

odin_bootstrap_validate_action_flags() {
  local action="$1"
  local allow_confirm="$2"
  shift 2 || true

  local dry_run_count=0
  local confirm_count=0
  local arg
  for arg in "$@"; do
    case "${arg}" in
      --dry-run)
        dry_run_count=$((dry_run_count + 1))
        ;;
      --confirm)
        if [[ "${allow_confirm}" != "yes" ]]; then
          odin_bootstrap_err "${action}: unknown flag '${arg}'"
          return 64
        fi
        confirm_count=$((confirm_count + 1))
        ;;
      -*)
        odin_bootstrap_err "${action}: unknown flag '${arg}'"
        return 64
        ;;
      *)
        odin_bootstrap_err "${action}: unexpected argument '${arg}'"
        return 64
        ;;
    esac
  done

  if [[ "${dry_run_count}" -gt 1 ]]; then
    odin_bootstrap_err "${action}: --dry-run may be specified once"
    return 64
  fi

  if [[ "${confirm_count}" -gt 1 ]]; then
    odin_bootstrap_err "${action}: --confirm may be specified once"
    return 64
  fi

  return 0
}

odin_bootstrap_validate_optional_dry_run_only() {
  local action="$1"
  shift || true
  odin_bootstrap_validate_action_flags "${action}" "no" "$@"
}

odin_bootstrap_validate_optional_dry_run_or_confirm() {
  local action="$1"
  shift || true
  odin_bootstrap_validate_action_flags "${action}" "yes" "$@"
}

odin_bootstrap_guardrails_path() {
  if [[ -n "${ODIN_BOOTSTRAP_GUARDRAILS_OVERRIDE:-}" ]]; then
    echo "${ODIN_BOOTSTRAP_GUARDRAILS_OVERRIDE}"
    return
  fi
  if [[ -n "${ODIN_GUARDRAILS_PATH:-}" ]]; then
    echo "${ODIN_GUARDRAILS_PATH}"
    return
  fi
  echo "${ODIN_BOOTSTRAP_GUARDRAILS_DEFAULT}"
}

odin_bootstrap_guardrails_available() {
  [[ -f "$(odin_bootstrap_guardrails_path)" ]]
}

odin_bootstrap_guardrails_acknowledged() {
  [[ "${ODIN_GUARDRAILS_ACK:-}" == "yes" ]]
}

odin_bootstrap_action_category() {
  local action_id="$1"
  case "${action_id}" in
    connect|gateway.add)
      echo "integration"
      ;;
    start|tui|inbox.add)
      echo "mutating"
      ;;
    *)
      echo "readonly"
      ;;
  esac
}

odin_bootstrap_guardrails_list_contains() {
  local key="$1"
  local needle="$2"
  local path
  path="$(odin_bootstrap_guardrails_path)"

  if [[ ! -f "${path}" ]]; then
    return 1
  fi

  awk -v key="${key}" '
    BEGIN {
      in_section = 0
      section_indent = -1
    }
    {
      line = $0
      sub(/\r$/, "", line)
      sub(/[[:space:]]+#.*/, "", line)
      if (line ~ /^[[:space:]]*$/) {
        next
      }

      indent = match(line, /[^ ]/) - 1
      if (indent < 0) {
        indent = 0
      }

      key_pattern = "^[[:space:]]*" key ":[[:space:]]*$"
      if (!in_section && line ~ key_pattern) {
        in_section = 1
        section_indent = indent
        next
      }

      if (in_section && indent <= section_indent && line ~ /^[[:space:]]*[A-Za-z0-9_.-]+:[[:space:]]*.*$/) {
        in_section = 0
      }

      if (in_section && line ~ /^[[:space:]]*-[[:space:]]*.+$/) {
        item = line
        sub(/^[[:space:]]*-[[:space:]]*/, "", item)
        gsub(/[[:space:]]+$/, "", item)
        print item
      }
    }
  ' "${path}" | grep -Fxq "${needle}"
}

odin_bootstrap_action_requires_confirm() {
  local action_id="$1"
  local category
  category="$(odin_bootstrap_action_category "${action_id}")"

  if odin_bootstrap_guardrails_list_contains "confirm_required" "${action_id}"; then
    return 0
  fi

  if odin_bootstrap_guardrails_list_contains "confirm_required" "${category}"; then
    return 0
  fi

  return 1
}

odin_bootstrap_require_guardrails_or_dry_run() {
  local action_id="$1"
  local action_label="$2"
  shift 2 || true
  local args=("$@")

  if odin_bootstrap_has_dry_run "${args[@]}"; then
    return 0
  fi

  if ! odin_bootstrap_guardrails_available; then
    odin_bootstrap_err "BLOCKED ${action_label}: guardrails file not found at $(odin_bootstrap_guardrails_path). Use --dry-run for a no-op path."
    return 2
  fi

  if odin_bootstrap_guardrails_list_contains "denylist" "${action_id}"; then
    odin_bootstrap_err "BLOCKED ${action_label}: denylisted by guardrails at $(odin_bootstrap_guardrails_path)"
    return 2
  fi

  local category
  category="$(odin_bootstrap_action_category "${action_id}")"
  if [[ "${category}" != "readonly" ]] && ! odin_bootstrap_guardrails_acknowledged; then
    odin_bootstrap_err "BLOCKED ${action_label}: acknowledgement required. Set ODIN_GUARDRAILS_ACK=yes to execute mutating actions."
    return 2
  fi

  if odin_bootstrap_action_requires_confirm "${action_id}" && ! odin_bootstrap_has_confirm "${args[@]}"; then
    odin_bootstrap_err "BLOCKED ${action_label}: category '${category}' requires --confirm."
    return 2
  fi

  return 0
}

odin_bootstrap_cmd_connect() {
  local provider="$1"
  local auth_mode="$2"
  shift 2
  local args=("$@")

  if [[ "${auth_mode}" != "oauth" && "${auth_mode}" != "api" ]]; then
    odin_bootstrap_err "connect auth must be oauth or api"
    return 64
  fi

  if odin_bootstrap_validate_optional_dry_run_or_confirm "connect" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_require_guardrails_or_dry_run "connect" "connect" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_has_dry_run "${args[@]}"; then
    odin_bootstrap_info "DRY-RUN connect provider=${provider} auth=${auth_mode}"
    return 0
  fi

  odin_bootstrap_info "connect placeholder provider=${provider} auth=${auth_mode}"
}

odin_bootstrap_cmd_start() {
  local args=("$@")
  if odin_bootstrap_validate_optional_dry_run_or_confirm "start" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_require_guardrails_or_dry_run "start" "start" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_has_dry_run "${args[@]}"; then
    odin_bootstrap_info "DRY-RUN start"
    return 0
  fi

  odin_bootstrap_info "start placeholder"
}

odin_bootstrap_cmd_tui() {
  local args=("$@")
  if odin_bootstrap_validate_optional_dry_run_or_confirm "tui" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_require_guardrails_or_dry_run "tui" "tui" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_has_dry_run "${args[@]}"; then
    odin_bootstrap_info "DRY-RUN tui"
    return 0
  fi

  odin_bootstrap_info "tui placeholder"
}

odin_bootstrap_cmd_inbox_add() {
  local title="$1"
  shift
  local args=("$@")

  if odin_bootstrap_validate_optional_dry_run_or_confirm "inbox add" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_require_guardrails_or_dry_run "inbox.add" "inbox add" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_has_dry_run "${args[@]}"; then
    odin_bootstrap_info "DRY-RUN inbox add title=${title}"
    return 0
  fi

  odin_bootstrap_info "inbox add placeholder title=${title}"
}

odin_bootstrap_cmd_inbox_list() {
  local args=("$@")

  if odin_bootstrap_validate_optional_dry_run_only "inbox list" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if ! odin_bootstrap_guardrails_available; then
    odin_bootstrap_info "guardrails not found; running conservative read-only inbox list"
  fi
  odin_bootstrap_info "inbox list placeholder (empty)"
}

odin_bootstrap_validate_gateway_source() {
  local source="$1"

  case "${source}" in
    cli|slack|telegram)
      return 0
      ;;
    *)
      odin_bootstrap_err "gateway add source must be one of: cli, slack, telegram"
      return 64
      ;;
  esac
}

odin_bootstrap_cmd_gateway_add() {
  local gateway="$1"
  shift
  local args=("$@")

  if odin_bootstrap_validate_gateway_source "${gateway}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_validate_optional_dry_run_or_confirm "gateway add" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_require_guardrails_or_dry_run "gateway.add" "gateway add" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_has_dry_run "${args[@]}"; then
    odin_bootstrap_info "DRY-RUN gateway add source=${gateway}"
    return 0
  fi

  odin_bootstrap_info "gateway add placeholder source=${gateway}"
}

odin_bootstrap_cmd_verify() {
  local args=("$@")

  if odin_bootstrap_validate_optional_dry_run_only "verify" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_has_dry_run "${args[@]}"; then
    odin_bootstrap_info "DRY-RUN verify"
    return 0
  fi

  if odin_bootstrap_guardrails_available; then
    odin_bootstrap_info "verify placeholder guardrails=present"
  else
    odin_bootstrap_info "verify placeholder guardrails=missing (conservative mode)"
  fi
}

odin_bootstrap_dispatch() {
  ODIN_BOOTSTRAP_GUARDRAILS_OVERRIDE=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --guardrails)
        if [[ -n "${ODIN_BOOTSTRAP_GUARDRAILS_OVERRIDE}" ]]; then
          odin_bootstrap_err "--guardrails may be specified once"
          return 64
        fi
        shift
        if [[ $# -eq 0 ]]; then
          odin_bootstrap_err "--guardrails requires a path argument"
          return 64
        fi
        ODIN_BOOTSTRAP_GUARDRAILS_OVERRIDE="$1"
        shift
        ;;
      --guardrails=*)
        if [[ -n "${ODIN_BOOTSTRAP_GUARDRAILS_OVERRIDE}" ]]; then
          odin_bootstrap_err "--guardrails may be specified once"
          return 64
        fi
        ODIN_BOOTSTRAP_GUARDRAILS_OVERRIDE="${1#--guardrails=}"
        if [[ -z "${ODIN_BOOTSTRAP_GUARDRAILS_OVERRIDE}" ]]; then
          odin_bootstrap_err "--guardrails requires a non-empty path argument"
          return 64
        fi
        shift
        ;;
      *)
        break
        ;;
    esac
  done

  local command="${1:-help}"

  case "${command}" in
    help|-h|--help)
      odin_bootstrap_usage
      ;;
    connect)
      shift
      if [[ $# -lt 2 ]]; then
        odin_bootstrap_err "usage: odin [--guardrails <path>] connect <provider> <oauth|api> [--dry-run] [--confirm]"
        return 64
      fi
      odin_bootstrap_cmd_connect "$@"
      ;;
    start)
      shift
      odin_bootstrap_cmd_start "$@"
      ;;
    tui)
      shift
      odin_bootstrap_cmd_tui "$@"
      ;;
    inbox)
      shift
      local subcommand="${1:-}"
      case "${subcommand}" in
        add)
          shift
          if [[ $# -lt 1 ]]; then
            odin_bootstrap_err "usage: odin [--guardrails <path>] inbox add \"<task>\" [--dry-run] [--confirm]"
            return 64
          fi
          odin_bootstrap_cmd_inbox_add "$@"
          ;;
        list)
          shift
          odin_bootstrap_cmd_inbox_list "$@"
          ;;
        *)
          odin_bootstrap_err "usage: odin inbox <add|list> ..."
          return 64
          ;;
      esac
      ;;
    gateway)
      shift
      local subcommand="${1:-}"
      case "${subcommand}" in
        add)
          shift
          if [[ $# -lt 1 ]]; then
            odin_bootstrap_err "usage: odin [--guardrails <path>] gateway add <cli|slack|telegram> [--dry-run] [--confirm]"
            return 64
          fi
          odin_bootstrap_cmd_gateway_add "$@"
          ;;
        *)
          odin_bootstrap_err "usage: odin [--guardrails <path>] gateway add <cli|slack|telegram> [--dry-run] [--confirm]"
          return 64
          ;;
      esac
      ;;
    verify)
      shift
      odin_bootstrap_cmd_verify "$@"
      ;;
    *)
      odin_bootstrap_err "unknown command: ${command}"
      odin_bootstrap_usage >&2
      return 64
      ;;
  esac
}
