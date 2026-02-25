"""Inbox queue collector."""

from __future__ import annotations

from pathlib import Path

from tui_core.collectors import file_age_seconds, list_json_files, read_json
from tui_core.formatting import compact_relative_age, task_label_for_type
from tui_core.models import PanelData


def collect(odin_dir: Path, limit: int = 20) -> PanelData:
    inbox_dir = odin_dir / "inbox"
    files = list_json_files(inbox_dir)

    items: list[dict] = []
    for file_path in files[:limit]:
        payload = read_json(file_path) or {}
        task_id = payload.get("task_id", file_path.stem)
        payload_type = (payload.get("payload") or {}).get("task_type")
        task_type = payload_type or payload.get("type") or "unknown"
        source = payload.get("source", "unknown")
        age = compact_relative_age(file_age_seconds(file_path))
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
