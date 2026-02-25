from __future__ import annotations

import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from tui_core.layout import select_layout_mode  # noqa: E402


class LayoutTests(unittest.TestCase):
    def test_narrow(self):
        self.assertEqual(select_layout_mode(80), "narrow")

    def test_medium(self):
        self.assertEqual(select_layout_mode(120), "medium")

    def test_wide(self):
        self.assertEqual(select_layout_mode(180), "wide")


if __name__ == "__main__":
    unittest.main()
