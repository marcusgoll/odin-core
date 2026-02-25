"""Profile resolution and user config merging for the core TUI."""

from __future__ import annotations

import json
from pathlib import Path

CORE_PANELS = ["header", "inbox", "kanban", "agents", "logs", "github"]

BUILTIN_PROFILES: dict[str, dict] = {
    "core": {
        "panels": CORE_PANELS,
        "refresh_seconds": 5,
    },
    "legacy": {
        "panels": [],
        "refresh_seconds": 5,
    },
}


def load_user_config(path: str | None) -> dict:
    if not path:
        return {}

    config_path = Path(path)
    if not config_path.exists():
        raise ValueError(f"config path not found: {config_path}")

    try:
        return json.loads(config_path.read_text())
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON config: {exc}") from exc


def resolve_profile(profile: str, config_path: str | None = None) -> dict:
    if profile not in BUILTIN_PROFILES:
        raise ValueError(f"unknown profile: {profile}")

    resolved = dict(BUILTIN_PROFILES[profile])
    user_config = load_user_config(config_path)

    selected_profile = user_config.get("profile")
    if selected_profile:
        if selected_profile not in BUILTIN_PROFILES:
            raise ValueError(f"unknown profile in config: {selected_profile}")
        resolved = dict(BUILTIN_PROFILES[selected_profile])
        profile = selected_profile

    if "refresh_seconds" in user_config:
        value = int(user_config["refresh_seconds"])
        resolved["refresh_seconds"] = max(1, value)

    if profile == "core":
        panel_config = user_config.get("panels")
        if isinstance(panel_config, dict):
            # disable map: {"github": false}
            active = []
            for panel in CORE_PANELS:
                keep = panel_config.get(panel, True)
                if keep:
                    active.append(panel)
            resolved["panels"] = active
        elif isinstance(panel_config, list) and panel_config:
            # explicit order
            allowed = set(CORE_PANELS)
            filtered = [p for p in panel_config if p in allowed]
            if filtered:
                resolved["panels"] = filtered

    resolved["name"] = profile
    return resolved
