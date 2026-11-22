"""Unit and tamper-detection tests for tools/m7_manifest.py (no runtime pass claim)."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

import m7_benchmark
import m7_manifest


class M7ManifestTests(unittest.TestCase):
    def test_build_manifest_freshness_and_coverage_gate_null(self):
        manifest = m7_manifest.build_manifest()
        self.assertEqual(manifest["schema_version"], 1)
        self.assertEqual(manifest["baseline"]["version"], "0.32.0")
        self.assertIsNone(manifest["coverage_gate"])
        counts = manifest["counts"]
        self.assertEqual(counts["prior_milestone_manifests"], 6)
        self.assertEqual(counts["m7_benchmark_workload_modes"], 34)
        self.assertEqual(counts["m7_soak_target_scenarios"], 18)
        self.assertEqual(counts["m7_fuzz_categories"], 7)
        self.assertEqual(counts["platform_matrix_entries"], 9)
        self.assertEqual(counts["excluded_scope_entries"], 3)

    def test_validate_mappings_rejects_unmapped_and_missing_symbol(self):
        contract = m7_benchmark.load_contract()
        mappings = m7_manifest.load_json(m7_manifest.ROOT / m7_manifest.MAPPINGS_PATH)

        # Unmapped benchmark workload
        bad_bench = copy.deepcopy(mappings)
        bad_bench["requirements"]["m7:perf:core-6-cold-cli"]["workload_ids"].pop()
        with self.assertRaisesRegex(ValueError, "unmapped M7 benchmark workloads"):
            m7_manifest.validate_mappings(m7_manifest.ROOT, bad_bench, contract)

        # Unmapped soak pair
        bad_soak = copy.deepcopy(mappings)
        bad_soak["requirements"]["m7:soak:native-memory-reclamation"]["soak_pairs"].pop()
        with self.assertRaisesRegex(ValueError, "unmapped M7 soak"):
            m7_manifest.validate_mappings(m7_manifest.ROOT, bad_soak, contract)

        # Unmapped fuzz category
        bad_fuzz = copy.deepcopy(mappings)
        bad_fuzz["requirements"]["m7:fuzz:seeded-differential-corpus"]["fuzz_categories"].pop()
        with self.assertRaisesRegex(ValueError, "unmapped M7 fuzz categories"):
            m7_manifest.validate_mappings(m7_manifest.ROOT, bad_fuzz, contract)

        # Missing implementation link symbol
        bad_sym = copy.deepcopy(mappings)
        bad_sym["requirements"]["m7:perf:core-6-cold-cli"]["implementation_links"][0]["symbol"] = "nonexistent_symbol_xyz"
        with self.assertRaisesRegex(ValueError, "implementation link symbol missing"):
            m7_manifest.validate_mappings(m7_manifest.ROOT, bad_sym, contract)

    def test_verify_release_honest_about_unverified_platforms_and_unmet_ratios(self):
        with tempfile.TemporaryDirectory() as tmp:
            bundle = Path(tmp) / "release"
            (bundle / "benchmark").mkdir(parents=True)
            (bundle / "soak").mkdir(parents=True)
            (bundle / "fuzz").mkdir(parents=True)

            fake_bench_pass = {
                "machine_start": {"system": "Darwin", "machine": "arm64"},
                "results": [
                    {
                        "id": "wasm-browser-eval-text:wasm-browser",
                        "family": "wasm-browser",
                        "correctness_verified": True,
                        "rust": {"browserErrors": []},
                        "vs_live_temurin_21": {"time_ratio": 0.5, "rss_ratio": 0.4, "verdict": "pass", "reason": "ok"},
                    }
                ],
            }
            fake_soak_pass = {
                "host": {"system": "Darwin", "machine": "arm64"},
                "results": [{"target": "native", "scenario": "cyclic-graphs", "verdict": "bounded"}],
            }
            fake_fuzz_pass = {
                "host": {"system": "Darwin", "machine": "arm64"},
                "results": [{"id": "fuzz-001", "status": "pass"}],
            }

            with (
                mock.patch("m7_benchmark.verify_report", return_value=fake_bench_pass),
                mock.patch("m7_memory_soak.verify_report", return_value=fake_soak_pass),
                mock.patch("m7_fuzz.verify_report", return_value=fake_fuzz_pass),
            ):
                verdict = m7_manifest.verify_release(m7_manifest.ROOT, bundle_dir=bundle)
                self.assertTrue(verdict["integrity_passed"])
                self.assertTrue(verdict["local_strict_gates_passed"])
                self.assertEqual(verdict["verified_platforms"], ["browser-chromium", "darwin-arm64"])
                self.assertEqual(verdict["observed_non_passing_platforms"], [])
                # Linux/Windows/Firefox/WebKit/hosted-CI are unverified -> release_ready MUST be False!
                self.assertFalse(verdict["release_ready"])
                self.assertIn("linux-x64-gnu", verdict["unverified_platforms"])
                self.assertIn("hosted-ci", verdict["unverified_platforms"])

            # Unmet performance ratio on one workload must flip local_strict_gates_passed to False
            # AND must NEVER mark darwin-arm64 or browser-chromium in verified_platforms (B4 honesty)!
            fake_bench_fail = copy.deepcopy(fake_bench_pass)
            fake_bench_fail["results"][0]["vs_live_temurin_21"] = {
                "time_ratio": 1.42,
                "rss_ratio": 0.5,
                "verdict": "fail",
                "reason": "time_ratio=1.4200>1.00",
            }

            def bench_side_effect(d, *, root=m7_manifest.ROOT, strict=False):
                if strict:
                    raise ValueError("wasm-browser-eval-text:wasm-browser exceeded 1.00x")
                return fake_bench_fail

            with (
                mock.patch("m7_benchmark.verify_report", side_effect=bench_side_effect),
                mock.patch("m7_memory_soak.verify_report", return_value=fake_soak_pass),
                mock.patch("m7_fuzz.verify_report", return_value=fake_fuzz_pass),
            ):
                verdict = m7_manifest.verify_release(m7_manifest.ROOT, bundle_dir=bundle)
                self.assertTrue(verdict["integrity_passed"])
                self.assertFalse(verdict["benchmark_strict_passed"])
                self.assertFalse(verdict["local_strict_gates_passed"])
                self.assertFalse(verdict["release_ready"])
                self.assertEqual(verdict["verified_platforms"], [])
                self.assertEqual(verdict["observed_non_passing_platforms"], ["browser-chromium", "darwin-arm64"])
                self.assertIn("darwin-arm64", verdict["unverified_platforms"])
                self.assertIn("browser-chromium", verdict["unverified_platforms"])
                self.assertEqual(len(verdict["unmet_performance_workloads_live_temurin_21"]), 1)
                self.assertEqual(
                    verdict["unmet_performance_workloads_live_temurin_21"][0]["id"],
                    "wasm-browser-eval-text:wasm-browser",
                )

            # Missing bundle directories fail integrity
            empty = Path(tmp) / "empty"
            empty.mkdir()
            v_empty = m7_manifest.verify_release(m7_manifest.ROOT, bundle_dir=empty)
            self.assertFalse(v_empty["integrity_passed"])
            self.assertFalse(v_empty["release_ready"])


if __name__ == "__main__":
    unittest.main()
