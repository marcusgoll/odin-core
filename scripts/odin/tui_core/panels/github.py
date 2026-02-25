"""GitHub panel renderer."""

from __future__ import annotations

from rich.table import Table

from tui_core.models import PanelData
from tui_core.panels import panel_from_table


def render(data: PanelData):
    table = Table(box=None, expand=True)
    table.add_column("PR", no_wrap=True)
    table.add_column("Title", overflow="fold")
    table.add_column("Author", no_wrap=True)

    if not data.items:
        if data.errors:
            table.add_row("-", data.errors[0], "-")
        else:
            table.add_row("-", "No open PRs", "-")
    else:
        for item in data.items[:12]:
            number = item.get("number", "-")
            draft = "draft " if item.get("draft") else ""
            pr = f"#{number}"
            table.add_row(pr, f"{draft}{item.get('title', '')}", str(item.get("author", "-")))

    title = f"GitHub ({data.meta.get('open_prs', 0)} open)"
    return panel_from_table(title, data.status, table)
