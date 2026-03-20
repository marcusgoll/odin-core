"""Odin Overseer TUI — Textual interactive dashboard."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Callable

from textual.app import App, ComposeResult
from textual.containers import Horizontal, Vertical
from textual.events import Key
from textual.widgets import Header, Footer, Input, DataTable
from textual import work

from .collectors import env_odin_dir
from .collectors.orchestrator import collect as collect_orchestrator
from .collectors.agents import collect as collect_agents
from .collectors.inbox import collect as collect_inbox
from .collectors.logs import collect as collect_logs
from .models import PanelData
from .widgets import InboxTable, AgentsTable, EventLog, ApprovalsTable, DetailModal, OdinHeader, BudgetPanel
from . import api_client

# ── Data-driven detail modal configuration ─────────────────────────
_DETAIL_CONFIGS: dict[str, dict[str, Any]] = {
    "inbox-table": {
        "data_attr": "_inbox_data",
        "id_key": "task_id",
        "title_fmt": "Task: {}",
        "actions": [("Requeue", "requeue"), ("Cancel", "cancel")],
        "api_map": {
            "requeue": api_client.requeue_task,
            "cancel": api_client.cancel_task,
        },
    },
    "agents-table": {
        "data_attr": "_agents_data",
        "id_key": "name",
        "title_fmt": "Agent: {}",
        "actions": [("Kill", "kill"), ("Restart", "restart")],
        "api_map": {
            "kill": api_client.kill_agent,
            "restart": api_client.restart_agent,
        },
    },
    "approvals-table": {
        "data_attr": "_approvals_data",
        "id_key": "task_id",
        "title_fmt": "Approval: {}",
        "actions": [("Approve", "approve"), ("Reject", "reject")],
        "api_map": {
            "approve": api_client.approve_task,
            "reject": api_client.reject_task,
        },
    },
}


HISTORY_FILE = Path.home() / ".odin-tui-history"


class CommandHistory:
    """File-backed command history (max 50 entries)."""

    def __init__(self) -> None:
        self.entries: list[str] = []
        self.index = -1
        self._load()

    def _load(self) -> None:
        try:
            self.entries = HISTORY_FILE.read_text().strip().splitlines()[-50:]
        except OSError:
            self.entries = []

    def add(self, cmd: str) -> None:
        if cmd and (not self.entries or self.entries[-1] != cmd):
            self.entries.append(cmd)
            self.entries = self.entries[-50:]
            try:
                HISTORY_FILE.write_text("\n".join(self.entries) + "\n")
            except OSError:
                pass
        self.index = -1

    def up(self) -> str | None:
        if not self.entries:
            return None
        if self.index < len(self.entries) - 1:
            self.index += 1
        return self.entries[-(self.index + 1)]

    def down(self) -> str | None:
        if self.index > 0:
            self.index -= 1
            return self.entries[-(self.index + 1)]
        self.index = -1
        return ""


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
        self._approvals_data: PanelData | None = None
        self._health_data: PanelData | None = None
        self._cmd_history = CommandHistory()

    def compose(self) -> ComposeResult:
        yield Header()
        yield OdinHeader(id="odin-header")
        yield BudgetPanel(id="budget-panel")
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

    @work(thread=True, exclusive=True)
    def refresh_data(self) -> None:
        """Collect data from all sources in a background thread."""
        odin_dir = self._odin_dir
        inbox_data = collect_inbox(odin_dir)
        agents_data = collect_agents(odin_dir)
        logs_data = collect_logs(odin_dir)
        health_data = collect_orchestrator(odin_dir)
        raw_approvals = api_client.fetch_approvals()
        approvals_data = PanelData(
            key="approvals",
            title="Approvals",
            status="ok" if raw_approvals else "warn",
            items=raw_approvals,
            meta={"pending": sum(1 for a in raw_approvals if a.get("status") == "pending")},
        )
        budget_data = api_client.fetch_budgets()
        self.call_from_thread(
            self._update_panels, inbox_data, agents_data, logs_data, health_data, approvals_data, budget_data
        )

    def _update_panels(
        self,
        inbox_data: PanelData,
        agents_data: PanelData,
        logs_data: PanelData,
        health_data: PanelData,
        approvals_data: PanelData,
        budget_data: dict | None = None,
    ) -> None:
        """Update all panels with freshly collected data (runs on UI thread)."""
        self._inbox_data = inbox_data
        self._agents_data = agents_data
        self._approvals_data = approvals_data
        self._health_data = health_data

        self.query_one("#inbox-table", InboxTable).update_data(inbox_data)
        self.query_one("#agents-table", AgentsTable).update_data(agents_data)
        self.query_one("#event-log", EventLog).update_data(logs_data)
        self.query_one("#approvals-table", ApprovalsTable).update_data(approvals_data)
        self.query_one("#budget-panel", BudgetPanel).update_data(budget_data)

        # Update status header bar (sole owner of health display)
        self.query_one("#odin-header", OdinHeader).update_data(
            health_data, agents_data, inbox_data
        )

    # ── Row selection → detail modals (data-driven) ────────────────

    def on_data_table_row_selected(self, event: DataTable.RowSelected) -> None:
        """Open a detail modal when Enter is pressed on a table row."""
        table = event.data_table
        table_id = table.id
        config = _DETAIL_CONFIGS.get(table_id) if table_id else None
        if not config:
            return

        data: PanelData | None = getattr(self, config["data_attr"], None)
        if not data:
            return

        row_idx = event.cursor_row
        if row_idx < 0 or row_idx >= len(data.items):
            return

        item = data.items[row_idx]
        item_id = item.get(config["id_key"], "unknown")
        title = config["title_fmt"].format(item_id)
        actions = [(label, f"{verb}-{item_id}") for label, verb in config["actions"]]
        api_map: dict[str, Callable] = config["api_map"]

        modal = DetailModal(title=title, content=item, actions=actions)

        def _on_dismiss(action_id: str | None) -> None:
            if not action_id:
                return
            for verb, fn in api_map.items():
                prefix = f"{verb}-"
                if action_id.startswith(prefix):
                    target = action_id[len(prefix):]
                    self._run_api_action(verb, target, fn)
                    return

        self.push_screen(modal, callback=_on_dismiss)

    # ── API action runner ──────────────────────────────────────────

    @work(thread=True)
    def _run_api_action(self, verb: str, target: str, fn: Callable) -> None:
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
            self._cmd_history.add(cmd)
            self._dispatch_command(cmd)

    def on_key(self, event: Key) -> None:
        """Handle up/down arrow keys for command history navigation."""
        command_bar = self.query_one("#command-bar", Input)
        if command_bar.has_focus and command_bar.display:
            if event.key == "up":
                val = self._cmd_history.up()
                if val is not None:
                    command_bar.value = val
                    command_bar.cursor_position = len(val)
                event.prevent_default()
            elif event.key == "down":
                val = self._cmd_history.down()
                if val is not None:
                    command_bar.value = val
                    command_bar.cursor_position = len(val)
                event.prevent_default()

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
