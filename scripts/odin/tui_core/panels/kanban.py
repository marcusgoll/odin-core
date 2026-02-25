"""Kanban panel renderer."""

from __future__ import annotations

from rich.columns import Columns
from rich.panel import Panel
from rich.table import Table
from rich.text import Text

from tui_core.models import PanelData
from tui_core.panels import border_for, panel_from_table

LANE_BORDER = {
    "ok": "green",
    "full": "yellow",
    "over": "red",
    "unbounded": "cyan",
}


def _lane_title(name: str) -> str:
    return name.replace("_", " ").title()


def _lane_panel(item: dict) -> Panel:
    lane_name = str(item.get("column", "-"))
    wip = str(item.get("wip", "0/-"))
    state = str(item.get("wip_state", "ok"))
    border = LANE_BORDER.get(state, "green")
    tasks = item.get("tasks") or item.get("top_tasks") or []
    if not isinstance(tasks, list):
        tasks = []

    content = Table.grid(expand=True)
    content.add_column()

    if not tasks:
        content.add_row(Text("No active tasks", style="dim"))
    else:
        shown = [str(task) for task in tasks[:5]]
        for task in shown:
            content.add_row(Text(f"- {task}", no_wrap=True, overflow="ellipsis"))
        remaining = len(tasks) - len(shown)
        if remaining > 0:
            content.add_row(Text(f"+{remaining} more", style="dim"))

    return Panel(
        content,
        title=f"[bold]{_lane_title(lane_name)}[/bold] [dim]{wip}[/dim]",
        border_style=border,
        padding=(0, 1),
        width=34,
    )


def render(data: PanelData):
    if not data.items:
        table = Table(box=None, expand=True)
        table.add_column("Column", overflow="fold", no_wrap=True)
        table.add_column("WIP", justify="right", no_wrap=True)
        table.add_column("Top Tasks", overflow="fold")
        table.add_row("no columns", "0/-", "No active tasks")
        title = f"Kanban ({data.meta.get('total_tasks', 0)} tasks)"
        return panel_from_table(title, data.status, table)

    lanes = [_lane_panel(item) for item in data.items]
    board = Columns(lanes, equal=True, expand=True, padding=(0, 1))
    title = f"[bold]Kanban ({data.meta.get('total_tasks', 0)} tasks)[/bold]"
    return Panel(board, title=title, border_style=border_for(data.status))
