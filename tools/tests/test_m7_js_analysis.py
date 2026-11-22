"""Unit tests for tools/m7_js_analysis.py (formatting, CLI validation, browser smoke contract)."""
from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import unittest

TOOLS = Path(__file__).resolve().parents[1]
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

import m7_js_analysis


class M7JsAnalysisTests(unittest.TestCase):
    def test_format_helpers(self):
        self.assertEqual(m7_js_analysis.fmt_ns(None), "—")
        self.assertEqual(m7_js_analysis.fmt_ns(500.0), "500 ns")
        self.assertEqual(m7_js_analysis.fmt_ns(17_500.0), "17.50 us")
        self.assertEqual(m7_js_analysis.fmt_ns(42_500_000.0), "42.50 ms")
        self.assertEqual(m7_js_analysis.fmt_ns(1_250_000_000.0), "1.250 s")
        self.assertEqual(m7_js_analysis.fmt_mib(256 * 1024 * 1024), "256.0 MiB")
        self.assertEqual(m7_js_analysis.fmt_ratio(15.0, 30.0), "0.500x")
        self.assertEqual(m7_js_analysis.fmt_ratio(15.0, 0.0), "—")

    def test_rejects_unknown_browser_engine(self):
        proc = subprocess.run(
            [
                sys.executable,
                str(TOOLS / "m7_js_analysis.py"),
                "--smoke-browsers",
                "chromium,opera",
                "--output",
                str(TOOLS.parent / "target/tmp-invalid-browser-test"),
            ],
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("Unsupported browser: opera", proc.stderr)


if __name__ == "__main__":
    unittest.main()
