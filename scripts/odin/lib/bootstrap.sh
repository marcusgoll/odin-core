#!/usr/bin/env bash

ODIN_BOOTSTRAP_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ODIN_BOOTSTRAP_ROOT_DIR="$(cd "${ODIN_BOOTSTRAP_LIB_DIR}/../../.." && pwd)"
ODIN_BOOTSTRAP_GUARDRAILS_DEFAULT="${ODIN_BOOTSTRAP_ROOT_DIR}/config/guardrails.yaml"

odin_bootstrap_usage() {
  cat <<'EOF'
Usage:
  odin help
  odin connect <provider> <oauth|api> [--dry-run]
  odin start [--dry-run]
  odin tui [--dry-run]
  odin inbox add "<task>" [--dry-run]
  odin inbox list
  odin gateway add <cli|slack|telegram> [--dry-run]
  odin verify [--dry-run]
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

odin_bootstrap_reject_unknown_flags() {
  local action="$1"
  shift || true

  local arg
  for arg in "$@"; do
    if [[ "${arg}" == -* && "${arg}" != "--dry-run" ]]; then
      odin_bootstrap_err "${action}: unknown flag '${arg}'"
      return 64
    fi
  done

  return 0
}

odin_bootstrap_guardrails_path() {
  if [[ -n "${ODIN_GUARDRAILS_PATH:-}" ]]; then
    echo "${ODIN_GUARDRAILS_PATH}"
    return
  fi
  echo "${ODIN_BOOTSTRAP_GUARDRAILS_DEFAULT}"
}

odin_bootstrap_guardrails_available() {
  [[ -f "$(odin_bootstrap_guardrails_path)" ]]
}

odin_bootstrap_require_guardrails_or_dry_run() {
  local action="$1"
  shift || true

  if odin_bootstrap_has_dry_run "$@"; then
    return 0
  fi

  if odin_bootstrap_guardrails_available; then
    return 0
  fi

  odin_bootstrap_err "BLOCKED ${action}: guardrails file not found at $(odin_bootstrap_guardrails_path). Use --dry-run for a no-op path."
  return 2
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

  if odin_bootstrap_reject_unknown_flags "connect" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_require_guardrails_or_dry_run "connect" "${args[@]}"; then
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
  if odin_bootstrap_reject_unknown_flags "start" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_require_guardrails_or_dry_run "start" "${args[@]}"; then
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
  if odin_bootstrap_reject_unknown_flags "tui" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_require_guardrails_or_dry_run "tui" "${args[@]}"; then
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

  if odin_bootstrap_reject_unknown_flags "inbox add" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_require_guardrails_or_dry_run "inbox add" "${args[@]}"; then
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

  if odin_bootstrap_reject_unknown_flags "gateway add" "${args[@]}"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_require_guardrails_or_dry_run "gateway add" "${args[@]}"; then
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

  if odin_bootstrap_reject_unknown_flags "verify" "${args[@]}"; then
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
  local command="${1:-help}"

  case "${command}" in
    help|-h|--help)
      odin_bootstrap_usage
      ;;
    connect)
      shift
      if [[ $# -lt 2 ]]; then
        odin_bootstrap_err "usage: odin connect <provider> <oauth|api> [--dry-run]"
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
            odin_bootstrap_err "usage: odin inbox add \"<task>\" [--dry-run]"
            return 64
          fi
          odin_bootstrap_cmd_inbox_add "$@"
          ;;
        list)
          shift
          if [[ $# -ne 0 ]]; then
            odin_bootstrap_err "usage: odin inbox list"
            return 64
          fi
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
            odin_bootstrap_err "usage: odin gateway add <cli|slack|telegram> [--dry-run]"
            return 64
          fi
          odin_bootstrap_cmd_gateway_add "$@"
          ;;
        *)
          odin_bootstrap_err "usage: odin gateway add <cli|slack|telegram> [--dry-run]"
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
