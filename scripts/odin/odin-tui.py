#!/usr/bin/env python3
"""Thin compatibility entrypoint for the modular odin-core TUI."""

from __future__ import annotations

from tui_core.app import main


if __name__ == "__main__":
    raise SystemExit(main())
