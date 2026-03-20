"""Approvals DataTable widget."""
from textual.widgets import DataTable


class ApprovalsTable(DataTable):
    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self._approvals: list[dict] = []

    def on_mount(self) -> None:
        self.cursor_type = "row"
        self.add_columns("Task", "Risk", "Status", "Created")
        self.border_title = "Approvals (0)"

    def update_data(self, approvals: list[dict] | None) -> None:
        self.clear()
        self._approvals = approvals or []
        for a in self._approvals:
            self.add_row(
                a.get("task_id", ""),
                f"{a.get('risk_score', 0):.2f}",
                a.get("status", ""),
                a.get("created_at", "")[:19],
                key=a.get("id", str(id(a))),
            )
        pending = sum(1 for a in self._approvals if a.get("status") == "pending")
        self.border_title = f"Approvals ({pending} pending)"

    def get_selected_task_id(self) -> str | None:
        if self.cursor_row is not None and self.cursor_row < len(self._approvals):
            return self._approvals[self.cursor_row].get("task_id")
        return None
