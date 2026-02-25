"""Activity logs collector."""

from __future__ import annotations

import json
from datetime import datetime
from pathlib import Path

from tui_core.models import PanelData

LOG_SOURCES = [
    "events.jsonl",
    "agents.log",
    "inbox.log",
    "keepalive.log",
    "alerts.log",
    "cost.log",
    "ssh-dispatch.log",
]


def _extract_text(line: str) -> str:
    line = line.strip()
    if not line:
        return ""
    if line.startswith("{") and line.endswith("}"):
        try:
            payload = json.loads(line)
            event = payload.get("event") or payload.get("event_type") or "event"
            msg = payload.get("message") or payload.get("detail") or payload.get("task_id") or ""
            return f"{event}: {msg}".strip()
        except json.JSONDecodeError:
            return line
    return line


def collect(odin_dir: Path, limit: int = 30) -> PanelData:
    today = datetime.now().strftime("%Y-%m-%d")
    log_dir = odin_dir / "logs" / today

    entries: list[dict] = []
    if log_dir.exists():
        for name in LOG_SOURCES:
            path = log_dir / name
            if not path.exists() or not path.is_file():
                continue
            try:
                lines = path.read_text(errors="replace").splitlines()[-8:]
            except OSError:
                continue
            for raw in lines:
                text = _extract_text(raw)
                if not text:
                    continue
                entries.append({
                    "source": name,
                    "message": text,
                })

    entries = entries[-limit:]
    status = "ok" if entries else "warn"
    return PanelData(
        key="logs",
        title="Logs",
        status=status,
        items=entries,
        meta={
            "shown": len(entries),
            "log_dir": str(log_dir),
        },
        errors=[] if entries else ["no log lines available"],
    )
