"""Event stream Log widget."""
from textual.widgets import Log
from ..models import PanelData


class EventLog(Log):
    def __init__(self, **kwargs):
        super().__init__(highlight=True, max_lines=500, **kwargs)
        self._seen: dict[str, None] = {}

    def update_data(self, data: PanelData) -> None:
        for item in data.items:
            key = f"{item.get('time', '')}-{item.get('message', '')}"
            if key not in self._seen:
                self._seen[key] = None
                ts = item.get("time", "")
                src = item.get("source", "")
                msg = item.get("message", "")
                self.write_line(f"[{ts}] [{src}] {msg}")
        if len(self._seen) > 1000:
            self._seen = dict(list(self._seen.items())[-500:])
        self.border_title = f"Events ({data.meta.get('shown', 0)})"
