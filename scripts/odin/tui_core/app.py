"""Odin Overseer TUI — Textual interactive dashboard."""

from __future__ import annotations

import os
from pathlib import Path

from textual.app import App, ComposeResult
from textual.containers import Horizontal, Vertical
from textual.widgets import Header, Footer, Static, DataTable, Log, Input
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
        self._log_seen: set[str] = set()

    def compose(self) -> ComposeResult:
        yield Header()
        with Horizontal(id="main"):
            with Vertical(id="left", classes="column"):
                yield DataTable(id="inbox-table")
                yield DataTable(id="agents-table")
            with Vertical(id="right", classes="column"):
                yield Log(id="event-log", highlight=True, max_lines=500)
                yield DataTable(id="approvals-table")
        yield Input(id="command-bar", placeholder="Type command...")
        yield Footer()

    def on_mount(self) -> None:
        """Set up tables and start polling."""
        inbox_tbl = self.query_one("#inbox-table", DataTable)
        inbox_tbl.add_columns("Task", "Type", "Source", "Age")
        inbox_tbl.cursor_type = "row"
        inbox_tbl.border_title = "Inbox"

        agents_tbl = self.query_one("#agents-table", DataTable)
        agents_tbl.add_columns("Agent", "Role", "State", "Task")
        agents_tbl.cursor_type = "row"
        agents_tbl.border_title = "Agents"

        approvals_tbl = self.query_one("#approvals-table", DataTable)
        approvals_tbl.add_columns("Task", "Risk", "Status", "Created")
        approvals_tbl.cursor_type = "row"
        approvals_tbl.border_title = "Approvals"

        event_log = self.query_one("#event-log", Log)
        event_log.border_title = "Events"

        # Initial data load + periodic refresh
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
        self.call_from_thread(
            self._update_panels, inbox_data, agents_data, logs_data, health_data
        )

    def _update_panels(
        self,
        inbox_data: PanelData,
        agents_data: PanelData,
        logs_data: PanelData,
        health_data: PanelData,
    ) -> None:
        """Update all panels with freshly collected data (runs on UI thread)."""
        # Update inbox table
        tbl = self.query_one("#inbox-table", DataTable)
        tbl.clear()
        for item in inbox_data.items:
            tbl.add_row(
                item.get("task_id", ""),
                item.get("task_label", item.get("type", "")),
                item.get("source", ""),
                item.get("age", ""),
            )
        tbl.border_title = f"Inbox ({inbox_data.meta.get('pending', 0)})"

        # Update agents table
        tbl = self.query_one("#agents-table", DataTable)
        tbl.clear()
        for item in agents_data.items:
            tbl.add_row(
                item.get("name", ""),
                item.get("role", ""),
                item.get("state", ""),
                item.get("task", ""),
            )
        busy = agents_data.meta.get("busy", 0)
        total = agents_data.meta.get("count", 0)
        tbl.border_title = f"Agents ({busy}/{total} busy)"

        # Update event log — append only new lines to avoid duplication
        log = self.query_one("#event-log", Log)
        for item in logs_data.items:
            ts = item.get("time", "")
            src = item.get("source", "")
            msg = item.get("message", "")
            line_key = f"{item.get('ts', '')}|{src}|{msg}"
            if line_key not in self._log_seen:
                self._log_seen.add(line_key)
                log.write_line(f"[{ts}] [{src}] {msg}")

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
        """Handle command bar submission."""
        cmd = event.value.strip()
        command_bar = self.query_one("#command-bar", Input)
        command_bar.value = ""
        command_bar.display = False
        if cmd:
            self.notify(f"Command: {cmd}", title="Command", timeout=3)


def main():
    """Entry point for the Textual TUI."""
    app = OdinTUI()
    app.run()


if __name__ == "__main__":
    main()
