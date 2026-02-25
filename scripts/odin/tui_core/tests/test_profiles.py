from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from tui_core.profiles import CORE_PANELS, resolve_profile  # noqa: E402


class ProfileTests(unittest.TestCase):
    def test_core_default(self):
        profile = resolve_profile("core")
        self.assertEqual(profile["name"], "core")
        self.assertEqual(profile["panels"], CORE_PANELS)

    def test_core_disable_panel_map(self):
        with tempfile.TemporaryDirectory() as tmp:
            cfg_path = Path(tmp) / "cfg.json"
            cfg_path.write_text(json.dumps({"panels": {"github": False}}))
            profile = resolve_profile("core", str(cfg_path))
            self.assertNotIn("github", profile["panels"])
            self.assertIn("inbox", profile["panels"])

    def test_core_explicit_order(self):
        with tempfile.TemporaryDirectory() as tmp:
            cfg_path = Path(tmp) / "cfg.json"
            cfg_path.write_text(json.dumps({"panels": ["header", "logs", "inbox"]}))
            profile = resolve_profile("core", str(cfg_path))
            self.assertEqual(profile["panels"], ["header", "logs", "inbox"])

    def test_legacy_profile(self):
        profile = resolve_profile("legacy")
        self.assertEqual(profile["name"], "legacy")
        self.assertEqual(profile["panels"], [])


if __name__ == "__main__":
    unittest.main()
