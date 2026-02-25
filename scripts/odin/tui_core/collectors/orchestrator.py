"""Orchestrator status collector."""

from __future__ import annotations

from pathlib import Path

from tui_core.collectors import file_age_seconds, format_age, read_json
from tui_core.models import PanelData


def collect(odin_dir: Path) -> PanelData:
    heartbeat = odin_dir / "heartbeat"
    age = file_age_seconds(heartbeat)
    state = read_json(odin_dir / "state.json") or {}
    backend = state.get("orchestrator_backend") or state.get("backend") or "unknown"

    if age is None:
        status = "error"
        health = "missing heartbeat"
    elif age < 120:
        status = "ok"
        health = "healthy"
    elif age < 600:
        status = "warn"
        health = "degraded"
    else:
        status = "error"
        health = "stale"

    return PanelData(
        key="orchestrator",
        title="Orchestrator",
        status=status,
        items=[
            {"label": "Health", "value": health},
            {"label": "Heartbeat", "value": format_age(age)},
            {"label": "Backend", "value": str(backend)},
        ],
        meta={
            "heartbeat_age_seconds": age,
            "backend": backend,
        },
        errors=[] if age is not None else ["heartbeat file unavailable"],
    )
