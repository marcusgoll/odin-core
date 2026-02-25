"""Kanban board collector."""

from __future__ import annotations

from pathlib import Path

from tui_core.collectors import read_json
from tui_core.models import PanelData


def _summarize_columns(board: dict) -> list[dict]:
    columns = board.get("columns")
    result: list[dict] = []

    if isinstance(columns, dict):
        for name, value in columns.items():
            if isinstance(value, list):
                count = len(value)
            elif isinstance(value, dict):
                tasks = value.get("tasks")
                count = len(tasks) if isinstance(tasks, list) else 0
            else:
                count = 0
            result.append({"column": str(name), "count": count})

    elif isinstance(columns, list):
        for col in columns:
            if not isinstance(col, dict):
                continue
            name = col.get("name") or col.get("id") or "column"
            tasks = col.get("tasks") if isinstance(col.get("tasks"), list) else []
            result.append({"column": str(name), "count": len(tasks)})

    return result


def collect(odin_dir: Path) -> PanelData:
    board = read_json(odin_dir / "kanban" / "board.json")
    if not isinstance(board, dict):
        return PanelData(
            key="kanban",
            title="Kanban",
            status="warn",
            items=[],
            meta={"columns": 0, "total_tasks": 0},
            errors=["board.json missing or invalid"],
        )

    columns = _summarize_columns(board)
    total = sum(c.get("count", 0) for c in columns)
    status = "ok" if columns else "warn"
    return PanelData(
        key="kanban",
        title="Kanban",
        status=status,
        items=columns,
        meta={
            "columns": len(columns),
            "total_tasks": total,
        },
        errors=[] if columns else ["no columns found"],
    )
