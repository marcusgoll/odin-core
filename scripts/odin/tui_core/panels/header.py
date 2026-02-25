"""Header renderer."""

from __future__ import annotations

from rich.panel import Panel


def render(profile_name: str, heartbeat: str, pending: int, total_agents: int, layout_mode: str) -> Panel:
    text = (
        f"Profile: [bold]{profile_name}[/bold]   "
        f"Heartbeat: [bold]{heartbeat}[/bold]   "
        f"Inbox: [bold]{pending}[/bold]   "
        f"Agents: [bold]{total_agents}[/bold]   "
        f"Layout: [bold]{layout_mode}[/bold]"
    )
    return Panel(text, title="[bold]Odin Core Dashboard[/bold]", border_style="cyan")
