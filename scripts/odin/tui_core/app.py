"""Modular TUI application entrypoint for odin-core."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

from rich.console import Console, Group
from rich.layout import Layout
from rich.live import Live
from rich.panel import Panel

from tui_core.collectors import env_odin_dir
from tui_core.collectors.agents import collect as collect_agents
from tui_core.collectors.github import collect as collect_github
from tui_core.collectors.inbox import collect as collect_inbox
from tui_core.collectors.kanban import collect as collect_kanban
from tui_core.collectors.logs import collect as collect_logs
from tui_core.collectors.orchestrator import collect as collect_orchestrator
from tui_core.layout import select_layout_mode
from tui_core.models import PanelData
from tui_core.panels.agents import render as render_agents
from tui_core.panels.github import render as render_github
from tui_core.panels.header import render as render_header
from tui_core.panels.inbox import render as render_inbox
from tui_core.panels.kanban import render as render_kanban
from tui_core.panels.logs import render as render_logs
from tui_core.profiles import resolve_profile

PANEL_RENDERERS = {
    "inbox": render_inbox,
    "kanban": render_kanban,
    "agents": render_agents,
    "logs": render_logs,
    "github": render_github,
}


def _legacy_script_path() -> Path:
    return Path(__file__).resolve().parents[1] / "odin-tui-legacy.py"


def _run_legacy(args: argparse.Namespace) -> int:
    cmd = [sys.executable, str(_legacy_script_path())]
    if args.live:
        cmd.append("--live")
    if args.json:
        cmd.append("--json")
    env = os.environ.copy()
    if args.odin_dir:
        env["ODIN_DIR"] = args.odin_dir
    completed = subprocess.run(cmd, env=env, check=False)
    return completed.returncode


def _collect_core(odin_dir: Path) -> dict[str, PanelData]:
    return {
        "orchestrator": collect_orchestrator(odin_dir),
        "inbox": collect_inbox(odin_dir),
        "kanban": collect_kanban(odin_dir),
        "agents": collect_agents(odin_dir),
        "logs": collect_logs(odin_dir),
        "github": collect_github(odin_dir),
    }


def _render_core(data: dict[str, PanelData], profile: dict, width: int, height: int):
    mode = select_layout_mode(width)
    panels = profile.get("panels", [])
    orchestrator = data["orchestrator"]

    header = render_header(
        profile_name=profile["name"],
        heartbeat=str(orchestrator.items[1]["value"] if len(orchestrator.items) > 1 else "n/a"),
        pending=int(data["inbox"].meta.get("pending", 0)),
        total_agents=int(data["agents"].meta.get("count", 0)),
        layout_mode=mode,
    )

    panel_map: dict[str, Panel] = {}
    for panel_key in panels:
        renderer = PANEL_RENDERERS.get(panel_key)
        if renderer is None:
            continue
        panel_map[panel_key] = renderer(data[panel_key])

    def p(key: str) -> Panel:
        return panel_map.get(key, Panel("disabled", title=key))

    if mode == "narrow":
        ordered = [header] + [p(name) for name in panels if name in panel_map]
        return Group(*ordered)

    layout = Layout()
    layout.split_column(
        Layout(header, name="header", size=4),
        Layout(name="body"),
    )

    if mode == "medium":
        layout["body"].split_row(
            Layout(Group(p("inbox"), p("kanban")), name="left", ratio=2),
            Layout(Group(p("agents"), p("logs"), p("github")), name="right", ratio=2),
        )
        return layout

    # wide
    layout["body"].split_column(
        Layout(name="middle", ratio=2),
        Layout(name="bottom", ratio=2),
    )
    layout["body"]["middle"].split_row(
        Layout(p("inbox"), name="inbox", ratio=2),
        Layout(p("kanban"), name="kanban", ratio=2),
        Layout(p("agents"), name="agents", ratio=2),
    )
    layout["body"]["bottom"].split_row(
        Layout(p("logs"), name="logs", ratio=3),
        Layout(p("github"), name="github", ratio=1),
    )
    return layout


def _json_output(profile: dict, data: dict[str, PanelData]) -> str:
    payload = {
        "profile": profile["name"],
        "collected_at": datetime.utcnow().isoformat() + "Z",
        "orchestrator": data["orchestrator"].to_dict(),
        "inbox": data["inbox"].to_dict(),
        "kanban": data["kanban"].to_dict(),
        "agents": data["agents"].to_dict(),
        "logs": data["logs"].to_dict(),
        "github": data["github"].to_dict(),
    }
    return json.dumps(payload, indent=2)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Odin core modular TUI")
    parser.add_argument("-l", "--live", action="store_true", help="Run live dashboard loop")
    parser.add_argument("--json", action="store_true", help="Emit JSON payload")
    parser.add_argument("--profile", default=os.environ.get("ODIN_TUI_PROFILE", "core"), help="Profile name: core|legacy")
    parser.add_argument("--config", help="Optional JSON config file for panel/profile overrides")
    parser.add_argument("--refresh", type=int, help="Refresh interval seconds override")
    parser.add_argument("--odin-dir", help="Override ODIN_DIR path")
    args = parser.parse_args(argv)

    profile = resolve_profile(args.profile, args.config)

    if args.profile == "legacy" or profile["name"] == "legacy":
        return _run_legacy(args)

    refresh_seconds = max(1, int(args.refresh or profile.get("refresh_seconds", 5)))
    odin_dir = Path(args.odin_dir) if args.odin_dir else env_odin_dir()

    console = Console()

    if args.json:
        data = _collect_core(odin_dir)
        print(_json_output(profile, data))
        return 0

    def build_renderable():
        data = _collect_core(odin_dir)
        width = console.size.width
        height = console.size.height
        return _render_core(data, profile, width, height)

    if args.live:
        with Live(build_renderable(), console=console, refresh_per_second=2, screen=True) as live:
            try:
                while True:
                    time.sleep(refresh_seconds)
                    live.update(build_renderable())
            except KeyboardInterrupt:
                return 0

    console.print(build_renderable())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
