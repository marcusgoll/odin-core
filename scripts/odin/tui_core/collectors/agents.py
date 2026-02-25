"""Agent status collector."""

from __future__ import annotations

from pathlib import Path

from tui_core.collectors import read_json
from tui_core.formatting import parse_iso_timestamp
from tui_core.models import PanelData

AGENT_EXCLUDE = {"orchestrator", "self"}


def _resolve_agent_state(status_data: dict, task: str) -> str:
    state = status_data.get("status") or status_data.get("state")
    if state:
        return str(state)
    if task and task != "-":
        return "busy"
    return "unknown"


def _dispatch_sort_key(task_id: str, info: dict | None) -> tuple[int, float, str]:
    created_at = parse_iso_timestamp((info or {}).get("created_at"))
    if created_at is None:
        return (0, float("-inf"), task_id)
    return (1, created_at.timestamp(), task_id)


def collect(odin_dir: Path) -> PanelData:
    agents_dir = odin_dir / "agents"
    state = read_json(odin_dir / "state.json") or {}
    dispatch = state.get("dispatched_tasks") or {}

    dispatch_by_agent: dict[str, tuple[tuple[int, float, str], str]] = {}
    for task_id, info in dispatch.items():
        agent = (info or {}).get("agent")
        if agent:
            key = _dispatch_sort_key(task_id, info)
            current = dispatch_by_agent.get(agent)
            if current is None or key > current[0]:
                dispatch_by_agent[agent] = (key, task_id)

    items: list[dict] = []
    if agents_dir.exists():
        for child in sorted(agents_dir.iterdir()):
            if not child.is_dir() or child.name in AGENT_EXCLUDE:
                continue
            status_data = read_json(child / "status.json") or {}
            dispatch_entry = dispatch_by_agent.get(child.name)
            task = dispatch_entry[1] if dispatch_entry else "-"
            item = {
                "name": child.name,
                "role": status_data.get("role", "agent"),
                "state": _resolve_agent_state(status_data, task),
                "task": task,
            }
            items.append(item)

    status = "ok" if items else "warn"
    return PanelData(
        key="agents",
        title="Agents",
        status=status,
        items=items,
        meta={
            "count": len(items),
            "busy": len([a for a in items if a.get("task") not in ("", "-")]),
        },
        errors=[] if items else ["no agents discovered"],
    )
