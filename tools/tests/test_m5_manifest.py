#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Unit and gate-enforcement tests for tools/m5_manifest.py."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import tempfile
import unittest

import compat
import m5_differential
import m5_manifest

ROOT = Path(__file__).resolve().parents[2]


class M5ManifestTests(unittest.TestCase):
    def test_build_manifest_exact_inventory_and_exclusion_counts(self) -> None:
        manifest = m5_manifest.build_manifest(ROOT)
        c = manifest["counts"]
        self.assertIsNone(manifest["coverage_gate"])
        self.assertEqual(c["inventory_cli_command_total"], 17)
        self.assertEqual(c["inventory_cli_command_dispositions"], {
            "excluded": 2,
            "m4-exercised-and-m5-product": 8,
            "m5-new": 7,
        })
        self.assertEqual(c["inventory_cli_argument_total"], 6)
        self.assertEqual(c["inventory_cli_option_total"], 86)
        self.assertEqual(c["inventory_cli_option_dispositions"], {
            "excluded": 19,
            "m3-covered": 12,
            "m4-exercised": 29,
            "m5-new": 26,
        })
        self.assertEqual(c["inventory_cli_option_m5_new_breakdown"], {
            "m5:cli-other": 15,
            "m5:format": 6,
            "m5:pkldoc": 5,
        })
        self.assertEqual(c["inventory_protocol_tag_total"], 19)
        self.assertEqual(c["inventory_protocol_tag_dispositions"], {
            "m4-and-m5-callbacks": 13,
            "m5-required": 6,
        })
        self.assertEqual(c["inventory_protocol_field_total"], 119)
        self.assertEqual(c["inventory_protocol_field_dispositions"], {
            "m4-required": 37,
            "m5-required": 82,
        })
        self.assertEqual(c["inventory_resource_total"], 27)
        self.assertEqual(c["inventory_resource_dispositions"]["m5-pkldoc"], 17)
        self.assertEqual(c["inventory_resource_dispositions"]["excluded-jvm-spi"], 2)
        self.assertEqual(c["stdlib_doc_package_info_items"], 8)
        self.assertEqual(c["corpus_formatter_snippet_cases"], 30)
        self.assertEqual(c["excluded_2026_11_20_java_api"], 8239)
        self.assertEqual(c["excluded_2026_11_20_kotlin_api"], 597)
        self.assertEqual(c["m5_dynamic_formatter_snippet_cases"], 30)
        self.assertEqual(c["m5_total_5lane_cases"], c["m5_static_cases"] + 30)

    def test_committed_manifest_matches_generated_bytes(self) -> None:
        manifest = m5_manifest.build_manifest(ROOT)
        expected = m5_manifest.encoded(manifest)
        committed = (ROOT / m5_manifest.OUTPUT_PATH).read_bytes()
        self.assertEqual(committed, expected)

    def test_validate_mappings_rejects_unmapped_case_and_bad_links(self) -> None:
        mappings = m5_manifest.load_json(ROOT / m5_manifest.MAPPINGS_PATH)
        cases_doc = m5_manifest.load_json(ROOT / m5_manifest.M5_CASES_PATH)
        static_cases = m5_differential.validate_static_cases(ROOT, cases_doc)

        # Unmapped static case must fail
        broken = copy.deepcopy(mappings)
        first_req = next(iter(broken["requirements"].values()))
        first_req["case_ids"] = first_req["case_ids"][1:]
        with self.assertRaisesRegex(ValueError, "unmapped static M5 cases"):
            m5_manifest.validate_mappings(ROOT, broken, static_cases)

        # Linked requirement with empty implementation_links must fail
        broken_empty = copy.deepcopy(mappings)
        req_key = next(iter(broken_empty["requirements"]))
        broken_empty["requirements"][req_key]["implementation_status"] = "linked"
        broken_empty["requirements"][req_key]["implementation_links"] = []
        with self.assertRaisesRegex(ValueError, "empty implementation_links"):
            m5_manifest.validate_mappings(ROOT, broken_empty, static_cases)

        # Path outside crates/ must fail
        broken_path = copy.deepcopy(mappings)
        broken_path["requirements"][req_key]["implementation_status"] = "linked"
        broken_path["requirements"][req_key]["implementation_links"] = [
            {"path": "tools/m5_manifest.py", "symbol": "build_manifest"}
        ]
        with self.assertRaisesRegex(ValueError, "invalid implementation link path"):
            m5_manifest.validate_mappings(ROOT, broken_path, static_cases)

        # Missing symbol inside an existing Rust file must fail
        broken_sym = copy.deepcopy(mappings)
        broken_sym["requirements"][req_key]["implementation_status"] = "linked"
        broken_sym["requirements"][req_key]["implementation_links"] = [
            {"path": "crates/pkl-runtime/src/lib.rs", "symbol": "NonExistentM5SymbolXYZ123"}
        ]
        with self.assertRaisesRegex(ValueError, "symbol missing"):
            m5_manifest.validate_mappings(ROOT, broken_sym, static_cases)

    def test_strict_coverage_rejects_java_only_and_pending_impl_links(self) -> None:
        cases_doc = m5_manifest.load_json(ROOT / m5_manifest.M5_CASES_PATH)
        mappings = m5_manifest.load_json(ROOT / m5_manifest.MAPPINGS_PATH)
        dyn_cases = m5_differential.enumerate_formatter_snippet_cases(ROOT)
        all_cases = cases_doc["cases"] + dyn_cases

        with tempfile.TemporaryDirectory() as td:
            rep_dir = Path(td) / "rep"
            rep_dir.mkdir()
            oracle_stub = {"exact_jdk_lock_match": True, "version": "0.32.0"}
            compat.write_json(rep_dir / "oracle.json", oracle_stub)
            (rep_dir / "cases.snapshot.json").write_bytes((ROOT / m5_manifest.M5_CASES_PATH).read_bytes())
            compat.write_json(rep_dir / "dynamic-cases.snapshot.json", {"schema_version": 1, "cases": dyn_cases})
            compat.write_json(rep_dir / "sources.snapshot.json", {"before": {"a": "1"}, "after": {"a": "1"}})

            results = []
            for c in all_cases:
                cdir = compat.case_directory(c["id"])
                folder = rep_dir / "cases" / cdir
                raw = {
                    "stdout": b"Pkl 0.32.0 (macOS 27.2, Java 21.0.13.1)\n" if c.get("comparison_mode") == "native-version" else b"ok\n",
                    "stderr": b"",
                    "exit_code": c["expected_exit"][0],
                    "files": {},
                    "timed_out": False,
                    "workspace_changes": {},
                    "elapsed_ns": 10,
                }
                jst = compat.store_capture(folder / "java", raw)
                results.append({
                    "id": c["id"],
                    "lane": c["lane"],
                    "exec_mode": c["exec_mode"],
                    "directory": cdir,
                    "status": "oracle-recorded",
                    "differences": [],
                    "expected_exit": c["expected_exit"],
                    "comparison_mode": c.get("comparison_mode", "raw"),
                    "java_assertion_failures": [],
                    "rust_assertion_failures": [],
                    "idempotency_ok": True if c["exec_mode"] == "format_snippet" else None,
                    "java": {"command": ["java"], "result": jst},
                    "rust": None,
                })

            lane_counts = {
                lane: {"oracle-recorded": sum(1 for r in results if r["lane"] == lane)}
                for lane in m5_differential.ALL_LANES
            }
            report = {
                "schema_version": 1,
                "baseline": "0.32.0",
                "mode": "java-only-oracle-recording",
                "cases_manifest_sha256": m5_differential.digest((rep_dir / "cases.snapshot.json").read_bytes()),
                "dynamic_cases_sha256": m5_differential.digest((rep_dir / "dynamic-cases.snapshot.json").read_bytes()),
                "sources_snapshot_sha256": m5_differential.digest((rep_dir / "sources.snapshot.json").read_bytes()),
                "source_stability": True,
                "lanes": list(m5_differential.ALL_LANES),
                "required_candidate_roles": ["api_probe", "pkl", "pkldoc"],
                "oracle": oracle_stub,
                "candidates": None,
                "selected_count": len(results),
                "status_counts": {"oracle-recorded": len(results)},
                "lane_status_counts": lane_counts,
                "results": results,
            }
            report["evidence_sha256"] = {
                str(p.relative_to(rep_dir).as_posix()): m5_differential.digest(p.read_bytes())
                for p in sorted(m5_differential.evidence_paths(rep_dir))
            }
            compat.write_json(rep_dir / "report.json", report)

            # Non-strict oracle coverage passes
            verdict_oracle = m5_manifest.verify_differential_coverage(ROOT, rep_dir, mappings, strict=False)
            self.assertTrue(verdict_oracle["passed"], verdict_oracle["failures"])
            self.assertEqual(verdict_oracle["requirement_count"], len(mappings["requirements"]))

            # Strict coverage MUST reject java-only recordings!
            with self.assertRaisesRegex(ValueError, "recorded-build"):
                m5_manifest.verify_differential_coverage(ROOT, rep_dir, mappings, strict=True)


if __name__ == "__main__":
    unittest.main()
