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
        self.assertEqual(counts["m7_native_image_required_workloads"], 15)
        self.assertGreaterEqual(counts["m7_native_image_pinned_platforms"], 1)
        self.assertEqual(counts["unpinned_optional_references"], 0)
        self.assertEqual(counts["m7_soak_target_scenarios"], 18)
        self.assertEqual(counts["m7_fuzz_categories"], 7)
        self.assertEqual(counts["platform_matrix_entries"], 11)
        self.assertEqual(counts["excluded_scope_entries"], 3)
        ni_ref = manifest["pinned_native_image_reference"]
        self.assertEqual(ni_ref["id"], "native-image-reference")
        self.assertEqual(ni_ref["status"], "pinned-required-release-baseline")
        self.assertEqual(ni_ref["required_workload_count"], 15)
        self.assertIn("Darwin-arm64", ni_ref["pinned_platforms"])

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
                        "id": "scalar:cold-cli",
                        "family": "core-6",
                        "mode": "cold-cli",
                        "correctness_verified": True,
                        "vs_official_native_image": {"time_ratio": 0.8, "rss_ratio": 0.6, "verdict": "pass", "reason": "ok"},
                        "vs_live_temurin_21": {"time_ratio": 0.02, "rss_ratio": 0.09, "verdict": "pass", "reason": "ok"},
                    },
                    {
                        "id": "wasm-browser-eval-text:wasm-browser",
                        "family": "wasm-browser",
                        "mode": "wasm-browser",
                        "correctness_verified": True,
                        "rust": {"browserErrors": []},
                        "vs_official_native_image": None,
                        "vs_live_temurin_21": {"time_ratio": 0.5, "rss_ratio": 0.4, "verdict": "pass", "reason": "ok"},
                    },
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
                self.assertIsNone(verdict["provenance"]["local_verification"]["git_head"])
                self.assertTrue(verdict["local_strict_gates_passed"])
                self.assertEqual(verdict["verified_platforms"], ["browser-chromium", "darwin-arm64"])
                self.assertEqual(verdict["observed_non_passing_platforms"], [])
                self.assertEqual(verdict["unmet_performance_workloads_official_native_image"], [])
                self.assertEqual(
                    verdict["pinned_native_image_reference"]["status"],
                    "pinned-required-release-baseline",
                )
                # Linux/Windows/Firefox/WebKit/hosted-CI are unverified -> release_ready MUST be False!
                self.assertFalse(verdict["release_ready"])
                self.assertIn("linux-x64-gnu", verdict["unverified_platforms"])
                self.assertIn("linux-x64-musl", verdict["unverified_platforms"])
                self.assertIn("hosted-ci", verdict["unverified_platforms"])

            # Unmet Official Apple Pkl native-image ratio on a CLI workload MUST fail local strict gate
            # and be explicitly enumerated in unmet_performance_workloads_official_native_image!
            fake_ni_fail = copy.deepcopy(fake_bench_pass)
            fake_ni_fail["results"][0]["vs_official_native_image"] = {
                "time_ratio": 1.30,
                "rss_ratio": 0.65,
                "verdict": "fail",
                "reason": "time_ratio=1.3000>1.00",
            }

            def ni_fail_side_effect(d, *, root=m7_manifest.ROOT, strict=False):
                if strict:
                    raise ValueError("scalar:cold-cli vs_official_native_image=fail")
                return fake_ni_fail

            with (
                mock.patch("m7_benchmark.verify_report", side_effect=ni_fail_side_effect),
                mock.patch("m7_memory_soak.verify_report", return_value=fake_soak_pass),
                mock.patch("m7_fuzz.verify_report", return_value=fake_fuzz_pass),
            ):
                verdict = m7_manifest.verify_release(m7_manifest.ROOT, bundle_dir=bundle)
                self.assertTrue(verdict["integrity_passed"])
                self.assertIsNone(verdict["provenance"]["local_verification"]["git_head"])
                self.assertFalse(verdict["benchmark_strict_passed"])
                self.assertFalse(verdict["local_strict_gates_passed"])
                self.assertFalse(verdict["release_ready"])
                self.assertEqual(len(verdict["unmet_performance_workloads_official_native_image"]), 1)
                self.assertEqual(
                    verdict["unmet_performance_workloads_official_native_image"][0]["id"],
                    "scalar:cold-cli",
                )

            # Unmet performance ratio on one workload must flip local_strict_gates_passed to False
            # AND must NEVER mark darwin-arm64 or browser-chromium in verified_platforms (B4 honesty)!
            fake_bench_fail = copy.deepcopy(fake_bench_pass)
            fake_bench_fail["results"][1]["vs_live_temurin_21"] = {
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
                self.assertIsNone(verdict["provenance"]["local_verification"]["git_head"])
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

    def test_verify_release_reaches_release_ready_with_3_browsers_and_hosted_ci_attestation(self):
        commit = "a" * 40
        sha = "b" * 64
        with tempfile.TemporaryDirectory() as tmp:
            bundle = Path(tmp) / "release"
            (bundle / "benchmark").mkdir(parents=True)
            (bundle / "soak").mkdir(parents=True)
            (bundle / "fuzz").mkdir(parents=True)
            (bundle / "js-analysis").mkdir(parents=True)

            fake_bench_pass = {
                "source": {"git_head": commit},
                "machine_start": {"system": "Darwin", "machine": "arm64"},
                "results": [
                    {
                        "id": "scalar:cold-cli",
                        "family": "core-6",
                        "mode": "cold-cli",
                        "correctness_verified": True,
                        "vs_official_native_image": {"time_ratio": 0.5, "rss_ratio": 0.4, "verdict": "pass", "reason": "ok"},
                        "vs_live_temurin_21": {"time_ratio": 0.02, "rss_ratio": 0.09, "verdict": "pass", "reason": "ok"},
                    },
                    {
                        "id": "wasm-browser-eval-text:wasm-browser",
                        "family": "wasm-browser",
                        "mode": "wasm-browser",
                        "correctness_verified": True,
                        "rust": {"browserErrors": []},
                        "vs_official_native_image": None,
                        "vs_live_temurin_21": {"time_ratio": 0.5, "rss_ratio": 0.4, "verdict": "pass", "reason": "ok"},
                    },
                ],
            }
            fake_soak_pass = {
                "source": {"git_head": commit},
                "host": {"system": "Darwin", "machine": "arm64"},
                "results": [{"target": "native", "scenario": "cyclic-graphs", "verdict": "bounded"}],
            }
            fake_fuzz_pass = {
                "source": {"git_head": commit},
                "host": {"system": "Darwin", "machine": "arm64"},
                "results": [{"id": "fuzz-001", "status": "pass"}],
            }

            js_rows = []
            for engine in ("chromium", "firefox", "webkit"):
                for wl in ("eval-text", "typed-conversion"):
                    js_rows.append(
                        {
                            "id": f"wasm-{engine}:{wl}",
                            "kind": "wasm-browser",
                            "browser_engine": engine,
                            "workload": wl,
                            "launch_args": [],
                            "status": "ok",
                            "preflight_verified": True,
                            "time_ratio_vs_java": 0.65,
                            "wasm_linear_memory_bytes": 6_000_000,
                            "contract_verdict": {"verdict": "pass", "reason": "ok"},
                        }
                    )
            (bundle / "js-analysis" / "report.json").write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "mode": "js-analysis",
                        "profile": "full",
                        "wasm_binary_gzip_bytes": 2_100_000,
                        "results": js_rows,
                    }
                ),
                encoding="utf-8",
            )

            attestation = {
                "schema_version": 1,
                "provenance": "hosted-ci",
                "workflow": "Release",
                "github_repository": "openedges/pkl-rs",
                "github_run_id": "123456789",
                "source_commit": commit,
                "checksums_sha256": sha,
                "wasm_package": {
                    "tarball": "openedges-pkl-wasm-0.32.0.tgz",
                    "sha256": sha,
                    "gzip_bytes": 2_100_000,
                    "tests_passed": True,
                },
                "crates_packaged": list(m7_manifest.REQUIRED_PUBLISHABLE_CRATES),
                "platforms": {
                    pid: {
                        "status": "pass",
                        "smoke_verified": True,
                        "cli_archive": {"name": f"pkl-{pid}.tar.gz", "sha256": sha},
                        "pkldoc_archive": {"name": f"pkldoc-{pid}.tar.gz", "sha256": sha},
                        "napi_addon": {"name": f"pkl_napi.{pid}.node", "sha256": sha},
                    }
                    for pid in m7_manifest.REQUIRED_CI_NATIVE_PLATFORMS
                },
            }
            att_path = bundle / "ci-attestation.json"
            att_path.write_text(json.dumps(attestation), encoding="utf-8")

            with (
                mock.patch("m7_benchmark.verify_report", return_value=fake_bench_pass),
                mock.patch("m7_memory_soak.verify_report", return_value=fake_soak_pass),
                mock.patch("m7_fuzz.verify_report", return_value=fake_fuzz_pass),
            ):
                verdict = m7_manifest.verify_release(m7_manifest.ROOT, bundle_dir=bundle)
                self.assertTrue(verdict["integrity_passed"])
                self.assertEqual(verdict["provenance"]["local_verification"]["git_head"], commit)
                self.assertTrue(verdict["local_strict_gates_passed"])
                self.assertEqual(verdict["unverified_platforms"], [])
                self.assertEqual(verdict["observed_non_passing_platforms"], [])
                self.assertTrue(verdict["release_ready"])
                self.assertEqual(verdict["provenance"]["hosted_ci_attestation"]["provenance"], "hosted-ci")

                # Commit mismatch between local verification and hosted CI attestation MUST fail integrity!
                bad_commit = copy.deepcopy(attestation)
                bad_commit["source_commit"] = "c" * 40
                att_path.write_text(json.dumps(bad_commit), encoding="utf-8")
                v_bad = m7_manifest.verify_release(m7_manifest.ROOT, bundle_dir=bundle)
                self.assertFalse(v_bad["integrity_passed"])
                self.assertFalse(v_bad["release_ready"])
                self.assertIn("hosted-ci", v_bad["unverified_platforms"])

                # Local provenance CANNOT spoof hosted-ci!
                bad_prov = copy.deepcopy(attestation)
                bad_prov["provenance"] = "local"
                att_path.write_text(json.dumps(bad_prov), encoding="utf-8")
                v_prov = m7_manifest.verify_release(m7_manifest.ROOT, bundle_dir=bundle)
                self.assertFalse(v_prov["integrity_passed"])
                self.assertFalse(v_prov["release_ready"])

                # Missing browser or failing browser verdict prevents release_ready
                att_path.write_text(json.dumps(attestation), encoding="utf-8")
                bad_js = copy.deepcopy(js_rows)
                bad_js[-1]["contract_verdict"] = {"verdict": "fail", "reason": "time_ratio=1.20>1.00"}
                (bundle / "js-analysis" / "report.json").write_text(
                    json.dumps(
                        {
                            "schema_version": 1,
                            "mode": "js-analysis",
                            "profile": "full",
                            "wasm_binary_gzip_bytes": 2_100_000,
                            "results": bad_js,
                        }
                    ),
                    encoding="utf-8",
                )
                v_browser_fail = m7_manifest.verify_release(m7_manifest.ROOT, bundle_dir=bundle)
                self.assertFalse(v_browser_fail["release_ready"])
                self.assertIn("browser-webkit", v_browser_fail["observed_non_passing_platforms"])
                self.assertIn("browser-webkit", v_browser_fail["unverified_platforms"])

    def test_verify_release_rejects_mismatched_git_head_across_m7_reports(self):
        commit_a = "a" * 40
        commit_b = "d" * 40
        with tempfile.TemporaryDirectory() as tmp:
            bundle = Path(tmp) / "release"
            (bundle / "benchmark").mkdir(parents=True)
            (bundle / "soak").mkdir(parents=True)
            (bundle / "fuzz").mkdir(parents=True)
            (bundle / "js-analysis").mkdir(parents=True)

            fake_bench = {
                "source": {"git_head": commit_a},
                "machine_start": {"system": "Darwin", "machine": "arm64"},
                "results": [
                    {
                        "id": "scalar:cold-cli",
                        "family": "core-6",
                        "mode": "cold-cli",
                        "correctness_verified": True,
                        "vs_official_native_image": {"time_ratio": 0.5, "rss_ratio": 0.4, "verdict": "pass", "reason": "ok"},
                        "vs_live_temurin_21": {"time_ratio": 0.02, "rss_ratio": 0.09, "verdict": "pass", "reason": "ok"},
                    }
                ],
            }
            fake_soak_mismatch = {
                "source": {"git_head": commit_b},
                "host": {"system": "Darwin", "machine": "arm64"},
                "results": [{"target": "native", "scenario": "cyclic-graphs", "verdict": "bounded"}],
            }
            fake_fuzz = {
                "source": {"git_head": commit_a},
                "host": {"system": "Darwin", "machine": "arm64"},
                "results": [{"id": "fuzz-001", "status": "pass"}],
            }
            with (
                mock.patch("m7_benchmark.verify_report", return_value=fake_bench),
                mock.patch("m7_memory_soak.verify_report", return_value=fake_soak_mismatch),
                mock.patch("m7_fuzz.verify_report", return_value=fake_fuzz),
            ):
                verdict = m7_manifest.verify_release(m7_manifest.ROOT, bundle_dir=bundle)
                self.assertFalse(verdict["integrity_passed"])
                self.assertFalse(verdict["local_strict_gates_passed"])
                self.assertFalse(verdict["release_ready"])
                self.assertTrue(
                    any("mismatched git_head" in msg for msg in verdict["integrity_failures"]),
                    verdict["integrity_failures"],
                )

            # Matching core reports + mismatched js-analysis git_head MUST also fail integrity!
            fake_soak_ok = copy.deepcopy(fake_soak_mismatch)
            fake_soak_ok["source"]["git_head"] = commit_a
            (bundle / "js-analysis" / "report.json").write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "mode": "js-analysis",
                        "profile": "full",
                        "source": {"git_head": commit_b},
                        "wasm_binary_gzip_bytes": 2_100_000,
                        "results": [
                            {
                                "id": "wasm-chromium:eval-text",
                                "kind": "wasm-browser",
                                "browser_engine": "chromium",
                                "workload": "eval-text",
                                "launch_args": [],
                                "status": "ok",
                                "preflight_verified": True,
                                "time_ratio_vs_java": 0.5,
                                "wasm_linear_memory_bytes": 5_000_000,
                                "contract_verdict": {"verdict": "pass", "reason": "ok"},
                            },
                            {
                                "id": "wasm-chromium:typed-conversion",
                                "kind": "wasm-browser",
                                "browser_engine": "chromium",
                                "workload": "typed-conversion",
                                "launch_args": [],
                                "status": "ok",
                                "preflight_verified": True,
                                "time_ratio_vs_java": 0.5,
                                "wasm_linear_memory_bytes": 5_000_000,
                                "contract_verdict": {"verdict": "pass", "reason": "ok"},
                            },
                        ],
                    }
                ),
                encoding="utf-8",
            )
            with (
                mock.patch("m7_benchmark.verify_report", return_value=fake_bench),
                mock.patch("m7_memory_soak.verify_report", return_value=fake_soak_ok),
                mock.patch("m7_fuzz.verify_report", return_value=fake_fuzz),
            ):
                verdict_js = m7_manifest.verify_release(m7_manifest.ROOT, bundle_dir=bundle)
                self.assertFalse(verdict_js["integrity_passed"])
                self.assertFalse(verdict_js["local_strict_gates_passed"])
                self.assertTrue(
                    any("mismatched git_head" in msg for msg in verdict_js["integrity_failures"]),
                    verdict_js["integrity_failures"],
                )


if __name__ == "__main__":
    unittest.main()
