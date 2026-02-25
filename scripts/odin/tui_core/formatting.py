"""Shared text and time formatting helpers for human-facing panels."""

from __future__ import annotations

import re
from datetime import datetime, timezone

SPECIAL_TASK_LABELS = {
    "pr_review_second": "Second PR Review",
}

TOKEN_LABELS = {
    "api": "API",
    "ci": "CI",
    "db": "DB",
    "llm": "LLM",
    "n8n": "n8n",
    "pr": "PR",
    "qa": "QA",
    "ssh": "SSH",
    "ui": "UI",
    "ux": "UX",
}

ORDINAL_SUFFIXES = {"first", "second", "third", "fourth", "fifth"}
DELIMITER_RE = re.compile(r"[._/\-]+")


def task_label_for_type(task_type: str | None) -> str:
    if not task_type:
        return "Unknown Task"

    raw = str(task_type).strip()
    if not raw:
        return "Unknown Task"

    mapped = SPECIAL_TASK_LABELS.get(raw)
    if mapped:
        return mapped

    tokens = [t for t in DELIMITER_RE.split(raw) if t]
    if not tokens:
        return "Unknown Task"

    # Patterns like "pr_review_second" are easier to scan as "Second PR Review".
    if len(tokens) > 1 and tokens[-1].lower() in ORDINAL_SUFFIXES:
        tokens = [tokens[-1], *tokens[:-1]]

    parts: list[str] = []
    for token in tokens:
        lower = token.lower()
        if lower in TOKEN_LABELS:
            parts.append(TOKEN_LABELS[lower])
        elif token.isupper():
            parts.append(token)
        else:
            parts.append(lower.capitalize())
    return " ".join(parts)


def compact_relative_age(age_seconds: float | int | None) -> str:
    if age_seconds is None:
        return "n/a"

    seconds = max(0, int(age_seconds))
    if seconds < 60:
        return f"{seconds}s ago"
    if seconds < 3600:
        return f"{seconds // 60}m ago"
    if seconds < 86400:
        return f"{seconds // 3600}h ago"
    return f"{seconds // 86400}d ago"


def wip_state(count: int, wip_limit: int) -> str:
    limit = int(wip_limit)
    size = int(count)
    if limit <= 0:
        return "unbounded"
    if size > limit:
        return "over"
    if size == limit:
        return "full"
    return "ok"


def parse_iso_timestamp(value: str | None) -> datetime | None:
    if not value:
        return None
    text = value.strip()
    if not text:
        return None
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    try:
        parsed = datetime.fromisoformat(text)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        return parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)

