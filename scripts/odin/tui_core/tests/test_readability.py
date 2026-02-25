from __future__ import annotations

import json
import os
import tempfile
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path
import sys

from rich.columns import Columns

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from tui_core.collectors.inbox import collect as collect_inbox  # noqa: E402
from tui_core.collectors.agents import collect as collect_agents  # noqa: E402
from tui_core.collectors.kanban import collect as collect_kanban  # noqa: E402
from tui_core.collectors.logs import collect as collect_logs  # noqa: E402
from tui_core.formatting import compact_relative_age, task_label_for_type, wip_state  # noqa: E402
from tui_core.models import PanelData  # noqa: E402
from tui_core.panels.inbox import render as render_inbox  # noqa: E402
from tui_core.panels.kanban import _lane_panel, render as render_kanban  # noqa: E402
from tui_core.panels.logs import render as render_logs  # noqa: E402


class FormattingTests(unittest.TestCase):
    def test_task_label_hybrid_map(self):
        self.assertEqual(task_label_for_type("pr_review_second"), "Second PR Review")
        self.assertEqual(task_label_for_type("watchdog.pr_health.poll"), "Watchdog PR Health Poll")

    def test_compact_relative_age(self):
        self.assertEqual(compact_relative_age(12), "12s ago")
        self.assertEqual(compact_relative_age(180), "3m ago")
        self.assertEqual(compact_relative_age(7200), "2h ago")

    def test_wip_state(self):
        self.assertEqual(wip_state(2, 0), "unbounded")
        self.assertEqual(wip_state(3, 5), "ok")
        self.assertEqual(wip_state(5, 5), "full")
        self.assertEqual(wip_state(6, 5), "over")


class CollectorReadabilityTests(unittest.TestCase):
    def test_agents_collect_prefers_status_field(self):
        with tempfile.TemporaryDirectory() as tmp:
            odin_dir = Path(tmp)
            agents_dir = odin_dir / "agents" / "sm"
            agents_dir.mkdir(parents=True)
            (agents_dir / "status.json").write_text(
                json.dumps(
                    {
                        "name": "sm",
                        "role": "sm",
                        "status": "busy",
                        "current_task": "cron-1",
                    }
                )
            )
            (odin_dir / "state.json").write_text(
                json.dumps({"dispatched_tasks": {"cron-1": {"agent": "sm"}}})
            )
            data = collect_agents(odin_dir)
            self.assertEqual(data.items[0]["state"], "busy")

    def test_agents_collect_prefers_newest_created_at_task_for_agent(self):
        with tempfile.TemporaryDirectory() as tmp:
            odin_dir = Path(tmp)
            agents_dir = odin_dir / "agents" / "sm"
            agents_dir.mkdir(parents=True)
            (agents_dir / "status.json").write_text(json.dumps({"name": "sm", "role": "sm"}))
            newer = (datetime.now(timezone.utc) - timedelta(minutes=3)).isoformat()
            older = (datetime.now(timezone.utc) - timedelta(minutes=10)).isoformat()
            (odin_dir / "state.json").write_text(
                json.dumps(
                    {
                        "dispatched_tasks": {
                            "task-newer": {"agent": "sm", "created_at": newer},
                            "task-older": {"agent": "sm", "created_at": older},
                        }
                    }
                )
            )
            data = collect_agents(odin_dir)
            self.assertEqual(data.items[0]["task"], "task-newer")
            self.assertEqual(data.items[0]["state"], "busy")

    def test_agents_collect_falls_back_to_unknown_without_dispatch_or_state(self):
        with tempfile.TemporaryDirectory() as tmp:
            odin_dir = Path(tmp)
            agents_dir = odin_dir / "agents" / "sm"
            agents_dir.mkdir(parents=True)
            (agents_dir / "status.json").write_text(json.dumps({"name": "sm", "role": "sm"}))
            data = collect_agents(odin_dir)
            self.assertEqual(data.items[0]["task"], "-")
            self.assertEqual(data.items[0]["state"], "unknown")

    def test_inbox_collect_has_readable_task_label(self):
        with tempfile.TemporaryDirectory() as tmp:
            odin_dir = Path(tmp)
            inbox_dir = odin_dir / "inbox"
            inbox_dir.mkdir(parents=True)
            (inbox_dir / "task-1.json").write_text(
                json.dumps(
                    {
                        "task_id": "watchdog-poll-pr-health-1700000000",
                        "type": "watchdog_poll",
                        "payload": {"task_type": "watchdog.pr_health.poll"},
                        "source": "keepalive",
                    }
                )
            )
            data = collect_inbox(odin_dir)
            self.assertTrue(data.items)
            self.assertIn("task_label", data.items[0])
            self.assertEqual(data.items[0]["task_label"], "Watchdog PR Health Poll")

    def test_inbox_age_prefers_created_at_over_file_mtime(self):
        with tempfile.TemporaryDirectory() as tmp:
            odin_dir = Path(tmp)
            inbox_dir = odin_dir / "inbox"
            inbox_dir.mkdir(parents=True)
            old_created = (datetime.now(timezone.utc) - timedelta(hours=6, minutes=5)).isoformat()
            task_file = inbox_dir / "task-2.json"
            task_file.write_text(
                json.dumps(
                    {
                        "task_id": "dispatch-work-1700000000",
                        "type": "dispatch_work",
                        "source": "cron",
                        "created_at": old_created,
                    }
                )
            )
            now = datetime.now().timestamp()
            os.utime(task_file, (now, now))
            data = collect_inbox(odin_dir)
            self.assertTrue(data.items)
            self.assertIn("h ago", data.items[0]["age"])

    def test_kanban_collect_has_wip_and_top_tasks(self):
        with tempfile.TemporaryDirectory() as tmp:
            odin_dir = Path(tmp)
            kanban_dir = odin_dir / "kanban"
            kanban_dir.mkdir(parents=True)
            (kanban_dir / "board.json").write_text(
                json.dumps(
                    {
                        "columns": {
                            "in_progress": {
                                "wip_limit": 3,
                                "tasks": [
                                    {"title": "Fix flaky test"},
                                    {"task_type": "pr_review_second"},
                                    {"task_type": "watchdog.pr_health.poll"},
                                ],
                            }
                        }
                    }
                )
            )
            data = collect_kanban(odin_dir)
            self.assertTrue(data.items)
            row = data.items[0]
            self.assertIn("wip", row)
            self.assertIn("wip_state", row)
            self.assertIn("top_tasks", row)
            self.assertIn("tasks", row)
            self.assertTrue(row["top_tasks"])

    def test_logs_collect_streams_with_timestamp_not_relative_age(self):
        with tempfile.TemporaryDirectory() as tmp:
            odin_dir = Path(tmp)
            log_dir = odin_dir / "logs" / datetime.now().strftime("%Y-%m-%d")
            log_dir.mkdir(parents=True)
            older = (datetime.now(timezone.utc) - timedelta(seconds=75)).isoformat()
            newer = (datetime.now(timezone.utc) - timedelta(seconds=30)).isoformat()
            (log_dir / "events.jsonl").write_text(
                json.dumps({"ts": newer, "event_type": "task.received", "message": "queued"}) + "\n"
            )
            (log_dir / "alerts.log").write_text(
                f"[{older}] RECEIVED n8n_failure severity=warning project=cfipros\n"
            )
            data = collect_logs(odin_dir)
            self.assertTrue(data.items)
            self.assertIn("time", data.items[0])
            self.assertNotIn("ago", data.items[0])
            self.assertEqual(data.items[-1]["message"], "task.received: queued")
            self.assertNotEqual(data.items[-1]["time"], "n/a")


class PanelReadabilityTests(unittest.TestCase):
    def test_inbox_panel_uses_three_columns(self):
        panel = render_inbox(
            PanelData(
                key="inbox",
                title="Inbox",
                status="ok",
                items=[{"task_label": "Dispatch Work", "type": "dispatch_work", "source": "cron", "age": "10s ago"}],
                meta={"pending": 1},
                errors=[],
            )
        )
        headers = [column.header for column in panel.renderable.columns]
        self.assertEqual(headers, ["Task", "Source", "Age"])

    def test_logs_panel_uses_time_column(self):
        panel = render_logs(
            PanelData(
                key="logs",
                title="Logs",
                status="ok",
                items=[{"time": "15:30:00", "source": "events.jsonl", "message": "task.received: queued"}],
                meta={},
                errors=[],
            )
        )
        headers = [column.header for column in panel.renderable.columns]
        self.assertEqual(headers, ["Time", "Source", "Message"])

    def test_kanban_panel_renders_lane_columns(self):
        panel = render_kanban(
            PanelData(
                key="kanban",
                title="Kanban",
                status="ok",
                items=[
                    {"column": "ready", "wip": "1/3", "wip_state": "ok", "tasks": ["Prepare release notes"]},
                    {"column": "in_progress", "wip": "2/4", "wip_state": "ok", "tasks": ["Fix flaky test"]},
                ],
                meta={"total_tasks": 3},
                errors=[],
            )
        )
        self.assertIsInstance(panel.renderable, Columns)

    def test_kanban_lane_panel_not_fixed_width(self):
        lane = _lane_panel({"column": "ready", "wip": "1/4", "wip_state": "ok", "tasks": ["Task"]})
        self.assertIsNone(lane.width)


if __name__ == "__main__":
    unittest.main()
