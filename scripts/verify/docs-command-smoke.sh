#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"
TMP_DIR="$(mktemp -d /tmp/odin-docs-command-smoke.XXXXXX)"
MISSING_GUARDRAILS_PATH="${TMP_DIR}/missing-guardrails.yaml"
MODE_STATE_PATH="${TMP_DIR}/bootstrap-state.json"
DOC_COMMANDS_TSV="${TMP_DIR}/doc-commands.tsv"
DOC_FILES=(
  "docs/quickstart.md"
  "docs/integrations/n8n.md"
  "docs/integrations/slack.md"
  "docs/integrations/telegram.md"
)

cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

require_file() {
  local path="$1"
  if [[ ! -f "${path}" ]]; then
    echo "[docs-command] ERROR missing required file: ${path}" >&2
    exit 1
  fi
  echo "[docs-command] PASS file exists: ${path}"
}

require_snippet() {
  local path="$1"
  local snippet="$2"
  if ! grep -Fq "${snippet}" "${path}"; then
    echo "[docs-command] ERROR missing snippet in ${path}: ${snippet}" >&2
    exit 1
  fi
  echo "[docs-command] PASS snippet in ${path}: ${snippet}"
}

extract_doc_commands() {
  local doc_path="$1"
  local line=""
  local command_line=""
  local maybe_delim=""
  local line_no=0
  local in_bash_fence=0
  local heredoc_delim=""

  while IFS= read -r line || [[ -n "${line}" ]]; do
    line_no=$((line_no + 1))

    if [[ -n "${heredoc_delim}" ]]; then
      if [[ "${line}" == "${heredoc_delim}" ]]; then
        heredoc_delim=""
      fi
      continue
    fi

    if [[ "${in_bash_fence}" -eq 0 ]]; then
      if [[ "${line}" =~ ^\`\`\`bash[[:space:]]*$ ]]; then
        in_bash_fence=1
      fi
      continue
    fi

    if [[ "${line}" =~ ^\`\`\`[[:space:]]*$ ]]; then
      in_bash_fence=0
      continue
    fi

    if [[ "${line}" =~ ^[[:space:]]*$ ]] || [[ "${line}" =~ ^[[:space:]]*# ]]; then
      continue
    fi

    command_line="$(printf '%s' "${line}" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//')"
    if [[ -z "${command_line}" ]]; then
      continue
    fi

    maybe_delim="$(printf '%s\n' "${command_line}" | sed -nE "s/.*<<-?'?([A-Za-z_][A-Za-z0-9_]*)'?$/\1/p")"
    if [[ -n "${maybe_delim}" ]]; then
      heredoc_delim="${maybe_delim}"
    fi

    printf '%s\t%s\t%s\n' "${doc_path}" "${line_no}" "${command_line}"
  done < "${doc_path}"

  if [[ "${in_bash_fence}" -ne 0 ]]; then
    echo "[docs-command] ERROR unterminated bash fence in ${doc_path}" >&2
    exit 1
  fi

  if [[ -n "${heredoc_delim}" ]]; then
    echo "[docs-command] ERROR unterminated heredoc '${heredoc_delim}' in ${doc_path}" >&2
    exit 1
  fi
}

discover_doc_commands() {
  : > "${DOC_COMMANDS_TSV}"

  local doc_path=""
  for doc_path in "${DOC_FILES[@]}"; do
    extract_doc_commands "${doc_path}" >> "${DOC_COMMANDS_TSV}"
  done

  local command_count
  command_count="$(wc -l < "${DOC_COMMANDS_TSV}" | tr -d '[:space:]')"
  if [[ "${command_count}" -eq 0 ]]; then
    echo "[docs-command] ERROR no commands discovered in docs markdown" >&2
    exit 1
  fi

  echo "[docs-command] DISCOVERED ${command_count} command lines from docs bash snippets"

  local discovered_doc=""
  local discovered_line=""
  local discovered_cmd=""
  while IFS=$'\t' read -r discovered_doc discovered_line discovered_cmd; do
    echo "[docs-command] COMMAND ${discovered_doc}:${discovered_line} ${discovered_cmd}"
  done < "${DOC_COMMANDS_TSV}"
}

require_extracted_command() {
  local command_line="$1"
  if ! awk -F '\t' -v needle="${command_line}" '$3 == needle { found = 1 } END { exit found ? 0 : 1 }' "${DOC_COMMANDS_TSV}"; then
    echo "[docs-command] ERROR discovered commands missing expected entry: ${command_line}" >&2
    exit 1
  fi
  echo "[docs-command] PASS discovered command: ${command_line}"
}

run_wrapper() {
  env \
    ODIN_GUARDRAILS_PATH="${MISSING_GUARDRAILS_PATH}" \
    ODIN_MODE_STATE_PATH="${MODE_STATE_PATH}" \
    "$@"
}

expect_success() {
  local label="$1"
  local expected_out="$2"
  shift 2

  local out_file
  local err_file
  out_file="$(mktemp "${TMP_DIR}/out.XXXXXX")"
  err_file="$(mktemp "${TMP_DIR}/err.XXXXXX")"

  echo "[docs-command] RUN ${label}"

  set +e
  "$@" >"${out_file}" 2>"${err_file}"
  local rc=$?
  set -e

  if [[ "${rc}" -ne 0 ]]; then
    echo "[docs-command] ERROR expected rc=0 for ${label}, got rc=${rc}" >&2
    cat "${out_file}" >&2
    cat "${err_file}" >&2
    exit 1
  fi

  if [[ -n "${expected_out}" ]] && ! grep -Fq "${expected_out}" "${out_file}"; then
    echo "[docs-command] ERROR expected stdout pattern '${expected_out}' for ${label}" >&2
    cat "${out_file}" >&2
    cat "${err_file}" >&2
    exit 1
  fi

  echo "[docs-command] PASS ${label}"
}

expect_failure() {
  local label="$1"
  local expected_rc="$2"
  local expected_err="$3"
  shift 3

  local out_file
  local err_file
  out_file="$(mktemp "${TMP_DIR}/out.XXXXXX")"
  err_file="$(mktemp "${TMP_DIR}/err.XXXXXX")"

  echo "[docs-command] RUN ${label}"

  set +e
  "$@" >"${out_file}" 2>"${err_file}"
  local rc=$?
  set -e

  if [[ "${rc}" -ne "${expected_rc}" ]]; then
    echo "[docs-command] ERROR expected rc=${expected_rc} for ${label}, got rc=${rc}" >&2
    cat "${out_file}" >&2
    cat "${err_file}" >&2
    exit 1
  fi

  if [[ -n "${expected_err}" ]] && ! grep -Fq "${expected_err}" "${err_file}"; then
    echo "[docs-command] ERROR expected stderr pattern '${expected_err}' for ${label}" >&2
    cat "${out_file}" >&2
    cat "${err_file}" >&2
    exit 1
  fi

  echo "[docs-command] PASS ${label} rc=${rc}"
}

for doc_path in "${DOC_FILES[@]}"; do
  require_file "${doc_path}"
done

require_snippet "README.md" "docs/quickstart.md"
require_snippet "README.md" "docs/integrations/n8n.md"
require_snippet "README.md" "docs/integrations/slack.md"
require_snippet "README.md" "docs/integrations/telegram.md"

discover_doc_commands

require_extracted_command "scripts/odin/odin help"
require_extracted_command "scripts/odin/odin connect claude oauth --dry-run"
require_extracted_command "scripts/odin/odin start --dry-run"
require_extracted_command "scripts/odin/odin tui --dry-run"
require_extracted_command "scripts/odin/odin inbox add \"bootstrap task\" --dry-run"
require_extracted_command "scripts/odin/odin inbox list"
require_extracted_command "scripts/odin/odin gateway add cli --dry-run"
require_extracted_command "scripts/odin/odin gateway add slack --dry-run"
require_extracted_command "scripts/odin/odin gateway add telegram --dry-run"
require_extracted_command "scripts/odin/odin verify --dry-run"
require_extracted_command "bash scripts/verify/docs-command-smoke.sh"
require_extracted_command "scripts/odin/odin inbox add \"n8n bootstrap task\" --dry-run"

expect_success \
  "help usage" \
  "Usage:" \
  run_wrapper scripts/odin/odin help
expect_success \
  "quickstart connect dry-run" \
  "DRY-RUN connect provider=claude auth=oauth" \
  run_wrapper scripts/odin/odin connect claude oauth --dry-run
expect_success \
  "quickstart start dry-run" \
  "DRY-RUN start" \
  run_wrapper scripts/odin/odin start --dry-run
expect_success \
  "quickstart tui dry-run" \
  "DRY-RUN tui" \
  run_wrapper scripts/odin/odin tui --dry-run
expect_success \
  "quickstart inbox add dry-run" \
  "DRY-RUN inbox add title=bootstrap task" \
  run_wrapper scripts/odin/odin inbox add "bootstrap task" --dry-run
expect_success \
  "quickstart inbox list" \
  "inbox list placeholder (empty)" \
  run_wrapper scripts/odin/odin inbox list
expect_success \
  "quickstart gateway cli dry-run" \
  "DRY-RUN gateway add source=cli" \
  run_wrapper scripts/odin/odin gateway add cli --dry-run
expect_success \
  "quickstart gateway slack dry-run" \
  "DRY-RUN gateway add source=slack" \
  run_wrapper scripts/odin/odin gateway add slack --dry-run
expect_success \
  "quickstart gateway telegram dry-run" \
  "DRY-RUN gateway add source=telegram" \
  run_wrapper scripts/odin/odin gateway add telegram --dry-run
expect_success \
  "quickstart verify dry-run" \
  "DRY-RUN verify" \
  run_wrapper scripts/odin/odin verify --dry-run
expect_success \
  "n8n gateway cli dry-run" \
  "DRY-RUN gateway add source=cli" \
  run_wrapper scripts/odin/odin gateway add cli --dry-run
expect_success \
  "n8n inbox add dry-run" \
  "DRY-RUN inbox add title=n8n bootstrap task" \
  run_wrapper scripts/odin/odin inbox add "n8n bootstrap task" --dry-run

expect_failure \
  "guardrails missing blocks mutating start" \
  2 \
  "BLOCKED start: guardrails file not found" \
  run_wrapper scripts/odin/odin start

expect_success \
  "guardrails missing smallest fix is --dry-run" \
  "DRY-RUN start" \
  run_wrapper scripts/odin/odin start --dry-run

echo "[docs-command] COMPLETE"
