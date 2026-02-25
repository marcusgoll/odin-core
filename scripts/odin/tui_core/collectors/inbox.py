"""Inbox queue collector."""

from __future__ import annotations

from pathlib import Path

from tui_core.collectors import list_json_files, read_json
from tui_core.models import PanelData


def collect(odin_dir: Path, limit: int = 20) -> PanelData:
    inbox_dir = odin_dir / "inbox"
    files = list_json_files(inbox_dir)

    items: list[dict] = []
    for file_path in files[:limit]:
        payload = read_json(file_path) or {}
        task_id = payload.get("task_id", file_path.stem)
        task_type = payload.get("type") or (payload.get("payload") or {}).get("task_type") or "unknown"
        source = payload.get("source", "unknown")
        items.append(
            {
                "task_id": str(task_id),
                "type": str(task_type),
                "source": str(source),
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
