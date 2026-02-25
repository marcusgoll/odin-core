"""Kanban board collector."""

from __future__ import annotations

from pathlib import Path

from tui_core.collectors import read_json
from tui_core.formatting import task_label_for_type, wip_state
from tui_core.models import PanelData


def _tasks_from_value(value: object) -> list[dict]:
    if isinstance(value, list):
        return [row for row in value if isinstance(row, dict)]
    if not isinstance(value, dict):
        return []

    tasks = value.get("tasks")
    if isinstance(tasks, list):
        return [row for row in tasks if isinstance(row, dict)]

    items = value.get("items")
    if isinstance(items, list):
        return [row for row in items if isinstance(row, dict)]
    return []


def _task_preview(task: dict) -> str:
    title = task.get("title")
    if isinstance(title, str) and title.strip():
        return title.strip()

    task_type = task.get("task_type") or task.get("type")
    if task_type:
        return task_label_for_type(str(task_type))

    issue_number = task.get("issue_number")
    if issue_number is not None:
        return f"Issue #{issue_number}"

    return "Task"


def _summarize_columns(board: dict) -> list[dict]:
    columns = board.get("columns")
    result: list[dict] = []

    if isinstance(columns, dict):
        for name, value in columns.items():
            tasks = _tasks_from_value(value)
            count = len(tasks)
            limit = int(value.get("wip_limit", 0)) if isinstance(value, dict) else 0
            result.append(
                {
                    "column": str(name),
                    "count": count,
                    "wip_limit": limit,
                    "wip": f"{count}/{limit}" if limit > 0 else f"{count}/-",
                    "wip_state": wip_state(count, limit),
                    "top_tasks": [_task_preview(task) for task in tasks[:3]],
                }
            )

    elif isinstance(columns, list):
        for col in columns:
            if not isinstance(col, dict):
                continue
            name = col.get("name") or col.get("id") or "column"
            tasks = _tasks_from_value(col)
            count = len(tasks)
            limit = int(col.get("wip_limit", 0)) if col.get("wip_limit") is not None else 0
            result.append(
                {
                    "column": str(name),
                    "count": count,
                    "wip_limit": limit,
                    "wip": f"{count}/{limit}" if limit > 0 else f"{count}/-",
                    "wip_state": wip_state(count, limit),
                    "top_tasks": [_task_preview(task) for task in tasks[:3]],
                }
            )

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
    over_limit = any(c.get("wip_state") == "over" for c in columns)
    status = "warn" if over_limit or not columns else "ok"
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
