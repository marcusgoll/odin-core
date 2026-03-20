"""Odin Overseer TUI — Textual interactive dashboard."""

from __future__ import annotations

import os
from pathlib import Path

from textual.app import App, ComposeResult
from textual.containers import Horizontal, Vertical
from textual.widgets import Header, Footer, Static, Input
from textual.worker import Worker
from textual import work

from .collectors import env_odin_dir
from .collectors.orchestrator import collect as collect_orchestrator
from .collectors.agents import collect as collect_agents
from .collectors.inbox import collect as collect_inbox
from .collectors.logs import collect as collect_logs
from .collectors.kanban import collect as collect_kanban
from .collectors.github import collect as collect_github
from .models import PanelData
from .widgets import InboxTable, AgentsTable, EventLog, ApprovalsTable
from . import api_client


class OdinTUI(App):
    """Odin Overseer — interactive terminal dashboard."""

    CSS_PATH = "styles.tcss"
    TITLE = "Odin Overseer"
    BINDINGS = [
        ("q", "focus_queue", "Queue"),
        ("a", "focus_agents", "Agents"),
        ("e", "focus_events", "Events"),
        ("p", "focus_approvals", "Approvals"),
        ("colon", "focus_command", "Command"),
        ("question_mark", "show_help", "Help"),
        ("ctrl+c", "quit", "Quit"),
    ]

    def __init__(self, odin_dir: Path | None = None, **kwargs):
        super().__init__(**kwargs)
        self._odin_dir = odin_dir or env_odin_dir()

    def compose(self) -> ComposeResult:
        yield Header()
        with Horizontal(id="main"):
            with Vertical(id="left", classes="column"):
                yield InboxTable(id="inbox-table")
                yield AgentsTable(id="agents-table")
            with Vertical(id="right", classes="column"):
                yield EventLog(id="event-log")
                yield ApprovalsTable(id="approvals-table")
        yield Input(id="command-bar", placeholder="Type command...")
        yield Footer()

    def on_mount(self) -> None:
        """Start polling — widgets handle their own column setup."""
        self.refresh_data()
        self.set_interval(5.0, self.refresh_data)

    @work(thread=True)
    def refresh_data(self) -> None:
        """Collect data from all sources in a background thread."""
        odin_dir = self._odin_dir
        inbox_data = collect_inbox(odin_dir)
        agents_data = collect_agents(odin_dir)
        logs_data = collect_logs(odin_dir)
        health_data = collect_orchestrator(odin_dir)
        approvals = api_client.fetch_approvals()
        self.call_from_thread(
            self._update_panels, inbox_data, agents_data, logs_data, health_data, approvals
        )

    def _update_panels(
        self,
        inbox_data: PanelData,
        agents_data: PanelData,
        logs_data: PanelData,
        health_data: PanelData,
        approvals: list[dict],
    ) -> None:
        """Update all panels with freshly collected data (runs on UI thread)."""
        self.query_one("#inbox-table", InboxTable).update_data(inbox_data)
        self.query_one("#agents-table", AgentsTable).update_data(agents_data)
        self.query_one("#event-log", EventLog).update_data(logs_data)
        self.query_one("#approvals-table", ApprovalsTable).update_data(approvals)

        # Update header subtitle with orchestrator health
        if health_data.items:
            health = health_data.items[0].get("value", "unknown")
            heartbeat = health_data.items[1].get("value", "n/a") if len(health_data.items) > 1 else "n/a"
            self.sub_title = f"Health: {health} | Heartbeat: {heartbeat}"

    def action_focus_queue(self) -> None:
        """Focus the inbox table."""
        self.query_one("#inbox-table").focus()

    def action_focus_agents(self) -> None:
        """Focus the agents table."""
        self.query_one("#agents-table").focus()

    def action_focus_events(self) -> None:
        """Focus the event log."""
        self.query_one("#event-log").focus()

    def action_focus_approvals(self) -> None:
        """Focus the approvals table."""
        self.query_one("#approvals-table").focus()

    def action_focus_command(self) -> None:
        """Show and focus the command bar."""
        cmd = self.query_one("#command-bar", Input)
        cmd.display = True
        cmd.focus()

    def action_show_help(self) -> None:
        """Display help overlay (placeholder)."""
        self.notify(
            "Keys: [q]ueue [a]gents [e]vents [p]approvals [:]cmd [Ctrl+C]quit",
            title="Help",
            timeout=5,
        )

    def on_input_submitted(self, event: Input.Submitted) -> None:
        """Handle command bar submission — dispatch to engine API."""
        cmd = event.value.strip()
        command_bar = self.query_one("#command-bar", Input)
        command_bar.value = ""
        command_bar.display = False
        if cmd:
            self._dispatch_command(cmd)

    @work(thread=True)
    def _dispatch_command(self, cmd: str) -> None:
        """Send command to engine API in a background thread."""
        result = api_client.send_command(cmd)
        if result:
            self.call_from_thread(
                self.notify, f"OK: {cmd}", title="Command", timeout=3
            )
        else:
            self.call_from_thread(
                self.notify, f"Failed: {cmd}", title="Command", severity="error", timeout=3
            )


def main():
    """Entry point for the Textual TUI."""
    app = OdinTUI()
    app.run()


if __name__ == "__main__":
    main()
