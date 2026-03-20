"""Agents DataTable widget."""
from textual.widgets import DataTable
from ..models import PanelData


class AgentsTable(DataTable):
    def on_mount(self) -> None:
        self.cursor_type = "row"
        self.add_columns("Agent", "Role", "State", "Task")
        self.border_title = "Agents (0)"

    def update_data(self, data: PanelData) -> None:
        self.clear()
        for item in data.items:
            self.add_row(
                item.get("name", ""),
                item.get("role", ""),
                item.get("state", ""),
                item.get("task_id", ""),
                key=item.get("name", str(id(item))),
            )
        busy = data.meta.get("busy", 0)
        total = data.meta.get("count", 0)
        self.border_title = f"Agents ({busy}/{total} busy)"
