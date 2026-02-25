"""Inbox panel renderer."""

from __future__ import annotations

from rich.table import Table

from tui_core.models import PanelData
from tui_core.panels import panel_from_table


def render(data: PanelData):
    table = Table(box=None, expand=True)
    table.add_column("Task", overflow="fold")
    table.add_column("Source", overflow="fold", no_wrap=True)
    table.add_column("Age", justify="right", no_wrap=True)

    if not data.items:
        table.add_row("-", "-", "-")
    else:
        for item in data.items[:20]:
            table.add_row(
                str(item.get("task_label") or item.get("task_id", "-")),
                str(item.get("source", "-")),
                str(item.get("age", "n/a")),
            )

    return panel_from_table(f"Inbox ({data.meta.get('pending', 0)})", data.status, table)
