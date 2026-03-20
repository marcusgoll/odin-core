"""Odin status header widget."""
from textual.widgets import Static
from ..models import PanelData


class OdinHeader(Static):
    DEFAULT_CSS = """
    OdinHeader {
        height: 1;
        background: $accent;
        color: $text;
        text-align: center;
        text-style: bold;
    }
    """

    def update_data(self, health, agents_data, inbox_data):
        status = health.status if health else "unknown"
        icon = "OK" if status == "ok" else "WARN" if status == "warn" else "ERR"
        agent_count = agents_data.meta.get("count", 0) if agents_data else 0
        busy = agents_data.meta.get("busy", 0) if agents_data else 0
        pending = inbox_data.meta.get("pending", 0) if inbox_data else 0

        self.update(
            f" [{status.upper()}] {icon} | "
            f"Agents: {busy}/{agent_count} | "
            f"Queue: {pending}"
        )
