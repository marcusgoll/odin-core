#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

echo "[quickstart] root=${ROOT_DIR}"

if [[ ! -f .env.example ]]; then
  echo "[quickstart] ERROR missing .env.example" >&2
  exit 66
fi

created_env=0
if [[ ! -f .env ]]; then
  cp .env.example .env
  created_env=1
  echo "[quickstart] created temporary .env from .env.example"
fi

cleanup() {
  if [[ "${created_env}" -eq 1 ]]; then
    rm -f .env
    echo "[quickstart] cleaned temporary .env"
  fi
}
trap cleanup EXIT

if command -v docker >/dev/null 2>&1; then
  if docker compose version >/dev/null 2>&1; then
    echo "[quickstart] RUN docker compose config"
    docker compose config >/dev/null
    echo "[quickstart] PASS docker compose config"
  else
    echo "[quickstart] WARN docker found but docker compose unavailable; skipping compose validation"
  fi
else
  echo "[quickstart] WARN docker unavailable; skipping compose validation"
fi

echo "[quickstart] RUN cargo run bootstrap"
timeout 30 cargo run -p odin-cli -- --run-once --config config/default.yaml >/tmp/odin_quickstart_cli.out 2>/tmp/odin_quickstart_cli.err
cat /tmp/odin_quickstart_cli.out
if [[ -s /tmp/odin_quickstart_cli.err ]]; then
  cat /tmp/odin_quickstart_cli.err >&2
fi
echo "[quickstart] PASS cargo bootstrap"

echo "[quickstart] RUN watchdog task bridge smoke"
cat > /tmp/odin_watchdog_quickstart.json <<'JSON'
{
  "schema_version": 1,
  "task_id": "watchdog-poll-quickstart-1700000000",
  "type": "watchdog_poll",
  "source": "quickstart-smoke",
  "created_at": "2026-02-25T00:00:00Z",
  "payload": {
    "task_type": "watchdog.sentry.poll",
    "source_key": "quickstart",
    "project": "private",
    "plugin": "private.ops-watchdog",
    "trigger": "smoke"
  }
}
JSON

cargo run -p odin-cli -- \
  --task-file /tmp/odin_watchdog_quickstart.json \
  --plugins-root "${ROOT_DIR}/examples/private-plugins" \
  --run-once >/tmp/odin_quickstart_watchdog.out 2>/tmp/odin_quickstart_watchdog.err
cat /tmp/odin_quickstart_watchdog.out
if [[ -s /tmp/odin_quickstart_watchdog.err ]]; then
  cat /tmp/odin_quickstart_watchdog.err >&2
fi
echo "[quickstart] PASS watchdog bridge smoke"

echo "[quickstart] COMPLETE"
