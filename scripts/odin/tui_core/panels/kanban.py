"""Kanban panel renderer."""

from __future__ import annotations

from rich.table import Table
from rich.text import Text

from tui_core.models import PanelData
from tui_core.panels import panel_from_table


def render(data: PanelData):
    table = Table(box=None, expand=True)
    table.add_column("Column", overflow="fold", no_wrap=True)
    table.add_column("WIP", justify="right", no_wrap=True)
    table.add_column("Top Tasks", overflow="fold")

    if not data.items:
        table.add_row("no columns", "0/-", "No active tasks")
    else:
        for item in data.items:
            state = str(item.get("wip_state", "ok"))
            style = "green"
            if state == "full":
                style = "yellow"
            elif state == "over":
                style = "red"
            elif state == "unbounded":
                style = "cyan"

            top_tasks = item.get("top_tasks") or []
            top_text = " | ".join(str(task) for task in top_tasks[:3]) if top_tasks else "No active tasks"
            table.add_row(
                str(item.get("column", "-")),
                Text(str(item.get("wip", "0/-")), style=style),
                top_text,
            )

    title = f"Kanban ({data.meta.get('total_tasks', 0)} tasks)"
    return panel_from_table(title, data.status, table)
