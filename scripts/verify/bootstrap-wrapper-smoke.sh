#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

run() {
  echo "[bootstrap-wrapper] RUN $*"
  "$@"
  echo "[bootstrap-wrapper] PASS $*"
}

run scripts/odin/odin help
run scripts/odin/odin connect claude oauth --dry-run
run scripts/odin/odin start --dry-run
run scripts/odin/odin tui --dry-run
run scripts/odin/odin inbox add "test task" --dry-run
run scripts/odin/odin verify --dry-run

echo "[bootstrap-wrapper] COMPLETE"
