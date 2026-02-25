"""Collector helpers and package exports."""

from __future__ import annotations

import json
import os
import time
from pathlib import Path
from typing import Any


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return None


def file_age_seconds(path: Path) -> float | None:
    try:
        return max(0.0, time.time() - path.stat().st_mtime)
    except OSError:
        return None


def format_age(age: float | None) -> str:
    if age is None:
        return "n/a"
    sec = int(age)
    if sec < 60:
        return f"{sec}s"
    if sec < 3600:
        return f"{sec // 60}m"
    return f"{sec // 3600}h {(sec % 3600) // 60:02d}m"


def list_json_files(dir_path: Path) -> list[Path]:
    try:
        return sorted(
            [p for p in dir_path.glob("*.json") if p.is_file()],
            key=lambda p: p.stat().st_mtime,
            reverse=True,
        )
    except OSError:
        return []


def env_odin_dir() -> Path:
    return Path(os.environ.get("ODIN_DIR", "/var/odin"))
