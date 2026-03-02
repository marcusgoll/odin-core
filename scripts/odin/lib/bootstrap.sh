#!/usr/bin/env bash

ODIN_BOOTSTRAP_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ODIN_BOOTSTRAP_ROOT_DIR="$(cd "${ODIN_BOOTSTRAP_LIB_DIR}/../../.." && pwd)"
ODIN_BOOTSTRAP_GUARDRAILS_DEFAULT="${ODIN_BOOTSTRAP_ROOT_DIR}/config/guardrails.yaml"
ODIN_BOOTSTRAP_GUARDRAILS_OVERRIDE=""
ODIN_BOOTSTRAP_GUARDRAILS_LAST_ERROR=""
ODIN_BOOTSTRAP_MODE_STATE_LAST_ERROR=""
ODIN_BOOTSTRAP_MODE_STATE_LIB="${ODIN_BOOTSTRAP_LIB_DIR}/mode_state.sh"

if [[ -f "${ODIN_BOOTSTRAP_MODE_STATE_LIB}" ]]; then
  # shellcheck source=/dev/null
  source "${ODIN_BOOTSTRAP_MODE_STATE_LIB}"
fi

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

Operations:
  odin status                Show dispatch loop and agent session status
  odin stop                  Stop dispatch loop and agent sessions
  odin restart               Stop and restart odin.service via systemctl
  odin logs                  Tail today's dispatch log
  odin agents                List active agent tmux sessions
  odin send                  Send JSON task from stdin to inbox
  odin test                  Run verify smoke tests
EOF
}

odin_bootstrap_err() {
  echo "[odin] ERROR: $*" >&2
}

odin_bootstrap_info() {
  echo "[odin] $*"
}

odin_bootstrap_now_utc() {
  date -u +"%Y-%m-%dT%H:%M:%SZ"
}

odin_bootstrap_print_inbox_normalized_fields() {
  local title="$1"
  local timestamp
  timestamp="$(odin_bootstrap_now_utc)"
  odin_bootstrap_info "normalized inbox item title=${title} raw_text=${title} source=cli timestamp=${timestamp}"
}

odin_bootstrap_mode_state_available() {
  declare -F odin_mode_state_init >/dev/null 2>&1
}

odin_bootstrap_mode_state_format_reason() {
  local raw="$1"
  raw="${raw//$'\r'/ }"
  raw="$(printf '%s' "${raw}" | tr '\n' ' ' | sed -E 's/[[:space:]]+/ /g; s/^ //; s/ $//')"
  printf '%s' "${raw}"
}

odin_bootstrap_mode_state_call() {
  ODIN_BOOTSTRAP_MODE_STATE_LAST_ERROR=""
  local mode_state_err=""
  local rc=0
  if mode_state_err="$({ "$@" >/dev/null; } 2>&1)"; then
    return 0
  else
    rc=$?
  fi

  ODIN_BOOTSTRAP_MODE_STATE_LAST_ERROR="$(odin_bootstrap_mode_state_format_reason "${mode_state_err}")"
  return "${rc}"
}

odin_bootstrap_mode_state_reason_suffix() {
  if [[ -z "${ODIN_BOOTSTRAP_MODE_STATE_LAST_ERROR:-}" ]]; then
    return 0
  fi
  printf ' (%s)' "${ODIN_BOOTSTRAP_MODE_STATE_LAST_ERROR}"
}

odin_bootstrap_mode_state_init() {
  if ! odin_bootstrap_mode_state_available; then
    return 0
  fi
  odin_bootstrap_mode_state_call odin_mode_state_init
}

odin_bootstrap_mode_state_record_event() {
  local event="$1"
  if ! odin_bootstrap_mode_state_available; then
    return 0
  fi
  odin_bootstrap_mode_state_call odin_mode_state_record_event "${event}"
}

odin_bootstrap_mode_state_set_mode_if_allowed() {
  local mode="$1"
  if ! odin_bootstrap_mode_state_available; then
    return 1
  fi
  odin_bootstrap_mode_state_call odin_mode_state_set_mode "${mode}"
}

odin_bootstrap_mode_state_require_init() {
  if odin_bootstrap_mode_state_init; then
    return 0
  fi
  odin_bootstrap_err "mode state initialization failed$(odin_bootstrap_mode_state_reason_suffix)"
  return 70
}

odin_bootstrap_mode_state_require_event() {
  local event="$1"
  if odin_bootstrap_mode_state_record_event "${event}"; then
    return 0
  fi
  odin_bootstrap_err "mode state update failed for event '${event}'$(odin_bootstrap_mode_state_reason_suffix)"
  return 70
}

odin_bootstrap_mode_state_require_mode() {
  local mode="$1"
  local rc=0
  if odin_bootstrap_mode_state_set_mode_if_allowed "${mode}"; then
    return 0
  else
    rc=$?
  fi

  if [[ "${rc}" -eq 2 ]]; then
    odin_bootstrap_err "BLOCKED mode transition to ${mode}: confidence/guardrails/task-cycle gate not satisfied."
    return 2
  fi

  odin_bootstrap_err "mode transition to ${mode} failed$(odin_bootstrap_mode_state_reason_suffix)"
  return 70
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
      sub(/^[[:space:]]*#.*/, "", line)
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

  if odin_bootstrap_mode_state_require_event "guardrails.acknowledged.verified"; then
    :
  else
    local rc=$?
    return "${rc}"
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
  if odin_bootstrap_mode_state_require_event "provider.connected.verified"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi
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
  if odin_bootstrap_mode_state_require_event "tui.opened.verified"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi
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
    odin_bootstrap_print_inbox_normalized_fields "${title}"
    return 0
  fi

  odin_bootstrap_info "inbox add placeholder title=${title}"
  odin_bootstrap_print_inbox_normalized_fields "${title}"
  if odin_bootstrap_mode_state_require_event "inbox.first_item.verified"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi
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

  if ! odin_bootstrap_guardrails_available; then
    if odin_bootstrap_mode_state_require_event "verify.failed"; then
      :
    else
      local rc=$?
      return "${rc}"
    fi
    odin_bootstrap_err "verify failed: guardrails file not found at $(odin_bootstrap_guardrails_path); mode set to RECOVERY."
    return 2
  fi

  if ! odin_bootstrap_guardrails_acknowledged; then
    if odin_bootstrap_mode_state_require_event "verify.failed"; then
      :
    else
      local rc=$?
      return "${rc}"
    fi
    odin_bootstrap_err "verify failed: acknowledgement required; mode set to RECOVERY."
    return 2
  fi

  if odin_bootstrap_mode_state_require_event "task.cycle.verified"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_mode_state_require_event "verify.passed.verified"; then
    :
  else
    local rc=$?
    return "${rc}"
  fi

  if odin_bootstrap_mode_state_require_mode "OPERATE"; then
    odin_bootstrap_info "verify placeholder guardrails=present mode=OPERATE task_cycle=verified"
  else
    local rc=$?
    return "${rc}"
  fi
}

odin_bootstrap_cmd_status() {
  local odin_dir="${ODIN_DIR:-/var/odin}"
  local pids
  pids="$(pgrep -f 'odin-dispatch\.sh' 2>/dev/null || true)"

  if [[ -n "${pids}" ]]; then
    odin_bootstrap_info "dispatch loop RUNNING (pids: $(echo ${pids} | tr '\n' ' '))"
  else
    odin_bootstrap_info "dispatch loop STOPPED"
  fi

  local heartbeat_file="${odin_dir}/heartbeat"
  if [[ -f "${heartbeat_file}" ]]; then
    local heartbeat
    heartbeat="$(cat "${heartbeat_file}" 2>/dev/null || true)"
    odin_bootstrap_info "heartbeat: ${heartbeat:-unknown}"
  else
    odin_bootstrap_info "heartbeat: no file"
  fi

  local sessions
  sessions="$(tmux list-sessions 2>/dev/null | grep '^odin-' || true)"
  if [[ -n "${sessions}" ]]; then
    odin_bootstrap_info "agent sessions:"
    while IFS= read -r line; do
      odin_bootstrap_info "  ${line}"
    done <<< "${sessions}"
  else
    odin_bootstrap_info "agent sessions: (none)"
  fi
}

odin_bootstrap_cmd_stop() {
  local pids
  pids="$(pgrep -f 'odin-dispatch\.sh' 2>/dev/null || true)"

  if [[ -z "${pids}" ]]; then
    odin_bootstrap_info "dispatch loop not running"
  else
    odin_bootstrap_info "stopping dispatch loop (pids: $(echo ${pids} | tr '\n' ' '))..."
    local p
    while IFS= read -r p; do
      kill "${p}" 2>/dev/null || true
    done <<< "${pids}"
    local waited=0
    local still_running=true
    while "${still_running}" && [[ "${waited}" -lt 5 ]]; do
      still_running=false
      while IFS= read -r p; do
        if kill -0 "${p}" 2>/dev/null; then
          still_running=true
        fi
      done <<< "${pids}"
      if "${still_running}"; then
        sleep 1
        waited=$((waited + 1))
      fi
    done
    while IFS= read -r p; do
      if kill -0 "${p}" 2>/dev/null; then
        odin_bootstrap_info "SIGKILL dispatch (pid=${p})"
        kill -9 "${p}" 2>/dev/null || true
      fi
    done <<< "${pids}"
    odin_bootstrap_info "dispatch loop stopped"
  fi

  local sessions
  sessions="$(tmux list-sessions 2>/dev/null | grep '^odin-' | cut -d: -f1 || true)"
  if [[ -n "${sessions}" ]]; then
    while IFS= read -r sess; do
      odin_bootstrap_info "killing tmux session: ${sess}"
      tmux kill-session -t "${sess}" 2>/dev/null || true
    done <<< "${sessions}"
  fi
}

odin_bootstrap_cmd_restart() {
  odin_bootstrap_cmd_stop
  sleep 1
  odin_bootstrap_info "restarting odin.service..."
  if ! sudo systemctl restart odin.service; then
    odin_bootstrap_err "systemctl restart odin.service failed"
    return 1
  fi
  odin_bootstrap_info "odin.service restarted"
}

odin_bootstrap_cmd_logs() {
  local odin_dir="${ODIN_DIR:-/var/odin}"
  local log_file="${odin_dir}/logs/$(date +%Y-%m-%d)/dispatch.log"
  if [[ ! -f "${log_file}" ]]; then
    odin_bootstrap_err "log file not found: ${log_file}"
    return 1
  fi
  exec tail -f "${log_file}"
}

odin_bootstrap_cmd_agents() {
  local sessions
  sessions="$(tmux list-sessions 2>/dev/null | grep '^odin-' || true)"
  if [[ -n "${sessions}" ]]; then
    echo "${sessions}"
  else
    odin_bootstrap_info "(no agent sessions)"
  fi
}

odin_bootstrap_cmd_send() {
  local odin_dir="${ODIN_DIR:-/var/odin}"
  local inbox_dir="${odin_dir}/inbox"

  if [[ -t 0 ]]; then
    odin_bootstrap_err "send: expected JSON on stdin"
    return 64
  fi

  local payload
  payload="$(cat)"

  if [[ -z "${payload}" ]]; then
    odin_bootstrap_err "send: empty input"
    return 64
  fi

  if command -v jq >/dev/null 2>&1; then
    if ! echo "${payload}" | jq empty 2>/dev/null; then
      odin_bootstrap_err "send: invalid JSON"
      return 65
    fi
  fi

  mkdir -p "${inbox_dir}"
  local filename="manual-$(date +%s)-$$.json"
  local tmp_file="${inbox_dir}/.${filename}.tmp"
  printf '%s\n' "${payload}" > "${tmp_file}"
  mv "${tmp_file}" "${inbox_dir}/${filename}"
  odin_bootstrap_info "sent → ${inbox_dir}/${filename}"
}

odin_bootstrap_cmd_test() {
  local script_dir="${ODIN_BOOTSTRAP_ROOT_DIR}/scripts/verify"
  local failed=0

  for test_script in quickstart-smoke.sh bootstrap-wrapper-smoke.sh odin-ops-cli-smoke.sh; do
    local path="${script_dir}/${test_script}"
    if [[ ! -f "${path}" ]]; then
      odin_bootstrap_info "SKIP ${test_script} (not found)"
      continue
    fi
    odin_bootstrap_info "RUN ${test_script}"
    if bash "${path}"; then
      odin_bootstrap_info "PASS ${test_script}"
    else
      odin_bootstrap_err "FAIL ${test_script}"
      failed=$((failed + 1))
    fi
  done

  if [[ "${failed}" -gt 0 ]]; then
    odin_bootstrap_err "${failed} test(s) failed"
    return 1
  fi
  odin_bootstrap_info "all tests passed"
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
  if [[ "${command}" != "help" && "${command}" != "-h" && "${command}" != "--help" ]]; then
    if odin_bootstrap_mode_state_require_init; then
      :
    else
      local rc=$?
      return "${rc}"
    fi
  fi

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
    status|stop|restart|logs|agents|send|test)
      local ops_cmd="${command}"
      shift
      if [[ $# -gt 0 && "${ops_cmd}" != "send" ]]; then
        odin_bootstrap_err "${ops_cmd}: unexpected arguments"
        return 64
      fi
      "odin_bootstrap_cmd_${ops_cmd}"
      ;;
    *)
      odin_bootstrap_err "unknown command: ${command}"
      odin_bootstrap_usage >&2
      return 64
      ;;
  esac
}
