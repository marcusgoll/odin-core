"""Odin Overseer TUI — Textual interactive dashboard."""

from __future__ import annotations

import os
from pathlib import Path

from textual.app import App, ComposeResult
from textual.containers import Horizontal, Vertical
from textual.widgets import Header, Footer, Static, Input, DataTable
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
from .widgets import InboxTable, AgentsTable, EventLog, ApprovalsTable, DetailModal, OdinHeader
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
        self._inbox_data: PanelData | None = None
        self._agents_data: PanelData | None = None
        self._health_data: PanelData | None = None

    def compose(self) -> ComposeResult:
        yield Header()
        yield OdinHeader(id="odin-header")
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
        self._inbox_data = inbox_data
        self._agents_data = agents_data
        self._health_data = health_data

        self.query_one("#inbox-table", InboxTable).update_data(inbox_data)
        self.query_one("#agents-table", AgentsTable).update_data(agents_data)
        self.query_one("#event-log", EventLog).update_data(logs_data)
        self.query_one("#approvals-table", ApprovalsTable).update_data(approvals)

        # Update header subtitle with orchestrator health
        if health_data.items:
            health = health_data.items[0].get("value", "unknown")
            heartbeat = health_data.items[1].get("value", "n/a") if len(health_data.items) > 1 else "n/a"
            self.sub_title = f"Health: {health} | Heartbeat: {heartbeat}"

        # Update status header bar
        self.query_one("#odin-header", OdinHeader).update_data(
            health_data, agents_data, inbox_data
        )

    # ── Row selection → detail modals ──────────────────────────────

    def on_data_table_row_selected(self, event: DataTable.RowSelected) -> None:
        """Open a detail modal when Enter is pressed on a table row."""
        table = event.data_table

        if isinstance(table, InboxTable):
            self._show_inbox_detail(event)
        elif isinstance(table, AgentsTable):
            self._show_agent_detail(event)
        elif isinstance(table, ApprovalsTable):
            self._show_approval_detail(event)

    def _show_inbox_detail(self, event: DataTable.RowSelected) -> None:
        """Show task detail modal for the selected inbox row."""
        if not self._inbox_data:
            return
        row_idx = event.cursor_row
        if row_idx < 0 or row_idx >= len(self._inbox_data.items):
            return
        task = self._inbox_data.items[row_idx]
        task_id = task.get("task_id", "unknown")
        modal = DetailModal(
            title=f"Task: {task_id}",
            content=task,
            actions=[
                ("Requeue", f"requeue-{task_id}"),
                ("Cancel", f"cancel-{task_id}"),
            ],
        )
        self.push_screen(modal, callback=self._handle_inbox_action)

    def _show_agent_detail(self, event: DataTable.RowSelected) -> None:
        """Show agent detail modal for the selected agent row."""
        if not self._agents_data:
            return
        row_idx = event.cursor_row
        if row_idx < 0 or row_idx >= len(self._agents_data.items):
            return
        agent = self._agents_data.items[row_idx]
        name = agent.get("name", "unknown")
        modal = DetailModal(
            title=f"Agent: {name}",
            content=agent,
            actions=[
                ("Kill", f"kill-{name}"),
                ("Restart", f"restart-{name}"),
            ],
        )
        self.push_screen(modal, callback=self._handle_agent_action)

    def _show_approval_detail(self, event: DataTable.RowSelected) -> None:
        """Show approval detail modal for the selected approval row."""
        table = self.query_one("#approvals-table", ApprovalsTable)
        row_idx = event.cursor_row
        if row_idx < 0 or row_idx >= len(table._approvals):
            return
        approval = table._approvals[row_idx]
        task_id = approval.get("task_id", "unknown")
        modal = DetailModal(
            title=f"Approval: {task_id}",
            content=approval,
            actions=[
                ("Approve", f"approve-{task_id}"),
                ("Reject", f"reject-{task_id}"),
            ],
        )
        self.push_screen(modal, callback=self._handle_approval_action)

    # ── Modal dismiss callbacks ────────────────────────────────────

    def _handle_inbox_action(self, action_id: str | None) -> None:
        """Process inbox modal action (requeue or cancel)."""
        if not action_id:
            return
        if action_id.startswith("requeue-"):
            task_id = action_id[len("requeue-"):]
            self._run_api_action("requeue", task_id, api_client.requeue_task)
        elif action_id.startswith("cancel-"):
            task_id = action_id[len("cancel-"):]
            self._run_api_action("cancel", task_id, api_client.cancel_task)

    def _handle_agent_action(self, action_id: str | None) -> None:
        """Process agent modal action (kill or restart)."""
        if not action_id:
            return
        if action_id.startswith("kill-"):
            name = action_id[len("kill-"):]
            self._run_api_action("kill", name, api_client.kill_agent)
        elif action_id.startswith("restart-"):
            name = action_id[len("restart-"):]
            self._run_api_action("restart", name, api_client.restart_agent)

    def _handle_approval_action(self, action_id: str | None) -> None:
        """Process approval modal action (approve or reject)."""
        if not action_id:
            return
        if action_id.startswith("approve-"):
            task_id = action_id[len("approve-"):]
            self._run_api_action("approve", task_id, api_client.approve_task)
        elif action_id.startswith("reject-"):
            task_id = action_id[len("reject-"):]
            self._run_api_action("reject", task_id, api_client.reject_task)

    @work(thread=True)
    def _run_api_action(self, verb: str, target: str, fn) -> None:
        """Execute an API action in a background thread and notify the user."""
        result = fn(target)
        if result is not None:
            self.call_from_thread(
                self.notify, f"{verb.capitalize()} {target}: OK", title="Action", timeout=3
            )
            # Refresh data after successful action
            self.call_from_thread(self.refresh_data)
        else:
            self.call_from_thread(
                self.notify, f"{verb.capitalize()} {target}: Failed",
                title="Action", severity="error", timeout=3,
            )

    # ── Keybinding actions ─────────────────────────────────────────

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
            "Keys: [q]ueue [a]gents [e]vents [p]approvals [:]cmd [Ctrl+C]quit | Enter: drill-down",
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
