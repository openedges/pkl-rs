"""Unit tests for tools/m7_js_analysis.py (formatting, CLI validation, browser smoke contract)."""
from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import unittest

TOOLS = Path(__file__).resolve().parents[1]
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

import check_linux_glibc
import m7_js_analysis


class M7JsAnalysisTests(unittest.TestCase):
    def test_linux_glibc_version_parser(self):
        sample_ok = "  0x0010: Name: GLIBC_2.17  Flags: none\n  0x0020: Name: GLIBC_2.28  Flags: none\n"
        self.assertEqual(check_linux_glibc.max_glibc_version(sample_ok), (2, 28, 0))
        self.assertLessEqual(check_linux_glibc.max_glibc_version(sample_ok), check_linux_glibc.parse_version("2.28"))
        sample_bad = sample_ok + "  0x0030: Name: GLIBC_2.39  Flags: none\n"
        self.assertEqual(check_linux_glibc.max_glibc_version(sample_bad), (2, 39, 0))
        self.assertGreater(check_linux_glibc.max_glibc_version(sample_bad), check_linux_glibc.parse_version("2.28"))

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
