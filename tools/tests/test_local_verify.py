#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Tests for tools/local_verify.py and GitHub Actions cost/packaging separation."""
from __future__ import annotations

import json
from pathlib import Path
import re
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))

import local_verify


class LocalVerifyAndWorkflowContractTests(unittest.TestCase):
    def test_ci_workflow_is_minimal_ubuntu_only_without_expensive_suites(self) -> None:
        ci_text = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
        # Must never schedule costly macOS (10x) or Windows (2x) runners on push/PR.
        self.assertNotIn("macos-", ci_text)
        self.assertNotIn("windows-", ci_text)
        self.assertIn("runs-on: ubuntu-24.04", ci_text)
        # Must keep minimal check + packaging smoke jobs and exclude heavy suites.
        self.assertIn("check-and-test:", ci_text)
        self.assertIn("package-smoke:", ci_text)
        for forbidden in (
            "m2_language.py",
            "m3_differential.py",
            "m4_differential.py",
            "m5_differential.py",
            "m6_differential.py",
            "m7_fuzz.py",
            "m7_memory_soak.py",
            "m7_js_analysis.py",
            "playwright install",
            "actions/setup-java",
        ):
            self.assertNotIn(forbidden, ci_text, f"Expensive step {forbidden!r} must live in local verification, not ci.yml")
        # Must enforce the 2.50 MiB gzip-9 .wasm budget during packaging smoke.
        self.assertIn("2621440", ci_text)

    def test_release_workflow_triggers_only_on_tags_or_dispatch_and_covers_all_7_napi_targets(self) -> None:
        rel_text = (ROOT / ".github" / "workflows" / "release.yml").read_text(encoding="utf-8")
        # Never triggers on pull_request or branch pushes without tag filter.
        self.assertNotIn("pull_request:", rel_text)
        self.assertIn("tags:", rel_text)
        self.assertIn("'v*'", rel_text)
        self.assertIn("workflow_dispatch:", rel_text)
        for job in (
            "package-linux-portable:",
            "package-macos-windows:",
            "package-wasm:",
            "assemble-and-release:",
        ):
            self.assertIn(job, rel_text)
        for addon in (
            "pkl_napi.darwin-arm64.node",
            "pkl_napi.darwin-x64.node",
            "pkl_napi.linux-x64-gnu.node",
            "pkl_napi.linux-arm64-gnu.node",
            "pkl_napi.linux-x64-musl.node",
            "pkl_napi.linux-arm64-musl.node",
            "pkl_napi.win32-x64.node",
        ):
            self.assertIn(addon, rel_text)
        self.assertIn("SHA256SUMS.txt", rel_text)
        self.assertIn("gh release create", rel_text)
        self.assertIn("npm publish", rel_text)
        # Every third-party action reference must remain commit-SHA pinned.
        for match in re.finditer(r"uses:\s+([^\s#]+)", rel_text):
            ref = match.group(1)
            self.assertRegex(ref, r"^[^@]+@[0-9a-f]{40}$", f"Unpinned action in release.yml: {ref}")

    def test_ci_workflow_actions_are_sha_pinned(self) -> None:
        ci_text = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
        for match in re.finditer(r"uses:\s+([^\s#]+)", ci_text):
            ref = match.group(1)
            self.assertRegex(ref, r"^[^@]+@[0-9a-f]{40}$", f"Unpinned action in ci.yml: {ref}")

    def test_python_test_discovery_accounts_for_every_file_in_tools_tests(self) -> None:
        all_files = sorted(f"tools/tests/{p.name}" for p in (ROOT / "tools" / "tests").glob("test_*.py"))
        discovered = local_verify.discover_python_test_modules(ROOT)
        active = discovered["active"]
        historical = discovered["historical"]
        self.assertEqual(sorted(active + historical), all_files, "No test_*.py file may be silently omitted")
        self.assertEqual(set(active) & set(historical), set())
        # Verify M1 (8), M2 (22), core (13), and M3-M7 (16) active modules are all included.
        self.assertGreaterEqual(len(active), 59)
        self.assertGreaterEqual(len(historical), 75)
        for required in (
            "tools/tests/test_local_verify.py",
            "tools/tests/test_runtime_fixture_links.py",
            "tools/tests/test_vendor_sources.py",
            "tools/tests/test_m1_platforms.py",
            "tools/tests/test_m2_language.py",
            "tools/tests/test_m7_benchmark.py",
        ):
            self.assertIn(required, active)

    def test_local_verify_full_and_full_strict_plan_complete_m0_through_m7_and_packages(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out_root = Path(tmp) / "out"
            selected = local_verify.resolve_suite_selection([], "full-strict", include_historical_tests=True)
            self.assertEqual(
                selected,
                [
                    "workspace",
                    "manifests",
                    "packages",
                    "m0",
                    "m1",
                    "m2",
                    "m3",
                    "m4",
                    "m5",
                    "m6",
                    "m7-fuzz",
                    "m7-soak",
                    "m7-bench",
                    "m7-js",
                    "m7-release",
                    "py-historical",
                ],
            )
            summary = local_verify.run_suites(
                selected,
                out_root=out_root,
                dry_run=True,
                full_corpus=True,
                baseline="both",
                strict_bench=True,
                all_soak_targets=True,
                local_checkpoint=True,
            )
            self.assertEqual(summary["overall_status"], "planned")
            self.assertEqual(summary["provenance"], "local-verification-environment-not-hosted-ci")
            self.assertFalse(out_root.exists(), "dry-run must not create output dirs")
            suite_map = {row["suite"]: row for row in summary["suites"]}

            # Manifests suite runs all active Python unit tests.
            manifest_steps = {s["name"]: s["command"] for s in suite_map["manifests"]["steps"]}
            self.assertIn("tools/tests/test_m1_platforms.py", manifest_steps["unit-tests-all-active-python"])
            self.assertIn("tools/tests/test_m2_language.py", manifest_steps["unit-tests-all-active-python"])

            # Packages suite covers Wasm gzip check, N-API run + pack-install, and CLI smoke.
            pkg_steps = [s["name"] for s in suite_map["packages"]["steps"]]
            self.assertIn("check-wasm-gzip-budget", pkg_steps)
            self.assertIn("test-wasm-pkg", pkg_steps)
            self.assertIn("test-napi-pkg", pkg_steps)
            self.assertIn("test-napi-pack-install", pkg_steps)

            # M0 includes Java replay comparison; M1 includes Graal archive check-report.
            m0_steps = [s["name"] for s in suite_map["m0"]["steps"]]
            self.assertIn("compat-compare-java-replay", m0_steps)
            m1_steps = [s["name"] for s in suite_map["m1"]["steps"]]
            self.assertIn("m1-graal-check-report", m1_steps)

            # M2 full verify, M4 all-5 Pantry, M7 strict benchmark, and M7 release wiring.
            self.assertIn("m2-verify-full", [s["name"] for s in suite_map["m2"]["steps"]])
            self.assertIn("m4-differential-all5", [s["name"] for s in suite_map["m4"]["steps"]])
            self.assertIn("m7-soak-verify-strict", [s["name"] for s in suite_map["m7-soak"]["steps"]])
            self.assertIn("m7-bench-verify-strict", [s["name"] for s in suite_map["m7-bench"]["steps"]])
            rel_cmd = suite_map["m7-release"]["steps"][0]["command"]
            self.assertIn("--benchmark-dir", rel_cmd)
            self.assertIn("--soak-dir", rel_cmd)
            self.assertIn("--fuzz-dir", rel_cmd)
            self.assertIn("--local-checkpoint", rel_cmd)

    def test_local_verify_rejects_unknown_suite_and_empty_selection(self) -> None:
        with self.assertRaises(ValueError):
            local_verify.resolve_suite_selection(["does-not-exist"], None)
        with self.assertRaises(ValueError):
            local_verify.resolve_suite_selection([], None)
        self.assertEqual(local_verify.main(["--suite", "bogus"]), 2)

    def test_local_verify_preserves_failure_honestly_in_summary_json(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out_root = Path(tmp) / "verify-run"
            # Run m7-release against an empty dir: must fail honestly and write summary.json.
            summary = local_verify.run_suites(
                ["m7-release"],
                out_root=out_root,
                dry_run=False,
                release_dir=Path(tmp) / "empty-release-dir",
            )
            self.assertEqual(summary["overall_status"], "fail")
            self.assertEqual(summary["suites"][0]["status"], "fail")
            self.assertTrue((out_root / "summary.json").is_file())
            stored = json.loads((out_root / "summary.json").read_text(encoding="utf-8"))
            self.assertEqual(stored["overall_status"], "fail")


if __name__ == "__main__":
    unittest.main()
