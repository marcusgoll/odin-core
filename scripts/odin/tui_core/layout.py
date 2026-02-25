"""Responsive layout mode selection by terminal width."""

from __future__ import annotations


def select_layout_mode(width: int) -> str:
    if width < 100:
        return "narrow"
    if width < 160:
        return "medium"
    return "wide"
