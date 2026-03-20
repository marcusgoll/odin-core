"""Approvals DataTable widget."""
from textual.widgets import DataTable
from ..models import PanelData


class ApprovalsTable(DataTable):
    def on_mount(self) -> None:
        self.cursor_type = "row"
        self.add_columns("Task", "Risk", "Status", "Created")
        self.border_title = "Approvals (0)"

    def update_data(self, data: PanelData) -> None:
        self.clear()
        for a in data.items:
            self.add_row(
                a.get("task_id", ""),
                f"{float(a.get('risk_score', 0) or 0):.2f}",
                a.get("status", ""),
                str(a.get("created_at") or "")[:19],
                key=a.get("id", str(id(a))),
            )
        pending = sum(1 for a in data.items if a.get("status") == "pending")
        self.border_title = f"Approvals ({pending} pending)"
