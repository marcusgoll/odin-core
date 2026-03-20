"""Inbox queue DataTable widget."""
from textual.widgets import DataTable
from ..models import PanelData


class InboxTable(DataTable):
    def on_mount(self) -> None:
        self.cursor_type = "row"
        self.add_columns("Task", "Type", "Source", "Age")
        self.border_title = "Queue (0)"

    def update_data(self, data: PanelData) -> None:
        self.clear()
        for item in data.items:
            self.add_row(
                item.get("task_id", ""),
                item.get("task_label", item.get("type", "")),
                item.get("source", ""),
                item.get("age", ""),
                key=item.get("task_id", str(id(item))),
            )
        self.border_title = f"Queue ({data.meta.get('pending', 0)})"
