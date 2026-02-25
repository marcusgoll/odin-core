"""Kanban panel renderer."""

from __future__ import annotations

from rich.table import Table

from tui_core.models import PanelData
from tui_core.panels import panel_from_table


def render(data: PanelData):
    table = Table(box=None, expand=True)
    table.add_column("Column", overflow="fold")
    table.add_column("Count", justify="right")

    if not data.items:
        table.add_row("no columns", "0")
    else:
        for item in data.items:
            table.add_row(str(item.get("column", "-")), str(item.get("count", 0)))

    title = f"Kanban ({data.meta.get('total_tasks', 0)} tasks)"
    return panel_from_table(title, data.status, table)
