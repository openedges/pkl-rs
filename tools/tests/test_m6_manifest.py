#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Unit and gate-enforcement tests for tools/m6_manifest.py."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import tempfile
import unittest

import compat
import m6_differential
import m6_manifest

ROOT = Path(__file__).resolve().parents[2]


class M6ManifestTests(unittest.TestCase):
    def test_build_manifest_freshness_and_counts(self) -> None:
        manifest = m6_manifest.build_manifest(ROOT)
        c = manifest["counts"]
        self.assertIsNone(manifest["coverage_gate"])
        self.assertGreaterEqual(c["m6_requirement_groups"], 14)
        self.assertEqual(
            c["m6_total_cases"],
            c["m6_executable_cases"] + c["m6_capability_boundary_cases"],
        )
        self.assertGreater(c["m6_executable_cases"], 50)
        self.assertGreaterEqual(c["m6_capability_boundary_cases"], 6)
        self.assertEqual(set(c["m6_cases_by_lane"].keys()), set(m6_differential.ALL_LANES))
        self.assertEqual(c["m6_requirement_impl_status"], {"linked": c["m6_requirement_groups"]})

    def test_committed_manifest_matches_generated_bytes(self) -> None:
        manifest = m6_manifest.build_manifest(ROOT)
        expected = m6_manifest.encoded(manifest)
        committed = (ROOT / m6_manifest.OUTPUT_PATH).read_bytes()
        self.assertEqual(committed, expected)

    def test_validate_mappings_rejects_unmapped_and_invalid_links(self) -> None:
        mappings = m6_manifest.load_json(ROOT / m6_manifest.MAPPINGS_PATH)
        cases_doc = m6_manifest.load_json(ROOT / m6_manifest.M6_CASES_PATH)
        cases_by_id = m6_differential.validate_cases(ROOT, cases_doc)

        broken = copy.deepcopy(mappings)
        first_req = next(iter(broken["requirements"].values()))
        first_req["case_ids"] = first_req["case_ids"][1:]
        with self.assertRaisesRegex(ValueError, "unmapped M6 cases"):
            m6_manifest.validate_mappings(ROOT, broken, cases_by_id)

        broken_empty = copy.deepcopy(mappings)
        req_key = next(iter(broken_empty["requirements"]))
        broken_empty["requirements"][req_key]["implementation_status"] = "linked"
        broken_empty["requirements"][req_key]["implementation_links"] = []
        with self.assertRaisesRegex(ValueError, "empty implementation_links"):
            m6_manifest.validate_mappings(ROOT, broken_empty, cases_by_id)

        broken_path = copy.deepcopy(mappings)
        broken_path["requirements"][req_key]["implementation_status"] = "linked"
        broken_path["requirements"][req_key]["implementation_links"] = [
            {"path": "tools/m6_manifest.py", "symbol": "build_manifest"}
        ]
        with self.assertRaisesRegex(ValueError, "invalid implementation link path"):
            m6_manifest.validate_mappings(ROOT, broken_path, cases_by_id)

        broken_sym = copy.deepcopy(mappings)
        broken_sym["requirements"][req_key]["implementation_status"] = "linked"
        broken_sym["requirements"][req_key]["implementation_links"] = [
            {"path": "crates/pkl-runtime/src/lib.rs", "symbol": "NonExistentM6SymbolXYZ999"}
        ]
        with self.assertRaisesRegex(ValueError, "symbol missing"):
            m6_manifest.validate_mappings(ROOT, broken_sym, cases_by_id)

    def test_oracle_coverage_vs_strict_and_stale_cases_rejection(self) -> None:
        cases_doc = m6_manifest.load_json(ROOT / m6_manifest.M6_CASES_PATH)
        mappings = m6_manifest.load_json(ROOT / m6_manifest.MAPPINGS_PATH)

        with tempfile.TemporaryDirectory() as td:
            rep_dir = Path(td) / "oracle_rep"
            rep_dir.mkdir()
            oracle_stub = {"exact_jdk_lock_match": True, "version": "0.32.0"}
            compat.write_json(rep_dir / "oracle.json", oracle_stub)
            (rep_dir / "cases.snapshot.json").write_bytes((ROOT / m6_manifest.M6_CASES_PATH).read_bytes())
            compat.write_json(rep_dir / "sources.snapshot.json", {"before": {"a": "1"}, "after": {"a": "1"}})

            results = []
            for c in cases_doc["cases"]:
                cdir = compat.case_directory(c["id"])
                raw = {
                    "stdout": b"ok\n",
                    "stderr": b"",
                    "exit_code": c["expected_exit"][0],
                    "files": {},
                    "timed_out": False,
                    "workspace_changes": {},
                    "elapsed_ns": 10,
                }
                jst = compat.store_capture(rep_dir / "cases" / cdir / "java", raw)
                compat.write_json(
                    rep_dir / "cases" / cdir / "java/assertions.json",
                    {"java_assertion_failures": []},
                )
                results.append({
                    "id": c["id"],
                    "lane": c["lane"],
                    "target": c["target"],
                    "exec_mode": c["exec_mode"],
                    "directory": cdir,
                    "status": "oracle-recorded",
                    "differences": [],
                    "expected_exit": c["expected_exit"],
                    "capability_boundary": bool(c.get("capability_boundary", False)),
                    "expected_capability": c.get("expected_capability"),
                    "capability_gap": None,
                    "java_assertion_failures": [],
                    "candidate_assertion_failures": [],
                    "telemetry": None,
                    "per_target_oracle_matches": None,
                    "shared_engine_match": None,
                    "java": {"command": ["java"], "result": jst},
                    "candidate": None,
                    "cross_target": None,
                })

            lane_counts = {
                lane: {"oracle-recorded": sum(1 for r in results if r["lane"] == lane)}
                for lane in m6_differential.ALL_LANES
            }
            cross_total = sum(1 for r in results if r["exec_mode"] == "cross_target_parity")
            report = {
                "schema_version": 1,
                "baseline": "0.32.0",
                "mode": "java-only-oracle-recording",
                "cases_manifest_sha256": m6_differential.digest((rep_dir / "cases.snapshot.json").read_bytes()),
                "sources_snapshot_sha256": m6_differential.digest((rep_dir / "sources.snapshot.json").read_bytes()),
                "source_stability": True,
                "lanes": list(m6_differential.ALL_LANES),
                "required_candidate_roles": ["napi_pkg", "pkl", "pkldoc", "wasm_pkg"],
                "oracle": oracle_stub,
                "candidates": None,
                "browser_provenance": None,
                "server_self_test": None,
                "selected_count": len(results),
                "executable_case_count": sum(1 for r in results if not r["capability_boundary"]),
                "capability_boundary_case_count": sum(1 for r in results if r["capability_boundary"]),
                "compatibility_pass_count": 0,
                "capability_blocked_count": 0,
                "status_counts": {"oracle-recorded": len(results)},
                "lane_status_counts": lane_counts,
                "cross_target_summary": {
                    "total": cross_total,
                    "per_target_passed": None,
                    "shared_engine_passed": None,
                    "all_targets_passed": None,
                },
                "conversion_telemetry": {},
                "results": results,
            }
            report["evidence_sha256"] = {
                str(p.relative_to(rep_dir).as_posix()): m6_differential.digest(p.read_bytes())
                for p in sorted(m6_differential.evidence_paths(rep_dir))
            }
            compat.write_json(rep_dir / "report.json", report)

            verdict_oracle = m6_manifest.verify_differential_coverage(ROOT, rep_dir, mappings, strict=False)
            self.assertTrue(verdict_oracle["passed"], verdict_oracle["failures"])
            self.assertEqual(verdict_oracle["compatibility_pass_count"], 0)

            with self.assertRaisesRegex(ValueError, "oracle-only"):
                m6_manifest.verify_differential_coverage(ROOT, rep_dir, mappings, strict=True)

            # Stale report whose cases.snapshot.json differs from live cases.json MUST be rejected!
            stale_snap = (rep_dir / "cases.snapshot.json").read_bytes() + b"\n"
            (rep_dir / "cases.snapshot.json").write_bytes(stale_snap)
            report["cases_manifest_sha256"] = m6_differential.digest(stale_snap)
            report["evidence_sha256"]["cases.snapshot.json"] = report["cases_manifest_sha256"]
            compat.write_json(rep_dir / "report.json", report)
            with self.assertRaisesRegex(ValueError, "cases_manifest_sha256 is stale"):
                m6_manifest.verify_differential_coverage(ROOT, rep_dir, mappings, strict=False)

    def test_strict_coverage_enforces_linked_implementation_status(self) -> None:
        cases_doc = m6_manifest.load_json(ROOT / m6_manifest.M6_CASES_PATH)
        mappings = m6_manifest.load_json(ROOT / m6_manifest.MAPPINGS_PATH)
        pinned_pw = json.loads((ROOT / m6_differential.PLAYWRIGHT_PACKAGE_JSON).read_bytes())["devDependencies"]["playwright"]

        with tempfile.TemporaryDirectory() as td:
            rep_dir = Path(td) / "strict_rep"
            rep_dir.mkdir()
            artifacts = rep_dir / "artifacts"
            artifacts.mkdir()
            cands = {}
            for role, fname in (("pkl", "pkl"), ("pkldoc", "pkldoc"), ("wasm_pkg", "wasm_pkg.tgz"), ("napi_pkg", "napi_pkg.tgz")):
                (artifacts / fname).write_bytes(f"FAKE-{fname}".encode())
                (rep_dir / f"build-{role}.stdout.bin").write_bytes(b"")
                (rep_dir / f"build-{role}.stderr.bin").write_bytes(b"ok\n")
                entry = {
                    "name": fname,
                    "sha256": m6_differential.digest((artifacts / fname).read_bytes()),
                    "source_binding": "recorded-build",
                    "build": {
                        "role": role,
                        "stdout_sha256": m6_differential.digest(b""),
                        "stderr_sha256": m6_differential.digest(b"ok\n"),
                    },
                }
                if role in ("wasm_pkg", "napi_pkg"):
                    (rep_dir / f"pack-{role}.stdout.bin").write_bytes(f"{fname}\n".encode())
                    (rep_dir / f"pack-{role}.stderr.bin").write_bytes(b"")
                    entry["pack"] = {
                        "stdout_sha256": m6_differential.digest(f"{fname}\n".encode()),
                        "stderr_sha256": m6_differential.digest(b""),
                    }
                cands[role] = entry

            oracle_stub = {"exact_jdk_lock_match": True, "version": "0.32.0"}
            compat.write_json(rep_dir / "oracle.json", oracle_stub)
            (rep_dir / "cases.snapshot.json").write_bytes((ROOT / m6_manifest.M6_CASES_PATH).read_bytes())
            compat.write_json(rep_dir / "sources.snapshot.json", {"before": {"a": "1"}, "after": {"a": "1"}})
            browser_prov = {
                "browser": "156.0.8078.4",
                "browserExecutableSha256": "a" * 64,
                "browserErrors": [],
                "playwright": pinned_pw,
                "playwrightCore": pinned_pw,
                "playwrightPin": pinned_pw,
            }
            compat.write_json(rep_dir / "browser_provenance.json", browser_prov)

            results = []
            lane_counts: dict[str, dict[str, int]] = {lane: {} for lane in m6_differential.ALL_LANES}
            status_counts: dict[str, int] = {}
            cross_total = 0

            for c in cases_doc["cases"]:
                cdir = compat.case_directory(c["id"])
                is_cap = bool(c.get("capability_boundary", False))
                raw_ok = {
                    "stdout": b"ok\n",
                    "stderr": b"",
                    "exit_code": c["expected_exit"][0],
                    "files": {},
                    "timed_out": False,
                    "workspace_changes": {},
                    "elapsed_ns": 10,
                }
                jst = compat.store_capture(rep_dir / "cases" / cdir / "java", raw_ok)
                compat.write_json(rep_dir / "cases" / cdir / "java/assertions.json", {"java_assertion_failures": []})

                if c["exec_mode"] == "cross_target_parity":
                    cross_total += 1
                    ep_stored = {
                        ep: compat.store_capture(rep_dir / "cases" / cdir / ep, raw_ok)
                        for ep in m6_differential.CROSS_TARGET_ENDPOINTS
                    }
                    compat.write_json(rep_dir / "cases" / cdir / "cross_target_meta.json", {"candidate_assertion_failures": []})
                    st = "pass"
                    results.append({
                        "id": c["id"],
                        "lane": c["lane"],
                        "target": c["target"],
                        "exec_mode": c["exec_mode"],
                        "directory": cdir,
                        "status": st,
                        "differences": [],
                        "expected_exit": c["expected_exit"],
                        "capability_boundary": False,
                        "expected_capability": None,
                        "capability_gap": None,
                        "java_assertion_failures": [],
                        "candidate_assertion_failures": [],
                        "telemetry": None,
                        "per_target_oracle_matches": {ep: True for ep in m6_differential.CROSS_TARGET_ENDPOINTS},
                        "shared_engine_match": True,
                        "java": {"command": ["java"], "result": jst},
                        "candidate": None,
                        "cross_target": ep_stored,
                    })
                elif is_cap:
                    cap_name = c["expected_capability"]
                    raw_cap = {
                        "stdout": b"",
                        "stderr": f"Capability {cap_name} unavailable\n".encode(),
                        "exit_code": 1,
                        "files": {},
                        "timed_out": False,
                        "workspace_changes": {},
                        "elapsed_ns": 10,
                    }
                    rst = compat.store_capture(rep_dir / "cases" / cdir / "candidate", raw_cap)
                    gap = {
                        "expected_capability": cap_name,
                        "capability_reported_false": True,
                        "error_code": "capability-unavailable",
                        "error_kind": "capability-unavailable",
                        "error_capability": cap_name,
                        "error_message": f"Capability {cap_name} unavailable",
                        "capability_contract_verified": True,
                    }
                    compat.write_json(
                        rep_dir / "cases" / cdir / "candidate/runner_meta.json",
                        {"candidate_assertion_failures": [], "capability_gap": gap, "telemetry": None},
                    )
                    st = "capability-blocked"
                    results.append({
                        "id": c["id"],
                        "lane": c["lane"],
                        "target": c["target"],
                        "exec_mode": c["exec_mode"],
                        "directory": cdir,
                        "status": st,
                        "differences": [],
                        "expected_exit": c["expected_exit"],
                        "capability_boundary": True,
                        "expected_capability": cap_name,
                        "capability_gap": gap,
                        "java_assertion_failures": [],
                        "candidate_assertion_failures": [],
                        "telemetry": None,
                        "per_target_oracle_matches": None,
                        "shared_engine_match": None,
                        "java": {"command": ["java"], "result": jst},
                        "candidate": {"result": rst},
                        "cross_target": None,
                    })
                else:
                    rst = compat.store_capture(rep_dir / "cases" / cdir / "candidate", raw_ok)
                    compat.write_json(
                        rep_dir / "cases" / cdir / "candidate/runner_meta.json",
                        {"candidate_assertion_failures": [], "capability_gap": None, "telemetry": None},
                    )
                    st = "pass"
                    results.append({
                        "id": c["id"],
                        "lane": c["lane"],
                        "target": c["target"],
                        "exec_mode": c["exec_mode"],
                        "directory": cdir,
                        "status": st,
                        "differences": [],
                        "expected_exit": c["expected_exit"],
                        "capability_boundary": False,
                        "expected_capability": None,
                        "capability_gap": None,
                        "java_assertion_failures": [],
                        "candidate_assertion_failures": [],
                        "telemetry": None,
                        "per_target_oracle_matches": None,
                        "shared_engine_match": None,
                        "java": {"command": ["java"], "result": jst},
                        "candidate": {"result": rst},
                        "cross_target": None,
                    })

                status_counts[st] = status_counts.get(st, 0) + 1
                lane_counts[c["lane"]][st] = lane_counts[c["lane"]].get(st, 0) + 1

            report = {
                "schema_version": 1,
                "baseline": "0.32.0",
                "mode": "differential",
                "cases_manifest_sha256": m6_differential.digest((rep_dir / "cases.snapshot.json").read_bytes()),
                "sources_snapshot_sha256": m6_differential.digest((rep_dir / "sources.snapshot.json").read_bytes()),
                "source_stability": True,
                "lanes": list(m6_differential.ALL_LANES),
                "required_candidate_roles": ["napi_pkg", "pkl", "pkldoc", "wasm_pkg"],
                "oracle": oracle_stub,
                "candidates": cands,
                "browser_provenance": browser_prov,
                "server_self_test": {"passed": True},
                "selected_count": len(results),
                "executable_case_count": sum(1 for r in results if not r["capability_boundary"]),
                "capability_boundary_case_count": sum(1 for r in results if r["capability_boundary"]),
                "compatibility_pass_count": status_counts.get("pass", 0),
                "capability_blocked_count": status_counts.get("capability-blocked", 0),
                "status_counts": dict(sorted(status_counts.items())),
                "lane_status_counts": {k: dict(sorted(v.items())) for k, v in lane_counts.items()},
                "cross_target_summary": {
                    "total": cross_total,
                    "per_target_passed": {ep: cross_total for ep in m6_differential.CROSS_TARGET_ENDPOINTS},
                    "shared_engine_passed": cross_total,
                    "all_targets_passed": cross_total,
                },
                "conversion_telemetry": {},
                "results": results,
            }
            report["evidence_sha256"] = {
                str(p.relative_to(rep_dir).as_posix()): m6_differential.digest(p.read_bytes())
                for p in sorted(m6_differential.evidence_paths(rep_dir))
            }
            compat.write_json(rep_dir / "report.json", report)

            verdict_ok = m6_manifest.verify_differential_coverage(ROOT, rep_dir, mappings, strict=True)
            self.assertTrue(verdict_ok["passed"], verdict_ok["failures"])
            self.assertEqual(verdict_ok["requirements_passed"], verdict_ok["requirement_count"])
            self.assertEqual(verdict_ok["compatibility_pass_count"], 80)
            self.assertEqual(verdict_ok["capability_blocked_count"], 6)

            pending_mappings = copy.deepcopy(mappings)
            first_req = next(iter(pending_mappings["requirements"]))
            pending_mappings["requirements"][first_req]["implementation_status"] = "pending-wave2"
            pending_mappings["requirements"][first_req]["implementation_links"] = []
            verdict_pending = m6_manifest.verify_differential_coverage(ROOT, rep_dir, pending_mappings, strict=True)
            self.assertFalse(verdict_pending["passed"])
            self.assertTrue(
                any("implementation_status is 'pending-wave2'" in f for f in verdict_pending["failures"]),
                verdict_pending["failures"],
            )


if __name__ == "__main__":
    unittest.main()
