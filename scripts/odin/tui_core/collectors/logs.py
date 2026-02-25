"""Activity logs collector."""

from __future__ import annotations

import json
import re
from datetime import datetime
from pathlib import Path

from tui_core.formatting import parse_iso_timestamp
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

TIMESTAMP_RE = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+\-]\d{2}:\d{2})")
MAX_LINES_PER_SOURCE = 32


def _extract_structured_line(line: str) -> tuple[str | None, str]:
    line = line.strip()
    if not line:
        return None, ""

    if line.startswith("{") and line.endswith("}"):
        try:
            payload = json.loads(line)
            ts = payload.get("ts") or payload.get("timestamp") or payload.get("time")
            event = payload.get("event") or payload.get("event_type") or payload.get("msg")
            msg = payload.get("message") or payload.get("detail") or payload.get("task_id") or ""
            if event and msg:
                text = f"{event}: {msg}"
            else:
                text = str(event or msg or "event")
            return str(ts) if ts else None, text.strip()
        except json.JSONDecodeError:
            return _extract_text_with_timestamp(line)

    return _extract_text_with_timestamp(line)


def _extract_text_with_timestamp(line: str) -> tuple[str | None, str]:
    match = TIMESTAMP_RE.search(line)
    if not match:
        return None, line

    timestamp = match.group(0)
    left = line[: match.start()].strip()
    right = line[match.end() :].strip()

    if left and right:
        text = f"{left} {right}".strip()
    else:
        text = left or right or line
    text = re.sub(r"^\[\s*\]\s*", "", text)
    text = re.sub(r"\s{2,}", " ", text).strip()
    return timestamp, text


def _display_time(ts_value: str | None) -> str:
    parsed = parse_iso_timestamp(ts_value)
    if parsed is None:
        return "n/a"
    return parsed.strftime("%H:%M:%S")


def collect(odin_dir: Path, limit: int = 30) -> PanelData:
    today = datetime.now().strftime("%Y-%m-%d")
    log_dir = odin_dir / "logs" / today

    scanned: list[dict] = []
    seq = 0
    if log_dir.exists():
        for name in LOG_SOURCES:
            path = log_dir / name
            if not path.exists() or not path.is_file():
                continue
            try:
                lines = path.read_text(errors="replace").splitlines()[-MAX_LINES_PER_SOURCE:]
            except OSError:
                continue
            for raw in lines:
                ts, text = _extract_structured_line(raw)
                if not text:
                    continue
                parsed = parse_iso_timestamp(ts)
                scanned.append(
                    {
                        "_seq": seq,
                        "_epoch": parsed.timestamp() if parsed is not None else -1.0,
                        "_has_ts": 1 if parsed is not None else 0,
                        "source": name,
                        "ts": ts or "",
                        "time": _display_time(ts),
                        "message": text,
                    }
                )
                seq += 1

    scanned.sort(key=lambda row: (row["_has_ts"], row["_epoch"], row["_seq"]))
    entries = [
        {
            "source": row["source"],
            "ts": row["ts"],
            "time": row["time"],
            "message": row["message"],
        }
        for row in scanned[-limit:]
    ]
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
