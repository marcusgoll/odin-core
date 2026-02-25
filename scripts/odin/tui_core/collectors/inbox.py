"""Inbox queue collector."""

from __future__ import annotations

from datetime import datetime, timezone
from pathlib import Path

from tui_core.collectors import file_age_seconds, list_json_files, read_json
from tui_core.formatting import compact_relative_age, parse_iso_timestamp, task_label_for_type
from tui_core.models import PanelData


def _age_seconds(payload: dict, file_path: Path, now: datetime) -> float | None:
    created_at = payload.get("created_at")
    received_at = (payload.get("ingest_metadata") or {}).get("received_at")
    for candidate in (created_at, received_at):
        parsed = parse_iso_timestamp(str(candidate) if candidate is not None else None)
        if parsed is not None:
            return max(0.0, (now - parsed).total_seconds())
    return file_age_seconds(file_path)


def collect(odin_dir: Path, limit: int = 20) -> PanelData:
    inbox_dir = odin_dir / "inbox"
    files = list_json_files(inbox_dir)

    items: list[dict] = []
    now = datetime.now(timezone.utc)
    for file_path in files[:limit]:
        payload = read_json(file_path) or {}
        task_id = payload.get("task_id", file_path.stem)
        payload_type = (payload.get("payload") or {}).get("task_type")
        task_type = payload_type or payload.get("type") or "unknown"
        source = payload.get("source", "unknown")
        age = compact_relative_age(_age_seconds(payload, file_path, now))
        items.append(
            {
                "task_id": str(task_id),
                "type": str(task_type),
                "task_label": task_label_for_type(str(task_type)),
                "source": str(source),
                "age": age,
            }
        )

    status = "ok" if files else "warn"
    return PanelData(
        key="inbox",
        title="Inbox",
        status=status,
        items=items,
        meta={
            "pending": len(files),
            "shown": len(items),
        },
        errors=[] if files else ["inbox empty"],
    )
