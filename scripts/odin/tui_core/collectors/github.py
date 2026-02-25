"""GitHub PR collector (fail-soft)."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

from tui_core.models import PanelData


def collect(_odin_dir: Path, limit: int = 15) -> PanelData:
    cmd = [
        "gh",
        "pr",
        "list",
        "--limit",
        str(limit),
        "--json",
        "number,title,state,isDraft,author,updatedAt,headRefName",
    ]

    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=8,
            check=False,
        )
    except FileNotFoundError:
        return PanelData(
            key="github",
            title="GitHub",
            status="warn",
            items=[],
            meta={"open_prs": 0},
            errors=["gh CLI not installed"],
        )
    except subprocess.TimeoutExpired:
        return PanelData(
            key="github",
            title="GitHub",
            status="warn",
            items=[],
            meta={"open_prs": 0},
            errors=["gh PR query timed out"],
        )

    if proc.returncode != 0:
        message = (proc.stderr or proc.stdout or "gh PR query failed").strip().splitlines()[0]
        return PanelData(
            key="github",
            title="GitHub",
            status="warn",
            items=[],
            meta={"open_prs": 0},
            errors=[message],
        )

    try:
        prs = json.loads(proc.stdout)
    except json.JSONDecodeError:
        return PanelData(
            key="github",
            title="GitHub",
            status="warn",
            items=[],
            meta={"open_prs": 0},
            errors=["invalid JSON from gh"],
        )

    items = []
    for pr in prs:
        items.append(
            {
                "number": pr.get("number"),
                "title": pr.get("title", ""),
                "state": pr.get("state", "OPEN"),
                "draft": bool(pr.get("isDraft")),
                "author": (pr.get("author") or {}).get("login", "unknown"),
                "branch": pr.get("headRefName", ""),
            }
        )

    return PanelData(
        key="github",
        title="GitHub",
        status="ok",
        items=items,
        meta={"open_prs": len(items)},
        errors=[],
    )
