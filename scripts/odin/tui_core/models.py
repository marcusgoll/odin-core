"""Shared model contracts for modular TUI data flow."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass
class PanelData:
    key: str
    title: str
    status: str = "ok"
    items: list[dict[str, Any]] = field(default_factory=list)
    meta: dict[str, Any] = field(default_factory=dict)
    errors: list[str] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        return {
            "key": self.key,
            "title": self.title,
            "status": self.status,
            "items": self.items,
            "meta": self.meta,
            "errors": self.errors,
        }
