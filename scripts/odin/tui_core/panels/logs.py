"""Logs panel renderer."""

from __future__ import annotations

from rich.table import Table

from tui_core.models import PanelData
from tui_core.panels import panel_from_table


def render(data: PanelData):
    table = Table(box=None, expand=True)
    table.add_column("Time", no_wrap=True)
    table.add_column("Source", style="cyan", no_wrap=True)
    table.add_column("Message", overflow="fold")

    if not data.items:
        table.add_row("-", "-", "No logs")
    else:
        for item in data.items[-25:]:
            table.add_row(
                str(item.get("time", "n/a")),
                str(item.get("source", "-")),
                str(item.get("message", "")),
            )

    return panel_from_table("Logs", data.status, table)
