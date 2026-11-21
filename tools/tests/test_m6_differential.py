#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Unit, honesty, and tamper-detection tests for tools/m6_differential.py."""
from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

import compat
import m6_differential

ROOT = Path(__file__).resolve().parents[2]
PINNED_PW = json.loads((ROOT / m6_differential.PLAYWRIGHT_PACKAGE_JSON).read_bytes())["devDependencies"]["playwright"]


def _cap(stdout: bytes, stderr: bytes = b"", exit_code: int = 0, files: dict | None = None, timed_out: bool = False) -> dict:
    return {
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": exit_code,
        "files": files or {},
        "timed_out": timed_out,
        "workspace_changes": {},
        "elapsed_ns": 100,
    }


def _build_synthetic_sealed_report(
    rep_dir: Path,
    *,
    source_binding: str = "recorded-build",
    exact_jdk_lock_match: bool = True,
    source_stability: bool = True,
    include_cross_target: bool = True,
    playwright_version: str = PINNED_PW,
) -> None:
    rep_dir.mkdir(parents=True)
    artifacts = rep_dir / "artifacts"
    artifacts.mkdir()
    for fname, role_key in (("pkl", "pkl"), ("wasm_pkg.tgz", "wasm_pkg"), ("napi_pkg.tgz", "napi_pkg")):
        (artifacts / fname).write_bytes(f"FAKE-{fname}".encode())
        (rep_dir / f"build-{role_key}.stdout.bin").write_bytes(b"")
        (rep_dir / f"build-{role_key}.stderr.bin").write_bytes(b"ok\n")
        if role_key in ("wasm_pkg", "napi_pkg"):
            (rep_dir / f"pack-{role_key}.stdout.bin").write_bytes(f"{fname}\n".encode())
            (rep_dir / f"pack-{role_key}.stderr.bin").write_bytes(b"")

    oracle_stub = {"exact_jdk_lock_match": exact_jdk_lock_match, "version": "0.32.0"}
    compat.write_json(rep_dir / "oracle.json", oracle_stub)

    cases = [
        {
            "id": "m6:synth:exec-raw",
            "lane": "wasm-browser",
            "target": "wasm-browser",
            "exec_mode": "js_api",
            "oracle_mode": "probe",
            "oracle_command": ["eval-version"],
            "expected_exit": [0],
            "capability_boundary": False,
            "expected_capability": None,
        },
        {
            "id": "m6:synth:cap-gap-native-fs",
            "lane": "wasm-browser",
            "target": "wasm-browser",
            "exec_mode": "js_api",
            "oracle_mode": "probe",
            "oracle_command": ["eval-version"],
            "expected_exit": [0],
            "capability_boundary": True,
            "expected_capability": "nativeFs",
        },
    ]
    lanes = ["wasm-browser"]
    roles = ["wasm_pkg"]
    if include_cross_target:
        cases.append({
            "id": "m6:synth:parity",
            "lane": "cross-target-parity",
            "target": "cross-target",
            "exec_mode": "cross_target_parity",
            "oracle_mode": "cli",
            "oracle_command": ["eval", "-"],
            "expected_exit": [0],
            "capability_boundary": False,
            "expected_capability": None,
        })
        lanes.append("cross-target-parity")
        roles = ["napi_pkg", "pkl", "wasm_pkg"]

    compat.write_json(rep_dir / "cases.snapshot.json", {"schema_version": 1, "baseline": "0.32.0", "cases": cases})
    compat.write_json(
        rep_dir / "sources.snapshot.json",
        {"before": {"Cargo.toml": "aaa"}, "after": {"Cargo.toml": "aaa" if source_stability else "bbb"}},
    )
    browser_prov = {
        "browser": "156.0.8078.4",
        "browserExecutableSha256": "a" * 64,
        "browserErrors": [],
        "playwright": playwright_version,
        "playwrightCore": playwright_version,
        "playwrightPin": playwright_version,
    }
    compat.write_json(rep_dir / "browser_provenance.json", browser_prov)

    results = []

    # Case 1: normal executable raw-byte match
    c1_dir = compat.case_directory("m6:synth:exec-raw")
    c1_cap = _cap(b"hello\n", files={"generated/a.txt": b"alpha\n"})
    j1 = compat.store_capture(rep_dir / "cases" / c1_dir / "java", c1_cap)
    compat.write_json(rep_dir / "cases" / c1_dir / "java/assertions.json", {"java_assertion_failures": []})
    r1 = compat.store_capture(rep_dir / "cases" / c1_dir / "candidate", c1_cap)
    compat.write_json(
        rep_dir / "cases" / c1_dir / "candidate/runner_meta.json",
        {"candidate_assertion_failures": [], "capability_gap": None, "telemetry": None},
    )
    results.append({
        "id": "m6:synth:exec-raw",
        "lane": "wasm-browser",
        "target": "wasm-browser",
        "exec_mode": "js_api",
        "directory": c1_dir,
        "status": "pass",
        "differences": [],
        "expected_exit": [0],
        "capability_boundary": False,
        "expected_capability": None,
        "capability_gap": None,
        "java_assertion_failures": [],
        "candidate_assertion_failures": [],
        "telemetry": None,
        "per_target_oracle_matches": None,
        "shared_engine_match": None,
        "java": {"command": ["java"], "result": j1},
        "candidate": {"result": r1},
        "cross_target": None,
    })

    # Case 2: declared capability-boundary case -> MUST be `capability-blocked`, NEVER `pass`!
    c2_dir = compat.case_directory("m6:synth:cap-gap-native-fs")
    j2 = compat.store_capture(rep_dir / "cases" / c2_dir / "java", _cap(b"secret = 1\n"))
    compat.write_json(rep_dir / "cases" / c2_dir / "java/assertions.json", {"java_assertion_failures": []})
    r2 = compat.store_capture(
        rep_dir / "cases" / c2_dir / "candidate",
        _cap(b"", stderr=b"Host capability `nativeFs` is unavailable in wasm-browser.\n", exit_code=1),
    )
    cap_gap_obj = {
        "expected_capability": "nativeFs",
        "capability_reported_false": True,
        "error_code": "capability-unavailable",
        "error_kind": "capability-unavailable",
        "error_capability": "nativeFs",
        "error_message": "Host capability `nativeFs` is unavailable in wasm-browser.",
        "capability_contract_verified": True,
    }
    compat.write_json(
        rep_dir / "cases" / c2_dir / "candidate/runner_meta.json",
        {"candidate_assertion_failures": [], "capability_gap": cap_gap_obj, "telemetry": None},
    )
    results.append({
        "id": "m6:synth:cap-gap-native-fs",
        "lane": "wasm-browser",
        "target": "wasm-browser",
        "exec_mode": "js_api",
        "directory": c2_dir,
        "status": "capability-blocked",
        "differences": [],
        "expected_exit": [0],
        "capability_boundary": True,
        "expected_capability": "nativeFs",
        "capability_gap": cap_gap_obj,
        "java_assertion_failures": [],
        "candidate_assertion_failures": [],
        "telemetry": None,
        "per_target_oracle_matches": None,
        "shared_engine_match": None,
        "java": {"command": ["java"], "result": j2},
        "candidate": {"result": r2},
        "cross_target": None,
    })

    # Case 3: 4-way cross-target parity
    cross_summary = None
    if include_cross_target:
        c3_dir = compat.case_directory("m6:synth:parity")
        c3_cap = _cap(b"x = 42\n")
        j3 = compat.store_capture(rep_dir / "cases" / c3_dir / "java", c3_cap)
        compat.write_json(rep_dir / "cases" / c3_dir / "java/assertions.json", {"java_assertion_failures": []})
        ep_stored = {
            ep: compat.store_capture(rep_dir / "cases" / c3_dir / ep, c3_cap)
            for ep in m6_differential.CROSS_TARGET_ENDPOINTS
        }
        compat.write_json(rep_dir / "cases" / c3_dir / "cross_target_meta.json", {"candidate_assertion_failures": []})
        results.append({
            "id": "m6:synth:parity",
            "lane": "cross-target-parity",
            "target": "cross-target",
            "exec_mode": "cross_target_parity",
            "directory": c3_dir,
            "status": "pass",
            "differences": [],
            "expected_exit": [0],
            "capability_boundary": False,
            "expected_capability": None,
            "capability_gap": None,
            "java_assertion_failures": [],
            "candidate_assertion_failures": [],
            "telemetry": None,
            "per_target_oracle_matches": {ep: True for ep in m6_differential.CROSS_TARGET_ENDPOINTS},
            "shared_engine_match": True,
            "java": {"command": ["java"], "result": j3},
            "candidate": None,
            "cross_target": ep_stored,
        })
        cross_summary = {
            "total": 1,
            "per_target_passed": {ep: 1 for ep in m6_differential.CROSS_TARGET_ENDPOINTS},
            "shared_engine_passed": 1,
            "all_targets_passed": 1,
        }

    cands = {}
    for role, fname in (("pkl", "pkl"), ("wasm_pkg", "wasm_pkg.tgz"), ("napi_pkg", "napi_pkg.tgz")):
        if role in roles:
            entry = {
                "name": fname,
                "sha256": m6_differential.digest((artifacts / fname).read_bytes()),
                "source_binding": source_binding,
                "build": {
                    "role": role,
                    "stdout_sha256": m6_differential.digest(b""),
                    "stderr_sha256": m6_differential.digest(b"ok\n"),
                },
            }
            if role in ("wasm_pkg", "napi_pkg"):
                entry["pack"] = {
                    "stdout_sha256": m6_differential.digest(f"{fname}\n".encode()),
                    "stderr_sha256": m6_differential.digest(b""),
                }
            cands[role] = entry

    pass_n = sum(1 for r in results if r["status"] == "pass")
    cap_n = sum(1 for r in results if r["status"] == "capability-blocked")
    status_counts = {"capability-blocked": cap_n, "pass": pass_n}
    lane_counts = {"wasm-browser": {"capability-blocked": 1, "pass": 1}}
    if include_cross_target:
        lane_counts["cross-target-parity"] = {"pass": 1}

    report = {
        "schema_version": 1,
        "baseline": "0.32.0",
        "mode": "differential",
        "cases_manifest_sha256": m6_differential.digest((rep_dir / "cases.snapshot.json").read_bytes()),
        "sources_snapshot_sha256": m6_differential.digest((rep_dir / "sources.snapshot.json").read_bytes()),
        "source_stability": source_stability,
        "lanes": lanes,
        "required_candidate_roles": roles,
        "oracle": oracle_stub,
        "candidates": cands,
        "browser_provenance": browser_prov,
        "server_self_test": {"passed": True},
        "selected_count": len(results),
        "executable_case_count": sum(1 for r in results if not r["capability_boundary"]),
        "capability_boundary_case_count": sum(1 for r in results if r["capability_boundary"]),
        "compatibility_pass_count": pass_n,
        "capability_blocked_count": cap_n,
        "status_counts": status_counts,
        "lane_status_counts": lane_counts,
        "cross_target_summary": cross_summary,
        "conversion_telemetry": {},
        "results": results,
    }
    report["evidence_sha256"] = {
        str(p.relative_to(rep_dir).as_posix()): m6_differential.digest(p.read_bytes())
        for p in sorted(m6_differential.evidence_paths(rep_dir))
    }
    compat.write_json(rep_dir / "report.json", report)


class M6DifferentialTests(unittest.TestCase):
    def test_raw_byte_mismatch_never_normalized(self) -> None:
        case = {"id": "c1", "exec_mode": "js_api", "expected_exit": [0], "capability_boundary": False}
        exp = _cap(b"a = 1\n", files={"generated/x.txt": b"abc"})

        st, diffs = m6_differential.classify_result(case, exp, _cap(b"a = 1 \n", files={"generated/x.txt": b"abc"}))
        self.assertEqual(st, "mismatch")
        self.assertEqual(diffs, ["stdout"])

        st2, diffs2 = m6_differential.classify_result(case, exp, _cap(b"a = 1", files={"generated/x.txt": b"abc"}))
        self.assertEqual(st2, "mismatch")
        self.assertEqual(diffs2, ["stdout"])

        st3, diffs3 = m6_differential.classify_result(case, exp, _cap(b"a = 1\n", files={"generated/x.txt": b"abd"}))
        self.assertEqual(st3, "mismatch")
        self.assertEqual(diffs3, ["files/generated/x.txt"])

        st4, diffs4 = m6_differential.classify_result(case, exp, _cap(b"a = 1\n", stderr=b"warn\n", files={"generated/x.txt": b"abc"}))
        self.assertEqual(st4, "mismatch")
        self.assertEqual(diffs4, ["stderr"])

    def test_missing_capability_never_counted_as_pass(self) -> None:
        cap_case = {
            "id": "cap1",
            "exec_mode": "js_api",
            "expected_exit": [0],
            "capability_boundary": True,
            "expected_capability": "nativeFs",
        }
        exp = _cap(b"x = 1\n")
        cand = _cap(b"", stderr=b"unavailable\n", exit_code=1)
        verified_gap = {
            "expected_capability": "nativeFs",
            "capability_reported_false": True,
            "error_code": "capability-unavailable",
            "error_capability": "nativeFs",
            "capability_contract_verified": True,
        }
        st, diffs = m6_differential.classify_result(cap_case, exp, cand, capability_gap=verified_gap)
        self.assertEqual(st, "capability-blocked")
        self.assertEqual(diffs, [])

        # Must require non-zero exit_code AND non-empty stderr!
        st_exit0, diffs_exit0 = m6_differential.classify_result(cap_case, exp, _cap(b"", stderr=b"x\n", exit_code=0), capability_gap=verified_gap)
        self.assertEqual(st_exit0, "mismatch")
        self.assertIn("capability_boundary_unexpected_exit_0", diffs_exit0)

        st_nostderr, diffs_nostderr = m6_differential.classify_result(cap_case, exp, _cap(b"", stderr=b"", exit_code=1), capability_gap=verified_gap)
        self.assertEqual(st_nostderr, "mismatch")
        self.assertIn("capability_boundary_empty_stderr", diffs_nostderr)

        # False capability_reported_false -> mismatch
        bad_gap = dict(verified_gap, capability_reported_false=False)
        st_bad, diffs_bad = m6_differential.classify_result(cap_case, exp, cand, capability_gap=bad_gap)
        self.assertEqual(st_bad, "mismatch")
        self.assertIn("capability_contract_unmet:nativeFs", diffs_bad)

        # Required executable case returning capability-unavailable -> mismatch (NEVER capability-blocked!)
        req_case = {
            "id": "req1",
            "exec_mode": "js_api",
            "expected_exit": [0],
            "capability_boundary": False,
            "expected_capability": None,
        }
        st_req, diffs_req = m6_differential.classify_result(req_case, exp, cand, capability_gap=verified_gap)
        self.assertEqual(st_req, "mismatch")
        self.assertTrue(any("unexpected_capability_unavailable" in d for d in diffs_req))

    def test_cross_target_parity_never_masks_single_target_diff(self) -> None:
        case = {"id": "p1", "exec_mode": "cross_target_parity", "expected_exit": [0], "capability_boundary": False}
        exp = _cap(b"v = 1\n")
        caps = {
            "native": _cap(b"v = 1\n"),
            "napi_node": _cap(b"v = 1\n"),
            "wasm_node": _cap(b"v = 1\n"),
            "wasm_browser": _cap(b"v = 2\n"),
        }
        st, diffs = m6_differential.classify_result(case, exp, None, cross_target_captures=caps)
        self.assertEqual(st, "mismatch")
        self.assertEqual(diffs, ["wasm_browser:stdout"])

    def test_verify_report_passes_and_detects_all_tampering(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            rep = Path(td) / "good"
            _build_synthetic_sealed_report(rep)
            verified = m6_differential.verify_report(rep, strict=False)
            self.assertEqual(verified["compatibility_pass_count"], 2)
            self.assertEqual(verified["capability_blocked_count"], 1)
            m6_differential.verify_report(rep, strict=True)

            orig_report = (rep / "report.json").read_bytes()

            # Tamper 1: modify 1 byte in candidate stdout.bin
            c1_dir = compat.case_directory("m6:synth:exec-raw")
            p = rep / "cases" / c1_dir / "candidate/stdout.bin"
            orig_stdout = p.read_bytes()
            p.write_bytes(b"hell0\n")
            with self.assertRaisesRegex(ValueError, "evidence file modified"):
                m6_differential.verify_report(rep, strict=False)
            p.write_bytes(orig_stdout)

            # Tamper 2: inject rogue unsealed file
            rogue = rep / "rogue.txt"
            rogue.write_text("sneaky", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "evidence file set differs"):
                m6_differential.verify_report(rep, strict=False)
            rogue.unlink()

            # Tamper 3: relabel capability-blocked case as pass in report.json!
            t3 = json.loads(orig_report)
            t3["results"][1]["status"] = "pass"
            t3["compatibility_pass_count"] = 3
            t3["capability_blocked_count"] = 0
            t3["status_counts"] = {"pass": 3}
            compat.write_json(rep / "report.json", t3)
            with self.assertRaisesRegex(ValueError, "recomputed status/differences disagree"):
                m6_differential.verify_report(rep, strict=False)
            (rep / "report.json").write_bytes(orig_report)

            # Tamper 4: forge capability_gap in report.json without matching sealed runner_meta.json!
            t4 = json.loads(orig_report)
            t4["results"][1]["capability_gap"]["error_message"] = "forged"
            compat.write_json(rep / "report.json", t4)
            with self.assertRaisesRegex(ValueError, "capability_gap tampered"):
                m6_differential.verify_report(rep, strict=False)
            (rep / "report.json").write_bytes(orig_report)

            # Tamper 5: forge/clear candidate_assertion_failures in report.json vs sealed runner_meta.json!
            t5 = json.loads(orig_report)
            t5["results"][0]["candidate_assertion_failures"] = ["target:event_loop_blocked"]
            compat.write_json(rep / "report.json", t5)
            with self.assertRaisesRegex(ValueError, "candidate_assertion_failures tampered"):
                m6_differential.verify_report(rep, strict=False)
            (rep / "report.json").write_bytes(orig_report)

            # Tamper 6: tamper java_assertion_failures in report.json vs sealed java/assertions.json!
            t6 = json.loads(orig_report)
            t6["results"][0]["java_assertion_failures"] = ["assertion:forged"]
            compat.write_json(rep / "report.json", t6)
            with self.assertRaisesRegex(ValueError, "java_assertion_failures tampered"):
                m6_differential.verify_report(rep, strict=False)
            (rep / "report.json").write_bytes(orig_report)

            # Tamper 7: tamper cross_target_summary in report.json
            t7 = json.loads(orig_report)
            t7["cross_target_summary"]["all_targets_passed"] = 99
            compat.write_json(rep / "report.json", t7)
            with self.assertRaisesRegex(ValueError, "cross_target_summary disagrees"):
                m6_differential.verify_report(rep, strict=False)
            (rep / "report.json").write_bytes(orig_report)

            # Tamper 8: tamper browser_provenance in report.json vs sealed browser_provenance.json
            t8 = json.loads(orig_report)
            t8["browser_provenance"]["browser"] = "forged"
            compat.write_json(rep / "report.json", t8)
            with self.assertRaisesRegex(ValueError, "browser_provenance.json"):
                m6_differential.verify_report(rep, strict=False)
            (rep / "report.json").write_bytes(orig_report)

            # Tamper 9: modify sealed runner_meta.json directly -> caught by evidence_sha256!
            meta_p = rep / "cases" / c1_dir / "candidate/runner_meta.json"
            orig_meta = meta_p.read_bytes()
            meta_p.write_text('{"tampered": true}\n', encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "evidence file modified"):
                m6_differential.verify_report(rep, strict=False)
            meta_p.write_bytes(orig_meta)

            # Strict rejects wrong Playwright pin!
            rep_pw = Path(td) / "bad_pw"
            _build_synthetic_sealed_report(rep_pw, playwright_version="0.0.0")
            m6_differential.verify_report(rep_pw, strict=False)
            with self.assertRaisesRegex(ValueError, "pinned Playwright"):
                m6_differential.verify_report(rep_pw, strict=True)

            # Strict rejects unbound-caller-supplied
            rep_unbound = Path(td) / "unbound"
            _build_synthetic_sealed_report(rep_unbound, source_binding="unbound-caller-supplied")
            m6_differential.verify_report(rep_unbound, strict=False)
            with self.assertRaisesRegex(ValueError, "recorded-build"):
                m6_differential.verify_report(rep_unbound, strict=True)

            # Strict rejects exact_jdk_lock_match == False
            rep_jdk = Path(td) / "bad_jdk"
            _build_synthetic_sealed_report(rep_jdk, exact_jdk_lock_match=False)
            m6_differential.verify_report(rep_jdk, strict=False)
            with self.assertRaisesRegex(ValueError, "exact_jdk_lock_match"):
                m6_differential.verify_report(rep_jdk, strict=True)

            # Strict rejects source_stability == False
            rep_stab = Path(td) / "unstable"
            _build_synthetic_sealed_report(rep_stab, source_stability=False)
            m6_differential.verify_report(rep_stab, strict=False)
            with self.assertRaisesRegex(ValueError, "source_stability"):
                m6_differential.verify_report(rep_stab, strict=True)


if __name__ == "__main__":
    unittest.main()
