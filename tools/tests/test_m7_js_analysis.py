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
import m7_benchmark
import m7_js_analysis


class M7JsAnalysisTests(unittest.TestCase):
    def test_browser_wasm_verdict_contract_and_firefox_only_exception(self):
        btargets = m7_benchmark.load_contract()["browser_wasm_targets"]
        # Chromium passing: time <= 1.0x, linear mem <= 0.15x, JS heap <= 64 MiB, gzip <= 2.50 MiB,
        # even when full multi-process browser tree RSS is 4.0x Java (preserved honestly).
        ok = m7_benchmark.evaluate_browser_wasm_verdict(
            80_000.0,
            250_000.0,
            1_000_000_000.0,
            250_000_000.0,
            wasm_linear_memory_bytes=19_000_000.0,
            js_heap_used_bytes=12_000_000.0,
            wasm_binary_gzip_bytes=2_500_000,
            correctness_verified=True,
            converged=True,
            browser_targets=btargets,
            browser_engine="chromium",
            workload_key="typed-conversion",
        )
        self.assertEqual(ok["verdict"], "pass")
        self.assertAlmostEqual(ok["rss_ratio"], 4.0, places=3)
        self.assertFalse(ok["firefox_time_exception_applied"])

        # Chromium > 1.00x time MUST FAIL (exception is Firefox-only)
        c_fail = m7_benchmark.evaluate_browser_wasm_verdict(
            110_000.0,
            100_000.0,
            900_000_000.0,
            250_000_000.0,
            wasm_linear_memory_bytes=12_000_000.0,
            js_heap_used_bytes=12_000_000.0,
            wasm_binary_gzip_bytes=2_500_000,
            correctness_verified=True,
            converged=True,
            browser_targets=btargets,
            browser_engine="chromium",
            workload_key="typed-conversion",
        )
        self.assertEqual(c_fail["verdict"], "fail")

        # Firefox eval-text > 1.00x MUST FAIL (only typed-conversion is exempted)
        ff_eval_fail = m7_benchmark.evaluate_browser_wasm_verdict(
            110_000.0,
            100_000.0,
            800_000_000.0,
            250_000_000.0,
            wasm_linear_memory_bytes=19_000_000.0,
            js_heap_used_bytes=None,
            wasm_binary_gzip_bytes=2_500_000,
            correctness_verified=True,
            converged=True,
            browser_targets=btargets,
            browser_engine="firefox",
            workload_key="eval-text",
        )
        self.assertEqual(ff_eval_fail["verdict"], "fail")

        # Firefox typed-conversion 1.35x passes ONLY via explicit Firefox exception
        ff_typed_ok = m7_benchmark.evaluate_browser_wasm_verdict(
            135_000.0,
            100_000.0,
            900_000_000.0,
            250_000_000.0,
            wasm_linear_memory_bytes=12_000_000.0,
            js_heap_used_bytes=None,
            wasm_binary_gzip_bytes=2_500_000,
            correctness_verified=True,
            converged=True,
            browser_targets=btargets,
            browser_engine="firefox",
            workload_key="typed-conversion",
        )
        self.assertEqual(ff_typed_ok["verdict"], "pass")
        self.assertTrue(ff_typed_ok["firefox_time_exception_applied"])

        # Excess Wasm linear memory (> 0.15x), JS heap (> 64 MiB), or gzip (> 2.50 MiB) MUST FAIL
        for kw in (
            {"wasm_linear_memory_bytes": 50_000_000.0},
            {"js_heap_used_bytes": 80_000_000.0},
            {"wasm_binary_gzip_bytes": 3_000_000},
        ):
            params = dict(
                wasm_linear_memory_bytes=12_000_000.0,
                js_heap_used_bytes=12_000_000.0,
                wasm_binary_gzip_bytes=2_500_000,
                correctness_verified=True,
                converged=True,
                browser_targets=btargets,
                browser_engine="chromium",
                workload_key="eval-text",
            )
            params.update(kw)
            bad = m7_benchmark.evaluate_browser_wasm_verdict(50_000.0, 100_000.0, 900_000_000.0, 250_000_000.0, **params)
            self.assertEqual(bad["verdict"], "fail")
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
