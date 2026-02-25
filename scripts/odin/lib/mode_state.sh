#!/usr/bin/env bash

# Deterministic persisted state format:
# {
#   "mode": "BOOTSTRAP|OPERATE|RECOVERY",
#   "confidence": <int>,
#   "guardrails_acknowledged": true|false,
#   "task_cycle_verified": true|false,
#   "last_verify_passed": true|false,
#   "recorded_events": "comma,separated,event,list"
# }
ODIN_MODE_STATE_DEFAULT_PATH="${ODIN_MODE_STATE_DEFAULT_PATH:-/var/odin/bootstrap-state.json}"
ODIN_MODE_STATE_FALLBACK_PATH="${ODIN_MODE_STATE_FALLBACK_PATH:-/tmp/odin/bootstrap-state.json}"
ODIN_MODE_STATE_RESOLVED_PATH="${ODIN_MODE_STATE_RESOLVED_PATH:-}"
ODIN_MODE_STATE_LOCK_MAX_WAIT_MS="${ODIN_MODE_STATE_LOCK_MAX_WAIT_MS:-5000}"

_ODIN_MODE_STATE_MODE="BOOTSTRAP"
_ODIN_MODE_STATE_CONFIDENCE=10
_ODIN_MODE_STATE_GUARDRAILS_ACK="false"
_ODIN_MODE_STATE_TASK_CYCLE="false"
_ODIN_MODE_STATE_LAST_VERIFY="true"
_ODIN_MODE_STATE_RECORDED_EVENTS=""

_odin_mode_state_err() {
  echo "[odin] ERROR: mode state: $*" >&2
}

_odin_mode_state_with_lock() {
  local path="$1"
  shift
  local lock_path="${path}.lock"

  if command -v flock >/dev/null 2>&1; then
    local lock_fd
    if ! exec {lock_fd}>"${lock_path}"; then
      _odin_mode_state_err "unable to open lock file ${lock_path}"
      return 1
    fi
    if ! flock -x "${lock_fd}"; then
      _odin_mode_state_err "unable to acquire lock ${lock_path}"
      eval "exec ${lock_fd}>&-"
      return 1
    fi

    "$@"
    local rc=$?
    flock -u "${lock_fd}" || true
    eval "exec ${lock_fd}>&-"
    return "${rc}"
  fi

  local lock_dir="${lock_path}.d"
  local waited_ms=0
  while ! mkdir "${lock_dir}" 2>/dev/null; do
    sleep 0.05
    waited_ms=$((waited_ms + 50))
    if (( waited_ms >= ODIN_MODE_STATE_LOCK_MAX_WAIT_MS )); then
      _odin_mode_state_err "timed out acquiring lock ${lock_dir}"
      return 1
    fi
  done

  "$@"
  local rc=$?
  rmdir "${lock_dir}" 2>/dev/null || true
  return "${rc}"
}

odin_mode_state_path() {
  if [[ -n "${ODIN_MODE_STATE_PATH:-}" ]]; then
    echo "${ODIN_MODE_STATE_PATH}"
    return 0
  fi

  if [[ -n "${ODIN_MODE_STATE_RESOLVED_PATH:-}" ]]; then
    echo "${ODIN_MODE_STATE_RESOLVED_PATH}"
    return 0
  fi

  if mkdir -p "$(dirname "${ODIN_MODE_STATE_DEFAULT_PATH}")" 2>/dev/null; then
    ODIN_MODE_STATE_RESOLVED_PATH="${ODIN_MODE_STATE_DEFAULT_PATH}"
  else
    ODIN_MODE_STATE_RESOLVED_PATH="${ODIN_MODE_STATE_FALLBACK_PATH}"
    if ! mkdir -p "$(dirname "${ODIN_MODE_STATE_RESOLVED_PATH}")" 2>/dev/null; then
      _odin_mode_state_err "unable to create state directory for ${ODIN_MODE_STATE_RESOLVED_PATH}"
      return 1
    fi
  fi

  echo "${ODIN_MODE_STATE_RESOLVED_PATH}"
}

_odin_mode_state_defaults() {
  _ODIN_MODE_STATE_MODE="BOOTSTRAP"
  _ODIN_MODE_STATE_CONFIDENCE=10
  _ODIN_MODE_STATE_GUARDRAILS_ACK="false"
  _ODIN_MODE_STATE_TASK_CYCLE="false"
  _ODIN_MODE_STATE_LAST_VERIFY="true"
  _ODIN_MODE_STATE_RECORDED_EVENTS=""
}

_odin_mode_state_validate_mode() {
  local mode="$1"
  case "${mode}" in
    BOOTSTRAP|OPERATE|RECOVERY)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

_odin_mode_state_read_string() {
  local path="$1"
  local field="$2"
  awk -v field="${field}" '
    match($0, "\"" field "\"[[:space:]]*:[[:space:]]*\"([^\"]*)\"", m) {
      print m[1]
      found = 1
      exit
    }
    END {
      if (!found) {
        exit 1
      }
    }
  ' "${path}"
}

_odin_mode_state_read_int() {
  local path="$1"
  local field="$2"
  awk -v field="${field}" '
    match($0, "\"" field "\"[[:space:]]*:[[:space:]]*([0-9]+)", m) {
      print m[1]
      found = 1
      exit
    }
    END {
      if (!found) {
        exit 1
      }
    }
  ' "${path}"
}

_odin_mode_state_read_bool() {
  local path="$1"
  local field="$2"
  awk -v field="${field}" '
    match($0, "\"" field "\"[[:space:]]*:[[:space:]]*(true|false)", m) {
      print m[1]
      found = 1
      exit
    }
    END {
      if (!found) {
        exit 1
      }
    }
  ' "${path}"
}

_odin_mode_state_load() {
  local path="$1"
  if [[ ! -f "${path}" ]]; then
    return 1
  fi

  local mode
  local confidence
  local guardrails_ack
  local task_cycle
  local last_verify
  local recorded_events

  mode="$(_odin_mode_state_read_string "${path}" "mode" 2>/dev/null)" || return 2
  confidence="$(_odin_mode_state_read_int "${path}" "confidence" 2>/dev/null)" || return 2
  guardrails_ack="$(_odin_mode_state_read_bool "${path}" "guardrails_acknowledged" 2>/dev/null)" || return 2
  task_cycle="$(_odin_mode_state_read_bool "${path}" "task_cycle_verified" 2>/dev/null)" || return 2
  last_verify="$(_odin_mode_state_read_bool "${path}" "last_verify_passed" 2>/dev/null)" || return 2
  recorded_events="$(_odin_mode_state_read_string "${path}" "recorded_events" 2>/dev/null)" || return 2

  _odin_mode_state_validate_mode "${mode}" || return 2

  _ODIN_MODE_STATE_MODE="${mode}"
  _ODIN_MODE_STATE_CONFIDENCE="${confidence}"
  _ODIN_MODE_STATE_GUARDRAILS_ACK="${guardrails_ack}"
  _ODIN_MODE_STATE_TASK_CYCLE="${task_cycle}"
  _ODIN_MODE_STATE_LAST_VERIFY="${last_verify}"
  _ODIN_MODE_STATE_RECORDED_EVENTS="${recorded_events}"
  return 0
}

_odin_mode_state_write() {
  local path="$1"
  local tmp_path="${path}.tmp.$$"
  if ! mkdir -p "$(dirname "${path}")"; then
    _odin_mode_state_err "unable to create state directory for ${path}"
    return 1
  fi

  if ! cat >"${tmp_path}" <<EOF
{
  "mode": "${_ODIN_MODE_STATE_MODE}",
  "confidence": ${_ODIN_MODE_STATE_CONFIDENCE},
  "guardrails_acknowledged": ${_ODIN_MODE_STATE_GUARDRAILS_ACK},
  "task_cycle_verified": ${_ODIN_MODE_STATE_TASK_CYCLE},
  "last_verify_passed": ${_ODIN_MODE_STATE_LAST_VERIFY},
  "recorded_events": "${_ODIN_MODE_STATE_RECORDED_EVENTS}"
}
EOF
  then
    rm -f "${tmp_path}" 2>/dev/null || true
    _odin_mode_state_err "unable to write temporary state file ${tmp_path}"
    return 1
  fi

  if ! mv "${tmp_path}" "${path}"; then
    rm -f "${tmp_path}" 2>/dev/null || true
    _odin_mode_state_err "unable to move temporary state file into ${path}"
    return 1
  fi
}

_odin_mode_state_event_seen() {
  local events="$1"
  local event="$2"
  [[ ",${events}," == *",${event},"* ]]
}

_odin_mode_state_event_append() {
  local events="$1"
  local event="$2"

  if _odin_mode_state_event_seen "${events}" "${event}"; then
    echo "${events}"
    return 0
  fi

  if [[ -z "${events}" ]]; then
    echo "${event}"
  else
    echo "${events},${event}"
  fi
}

_odin_mode_state_init_unlocked() {
  local path="$1"
  if _odin_mode_state_load "${path}"; then
    return 0
  fi

  _odin_mode_state_defaults
  _odin_mode_state_write "${path}"
}

odin_mode_state_init() {
  local path
  path="$(odin_mode_state_path)" || return 1
  _odin_mode_state_with_lock "${path}" _odin_mode_state_init_unlocked "${path}"
}

odin_mode_state_get() {
  local field="$1"
  local path
  path="$(odin_mode_state_path)" || return 1
  odin_mode_state_init >/dev/null || return 1
  _odin_mode_state_load "${path}" >/dev/null || return 1

  case "${field}" in
    mode)
      echo "${_ODIN_MODE_STATE_MODE}"
      ;;
    confidence)
      echo "${_ODIN_MODE_STATE_CONFIDENCE}"
      ;;
    guardrails_acknowledged)
      echo "${_ODIN_MODE_STATE_GUARDRAILS_ACK}"
      ;;
    task_cycle_verified)
      echo "${_ODIN_MODE_STATE_TASK_CYCLE}"
      ;;
    last_verify_passed)
      echo "${_ODIN_MODE_STATE_LAST_VERIFY}"
      ;;
    recorded_events)
      echo "${_ODIN_MODE_STATE_RECORDED_EVENTS}"
      ;;
    *)
      return 64
      ;;
  esac
}

_odin_mode_state_record_event_unlocked() {
  local path="$1"
  local event="$2"
  _odin_mode_state_init_unlocked "${path}" || return 1
  _odin_mode_state_load "${path}" >/dev/null || return 1

  local points=0
  case "${event}" in
    provider.connected.verified|tui.opened.verified|inbox.first_item.verified|task.split.verified|delegation.completed.verified)
      if ! _odin_mode_state_event_seen "${_ODIN_MODE_STATE_RECORDED_EVENTS}" "${event}"; then
        points=10
      fi
      ;;
    guardrails.acknowledged.verified)
      _ODIN_MODE_STATE_GUARDRAILS_ACK="true"
      if ! _odin_mode_state_event_seen "${_ODIN_MODE_STATE_RECORDED_EVENTS}" "${event}"; then
        points=10
      fi
      ;;
    task.cycle.verified)
      _ODIN_MODE_STATE_TASK_CYCLE="true"
      if ! _odin_mode_state_event_seen "${_ODIN_MODE_STATE_RECORDED_EVENTS}" "${event}"; then
        points=10
      fi
      ;;
    verify.passed.verified)
      _ODIN_MODE_STATE_LAST_VERIFY="true"
      ;;
    verify.failed)
      _ODIN_MODE_STATE_LAST_VERIFY="false"
      _ODIN_MODE_STATE_MODE="RECOVERY"
      _ODIN_MODE_STATE_RECORDED_EVENTS="$(_odin_mode_state_event_append "${_ODIN_MODE_STATE_RECORDED_EVENTS}" "${event}")"
      _odin_mode_state_write "${path}"
      return 0
      ;;
    *)
      ;;
  esac

  if [[ "${event}" == *.verified ]]; then
    _ODIN_MODE_STATE_RECORDED_EVENTS="$(_odin_mode_state_event_append "${_ODIN_MODE_STATE_RECORDED_EVENTS}" "${event}")"
  fi

  _ODIN_MODE_STATE_CONFIDENCE=$((_ODIN_MODE_STATE_CONFIDENCE + points))
  if (( _ODIN_MODE_STATE_CONFIDENCE > 100 )); then
    _ODIN_MODE_STATE_CONFIDENCE=100
  fi

  _odin_mode_state_write "${path}"
}

odin_mode_state_record_event() {
  local event="$1"
  local path
  path="$(odin_mode_state_path)" || return 1
  _odin_mode_state_with_lock "${path}" _odin_mode_state_record_event_unlocked "${path}" "${event}"
}

_odin_mode_state_can_operate_in_memory() {
  if [[ "${_ODIN_MODE_STATE_MODE}" == "RECOVERY" ]]; then
    return 1
  fi
  if (( _ODIN_MODE_STATE_CONFIDENCE < 60 )); then
    return 1
  fi
  if [[ "${_ODIN_MODE_STATE_GUARDRAILS_ACK}" != "true" ]]; then
    return 1
  fi
  if [[ "${_ODIN_MODE_STATE_TASK_CYCLE}" != "true" ]]; then
    return 1
  fi
  if [[ "${_ODIN_MODE_STATE_LAST_VERIFY}" != "true" ]]; then
    return 1
  fi
  return 0
}

odin_mode_state_can_operate() {
  local path
  path="$(odin_mode_state_path)" || return 1
  odin_mode_state_init >/dev/null || return 1
  _odin_mode_state_load "${path}" >/dev/null || return 1
  _odin_mode_state_can_operate_in_memory
}

_odin_mode_state_set_mode_unlocked() {
  local path="$1"
  local next_mode="$2"
  _odin_mode_state_init_unlocked "${path}" || return 1
  _odin_mode_state_load "${path}" >/dev/null || return 1

  _odin_mode_state_validate_mode "${next_mode}" || return 64

  if [[ "${next_mode}" == "OPERATE" ]]; then
    if ! _odin_mode_state_can_operate_in_memory; then
      return 2
    fi
  fi

  _ODIN_MODE_STATE_MODE="${next_mode}"
  _odin_mode_state_write "${path}"
}

odin_mode_state_set_mode() {
  local next_mode="$1"
  local path
  path="$(odin_mode_state_path)" || return 1
  _odin_mode_state_with_lock "${path}" _odin_mode_state_set_mode_unlocked "${path}" "${next_mode}"
}
