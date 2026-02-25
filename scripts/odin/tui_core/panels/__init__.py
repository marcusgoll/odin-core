"""Panel rendering helpers."""

from __future__ import annotations

from rich.panel import Panel
from rich.table import Table
from rich.text import Text

from tui_core.models import PanelData

STATUS_BORDER = {
    "ok": "cyan",
    "warn": "yellow",
    "error": "red",
}


def border_for(status: str) -> str:
    return STATUS_BORDER.get(status, "cyan")


def empty_panel(title: str, message: str = "No data") -> Panel:
    return Panel(Text(message, style="dim"), title=f"[bold]{title}[/bold]", border_style="cyan")


def kv_table(rows: list[tuple[str, str]]) -> Table:
    table = Table(box=None, show_header=False, expand=True, pad_edge=False)
    table.add_column("key", style="bold")
    table.add_column("value", style="default")
    for key, value in rows:
        table.add_row(key, value)
    return table


def panel_from_table(title: str, status: str, table: Table) -> Panel:
    return Panel(table, title=f"[bold]{title}[/bold]", border_style=border_for(status))


def panel_from_text(title: str, status: str, text: str) -> Panel:
    return Panel(Text(text), title=f"[bold]{title}[/bold]", border_style=border_for(status))


def error_suffix(data: PanelData) -> str:
    if not data.errors:
        return ""
    return f" ({'; '.join(data.errors[:1])})"
