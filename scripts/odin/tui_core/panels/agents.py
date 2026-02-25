"""Agents panel renderer."""

from __future__ import annotations

from rich.table import Table

from tui_core.models import PanelData
from tui_core.panels import panel_from_table


def render(data: PanelData):
    table = Table(box=None, expand=True)
    table.add_column("Agent", overflow="fold")
    table.add_column("State")
    table.add_column("Task", overflow="fold")

    if not data.items:
        table.add_row("none", "unknown", "-")
    else:
        for item in data.items[:20]:
            table.add_row(
                str(item.get("name", "-")),
                str(item.get("state", "-")),
                str(item.get("task", "-")),
            )

    title = f"Agents ({data.meta.get('count', 0)})"
    return panel_from_table(title, data.status, table)
