#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

echo "[tui-smoke] RUN readability tests"
python3 -m unittest scripts.odin.tui_core.tests.test_readability

echo "[tui-smoke] RUN compile"
python3 -m py_compile scripts/odin/odin-tui.py scripts/odin/odin-tui-legacy.py scripts/odin/tui_core/app.py

echo "[tui-smoke] RUN core json"
python3 scripts/odin/odin-tui.py --json >/tmp/odin_tui_core_json.out
python3 - <<'PY'
import json
from pathlib import Path

payload = json.loads(Path('/tmp/odin_tui_core_json.out').read_text())
required = ["inbox", "kanban", "agents", "logs", "github"]
missing = [k for k in required if k not in payload]
if missing:
    raise SystemExit(f"missing keys: {missing}")
print("core_json_ok")
PY

echo "[tui-smoke] RUN legacy json"
python3 scripts/odin/odin-tui.py --profile legacy --json >/tmp/odin_tui_legacy_json.out
python3 - <<'PY'
import json
from pathlib import Path

payload = json.loads(Path('/tmp/odin_tui_legacy_json.out').read_text())
if "agents" not in payload:
    raise SystemExit("legacy payload missing agents")
print("legacy_json_ok")
PY

echo "[tui-smoke] RUN live startup core"
set +e
timeout 6 python3 scripts/odin/odin-tui.py --live >/tmp/odin_tui_core_live.out 2>/tmp/odin_tui_core_live.err
rc_core=$?
timeout 6 bash scripts/odin/odin-tui --live >/tmp/odin_tui_wrapper_live.out 2>/tmp/odin_tui_wrapper_live.err
rc_wrapper=$?
set -e

if [[ "$rc_core" -ne 0 && "$rc_core" -ne 124 ]]; then
  echo "core live failed rc=$rc_core" >&2
  exit 1
fi
if [[ "$rc_wrapper" -ne 0 && "$rc_wrapper" -ne 124 ]]; then
  echo "wrapper live failed rc=$rc_wrapper" >&2
  exit 1
fi

echo "[tui-smoke] COMPLETE"
