"""Detail modal for task and agent drill-down."""
from textual.screen import ModalScreen
from textual.widgets import Static, Button
from textual.containers import Vertical, Horizontal
import json


class DetailModal(ModalScreen):
    CSS = """
    #detail-modal {
        width: 60%;
        height: 80%;
        background: $surface;
        border: thick $accent;
        padding: 1 2;
    }
    #detail-body {
        height: 1fr;
        overflow-y: auto;
    }
    #detail-actions {
        height: auto;
        dock: bottom;
        padding-top: 1;
    }
    """

    def __init__(self, title, content, actions=None):
        super().__init__()
        self.detail_title = title
        self.content = content
        self.detail_actions = actions or []

    def compose(self):
        with Vertical(id="detail-modal"):
            yield Static(f"[bold]{self.detail_title}[/bold]")
            yield Static(json.dumps(self.content, indent=2, default=str), id="detail-body")
            if self.detail_actions:
                with Horizontal(id="detail-actions"):
                    for label, action_id in self.detail_actions:
                        yield Button(label, id=action_id, variant="primary" if "approve" in action_id.lower() else "default")
                    yield Button("Close", id="close-modal", variant="error")
            else:
                yield Button("Close", id="close-modal", variant="error")

    def on_button_pressed(self, event):
        if event.button.id == "close-modal":
            self.dismiss(None)
        else:
            self.dismiss(event.button.id)
