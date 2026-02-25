from __future__ import annotations

import json
import tempfile
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from tui_core.collectors.inbox import collect as collect_inbox  # noqa: E402
from tui_core.collectors.kanban import collect as collect_kanban  # noqa: E402
from tui_core.collectors.logs import collect as collect_logs  # noqa: E402
from tui_core.formatting import compact_relative_age, task_label_for_type, wip_state  # noqa: E402


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
            self.assertTrue(row["top_tasks"])

    def test_logs_collect_has_relative_time(self):
        with tempfile.TemporaryDirectory() as tmp:
            odin_dir = Path(tmp)
            log_dir = odin_dir / "logs" / datetime.now().strftime("%Y-%m-%d")
            log_dir.mkdir(parents=True)
            ts = (datetime.now(timezone.utc) - timedelta(seconds=75)).isoformat()
            (log_dir / "events.jsonl").write_text(
                json.dumps({"ts": ts, "event_type": "task.received", "message": "queued"}) + "\n"
            )
            data = collect_logs(odin_dir)
            self.assertTrue(data.items)
            self.assertIn("ago", data.items[0])
            self.assertTrue(data.items[0]["ago"].endswith("ago"))


if __name__ == "__main__":
    unittest.main()
