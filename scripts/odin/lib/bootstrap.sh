#!/usr/bin/env bash

ODIN_BOOTSTRAP_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ODIN_BOOTSTRAP_ROOT_DIR="$(cd "${ODIN_BOOTSTRAP_LIB_DIR}/../../.." && pwd)"
ODIN_BOOTSTRAP_GUARDRAILS_DEFAULT="${ODIN_BOOTSTRAP_ROOT_DIR}/config/guardrails.yaml"
ODIN_BOOTSTRAP_GUARDRAILS_OVERRIDE=""
ODIN_BOOTSTRAP_GUARDRAILS_LAST_ERROR=""

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
  ODIN_BOOTSTRAP_GUARDRAILS_LAST_ERROR=""

  if [[ ! -f "${path}" ]]; then
    return 1
  fi

  if [[ ! -r "${path}" ]]; then
    ODIN_BOOTSTRAP_GUARDRAILS_LAST_ERROR="file is not readable"
    return 2
  fi

  local rc
  if awk -v key="${key}" -v needle="${needle}" '
    function ltrim(s) {
      sub(/^[[:space:]]+/, "", s)
      return s
    }
    function rtrim(s) {
      sub(/[[:space:]]+$/, "", s)
      return s
    }
    function trim(s) {
      return rtrim(ltrim(s))
    }
    function mentions_key_token(token, expected, pattern) {
      pattern = "(^|[^A-Za-z0-9_.-])" expected "([^A-Za-z0-9_.-]|$)"
      return (token ~ pattern)
    }
    function normalize_item(raw, single, item, first, last) {
      single = sprintf("%c", 39)
      item = trim(raw)
      if (item == "") {
        parse_error = 1
        return ""
      }
      first = substr(item, 1, 1)
      last = substr(item, length(item), 1)
      if ((first == "\"" && last == "\"") || (first == single && last == single)) {
        item = substr(item, 2, length(item) - 2)
      } else if (first == "\"" || first == single || last == "\"" || last == single) {
        parse_error = 1
        return ""
      }
      item = trim(item)
      if (item == "") {
        parse_error = 1
        return ""
      }
      if (item ~ /^[\[\{]/ || item ~ /[\]\}]$/) {
        parse_error = 1
        return ""
      }
      return item
    }
    function parse_flow_list(rest, inner, count, i, token) {
      if (rest !~ /^\[.*\]$/) {
        parse_error = 1
        return
      }
      inner = rest
      sub(/^\[/, "", inner)
      sub(/\]$/, "", inner)
      inner = trim(inner)
      if (inner == "") {
        return
      }
      count = split(inner, flow_parts, /,/)
      for (i = 1; i <= count; i++) {
        token = normalize_item(flow_parts[i])
        if (parse_error) {
          return
        }
        if (token == needle) {
          found = 1
        }
      }
    }
    function parse_key_declaration(line, expected, key_part, rest, single, first, last, inner) {
      single = sprintf("%c", 39)
      key_part = line
      sub(/:.*/, "", key_part)
      key_part = trim(key_part)

      rest = line
      sub(/^[^:]*:[[:space:]]*/, "", rest)
      parsed_rest = rest

      if (key_part == expected) {
        return 1
      }

      first = substr(key_part, 1, 1)
      last = substr(key_part, length(key_part), 1)
      if ((first == "\"" && last == "\"") || (first == single && last == single)) {
        inner = substr(key_part, 2, length(key_part) - 2)
        inner = trim(inner)
        if (inner == expected) {
          return 1
        }
        if (mentions_key_token(inner, expected)) {
          parse_error = 1
        }
        return 0
      }

      if (mentions_key_token(key_part, expected)) {
        parse_error = 1
      }
      return 0
    }
    BEGIN {
      parse_error = 0
      found = 0
      in_section = 0
      section_indent = -1
      parsed_rest = ""
    }
    {
      raw = $0
      if (NR == 1) {
        sub(/^\xef\xbb\xbf/, "", raw)
      }
      sub(/\r$/, "", raw)
      line = raw
      sub(/[[:space:]]+#.*/, "", line)
      if (line ~ /^[[:space:]]*$/) {
        next
      }

      indent = match(line, /[^ ]/) - 1
      if (indent < 0) {
        indent = 0
      }

      if (in_section) {
        if (line ~ /^[[:space:]]*-[[:space:]]*.+$/) {
          item = line
          sub(/^[[:space:]]*-[[:space:]]*/, "", item)
          item = normalize_item(item)
          if (parse_error) {
            next
          }
          if (item == needle) {
            found = 1
          }
          next
        }

        if (indent <= section_indent && line ~ /^[[:space:]]*[^:]+:[[:space:]]*.*$/) {
          in_section = 0
        } else {
          parse_error = 1
          next
        }
      }

      if (line ~ /^[[:space:]]*[^:]+:[[:space:]]*.*$/ && parse_key_declaration(line, key)) {
        rest = parsed_rest
        rest = trim(rest)
        if (rest == "") {
          in_section = 1
          section_indent = indent
          next
        }
        parse_flow_list(rest)
        next
      }
    }
    END {
      if (parse_error) {
        exit 2
      }
      if (found) {
        exit 0
      }
      exit 1
    }
  ' "${path}"; then
    rc=0
  else
    rc=$?
  fi
  case "${rc}" in
    0|1)
      return "${rc}"
      ;;
    *)
      ODIN_BOOTSTRAP_GUARDRAILS_LAST_ERROR="unsupported '${key}' policy syntax"
      return 2
      ;;
  esac
}

odin_bootstrap_guardrails_policy_error() {
  local action_label="$1"
  local reason="${ODIN_BOOTSTRAP_GUARDRAILS_LAST_ERROR:-policy could not be parsed}"
  odin_bootstrap_err "BLOCKED ${action_label}: guardrails policy unreadable or unsupported at $(odin_bootstrap_guardrails_path) (${reason})"
}

odin_bootstrap_action_requires_confirm() {
  local action_id="$1"
  local category
  category="$(odin_bootstrap_action_category "${action_id}")"

  local rc
  if odin_bootstrap_guardrails_list_contains "confirm_required" "${action_id}"; then
    rc=0
  else
    rc=$?
  fi
  case "${rc}" in
    0)
      return 0
      ;;
    2)
      return 2
      ;;
  esac

  if odin_bootstrap_guardrails_list_contains "confirm_required" "${category}"; then
    rc=0
  else
    rc=$?
  fi
  case "${rc}" in
    0)
      return 0
      ;;
    2)
      return 2
      ;;
  esac

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

  local deny_rc
  if odin_bootstrap_guardrails_list_contains "denylist" "${action_id}"; then
    deny_rc=0
  else
    deny_rc=$?
  fi
  if [[ "${deny_rc}" -eq 0 ]]; then
    odin_bootstrap_err "BLOCKED ${action_label}: denylisted by guardrails at $(odin_bootstrap_guardrails_path)"
    return 2
  fi
  if [[ "${deny_rc}" -eq 2 ]]; then
    odin_bootstrap_guardrails_policy_error "${action_label}"
    return 2
  fi

  local category
  category="$(odin_bootstrap_action_category "${action_id}")"
  if [[ "${category}" != "readonly" ]] && ! odin_bootstrap_guardrails_acknowledged; then
    odin_bootstrap_err "BLOCKED ${action_label}: acknowledgement required. Set ODIN_GUARDRAILS_ACK=yes to execute integration or mutating actions."
    return 2
  fi

  local confirm_rc
  if odin_bootstrap_action_requires_confirm "${action_id}"; then
    confirm_rc=0
  else
    confirm_rc=$?
  fi
  if [[ "${confirm_rc}" -eq 2 ]]; then
    odin_bootstrap_guardrails_policy_error "${action_label}"
    return 2
  fi
  if [[ "${confirm_rc}" -eq 0 ]] && ! odin_bootstrap_has_confirm "${args[@]}"; then
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
