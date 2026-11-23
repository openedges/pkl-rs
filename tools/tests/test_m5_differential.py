#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Unit and tamper-detection tests for tools/m5_differential.py."""
from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

import compat
import m5_differential


def _make_capture(stdout: bytes, stderr: bytes = b"", exit_code: int = 0, files: dict | None = None) -> dict:
    return {
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": exit_code,
        "files": files or {},
        "timed_out": False,
        "workspace_changes": {},
        "elapsed_ns": 100,
    }


def _build_synthetic_sealed_report(rep_dir: Path, *, source_binding: str = "recorded-build") -> None:
    rep_dir.mkdir(parents=True)
    artifacts = rep_dir / "artifacts"
    artifacts.mkdir()
    fake_bin = artifacts / "pkl"
    fake_bin.write_bytes(b"FAKE-ELF")
    (rep_dir / "build-pkl.stdout.bin").write_bytes(b"")
    (rep_dir / "build-pkl.stderr.bin").write_bytes(b"Finished release\n")

    oracle_stub = {"exact_jdk_lock_match": True, "version": "0.32.0"}
    compat.write_json(rep_dir / "oracle.json", oracle_stub)
    static_doc = {
        "schema_version": 1,
        "baseline": "0.32.0",
        "cases": [
            {
                "id": "m5:synth:raw",
                "lane": "rust-api-and-cli",
                "exec_mode": "cli",
                "command": ["eval", "a.pkl"],
                "expected_exit": [0],
            },
            {
                "id": "m5:synth:ver",
                "lane": "rust-api-and-cli",
                "exec_mode": "cli",
                "command": ["--version"],
                "comparison_mode": "native-version",
                "expected_exit": [0],
            },
        ],
    }
    compat.write_json(rep_dir / "cases.snapshot.json", static_doc)
    compat.write_json(rep_dir / "dynamic-cases.snapshot.json", {"schema_version": 1, "cases": []})
    compat.write_json(rep_dir / "sources.snapshot.json", {"before": {"Cargo.toml": "aaa"}, "after": {"Cargo.toml": "aaa"}})

    results = []
    # Case 1: raw exact match including symlink file
    c1_dir = compat.case_directory("m5:synth:raw")
    c1_raw = _make_capture(b"hello\n", files={"generated/index.html": b"<html/>", "symlink/pkg/current": b"1.0.0"})
    j1 = compat.store_capture(rep_dir / "cases" / c1_dir / "java", c1_raw)
    r1 = compat.store_capture(rep_dir / "cases" / c1_dir / "rust", c1_raw)
    results.append({
        "id": "m5:synth:raw",
        "lane": "rust-api-and-cli",
        "exec_mode": "cli",
        "directory": c1_dir,
        "status": "pass",
        "differences": [],
        "expected_exit": [0],
        "comparison_mode": "raw",
        "java_assertion_failures": [],
        "rust_assertion_failures": [],
        "idempotency_ok": None,
        "java": {"command": ["java"], "result": j1},
        "rust": {"command": ["pkl"], "result": r1},
    })

    # Case 2: honest native-version contract
    c2_dir = compat.case_directory("m5:synth:ver")
    j2 = compat.store_capture(rep_dir / "cases" / c2_dir / "java", _make_capture(b"Pkl 0.32.0 (macOS 27.2, Java 21.0.13.1)\n"))
    r2 = compat.store_capture(rep_dir / "cases" / c2_dir / "rust", _make_capture(b"pkl-rs 0.32.0 (macOS 27.2, native)\n"))
    results.append({
        "id": "m5:synth:ver",
        "lane": "rust-api-and-cli",
        "exec_mode": "cli",
        "directory": c2_dir,
        "status": "pass",
        "differences": [],
        "expected_exit": [0],
        "comparison_mode": "native-version",
        "java_assertion_failures": [],
        "rust_assertion_failures": [],
        "idempotency_ok": None,
        "java": {"command": ["java", "--version"], "result": j2},
        "rust": {"command": ["pkl", "--version"], "result": r2},
    })

    report = {
        "schema_version": 1,
        "baseline": "0.32.0",
        "mode": "differential",
        "cases_manifest_sha256": m5_differential.digest((rep_dir / "cases.snapshot.json").read_bytes()),
        "dynamic_cases_sha256": m5_differential.digest((rep_dir / "dynamic-cases.snapshot.json").read_bytes()),
        "sources_snapshot_sha256": m5_differential.digest((rep_dir / "sources.snapshot.json").read_bytes()),
        "source_stability": True,
        "lanes": ["rust-api-and-cli"],
        "required_candidate_roles": ["pkl"],
        "oracle": oracle_stub,
        "candidates": {
            "pkl": {
                "name": "pkl",
                "sha256": m5_differential.digest(fake_bin.read_bytes()),
                "source_binding": source_binding,
                "build": {
                    "bin": "pkl",
                    "stdout_sha256": m5_differential.digest(b""),
                    "stderr_sha256": m5_differential.digest(b"Finished release\n"),
                },
            }
        },
        "selected_count": 2,
        "status_counts": {"pass": 2},
        "lane_status_counts": {"rust-api-and-cli": {"pass": 2}},
        "results": results,
    }
    report["evidence_sha256"] = {
        str(p.relative_to(rep_dir).as_posix()): m5_differential.digest(p.read_bytes())
        for p in sorted(m5_differential.evidence_paths(rep_dir))
    }
    compat.write_json(rep_dir / "report.json", report)


class M5DifferentialTests(unittest.TestCase):
    def test_native_version_contract_strict(self) -> None:
        j_pkl = _make_capture(b"Pkl 0.32.0 (macOS 27.2, Java 21.0.13.1)\n")
        r_ok = _make_capture(b"pkl-rs 0.32.0 (macOS 27.2, native)\n")
        self.assertEqual(m5_differential.check_native_version_contract(j_pkl, r_ok), [])

        # Rust MUST NOT fake Java version or Apple's upstream Pkl brand!
        r_faked = _make_capture(b"Pkl 0.32.0 (macOS 27.2, Java 21.0.13.1)\n")
        self.assertEqual(m5_differential.check_native_version_contract(j_pkl, r_faked), ["stdout"])
        r_apple = _make_capture(b"Pkl 0.32.0 (macOS 27.2, native)\n")
        self.assertEqual(m5_differential.check_native_version_contract(j_pkl, r_apple), ["stdout"])

        # pkldoc native-version contract works identically
        j_doc = _make_capture(b"pkldoc 0.32.0 (macOS 27.2, Java 21.0.13.1)\n")
        r_doc_ok = _make_capture(b"pkldoc-rs 0.32.0 (macOS 27.2, native)\n")
        self.assertEqual(m5_differential.check_native_version_contract(j_doc, r_doc_ok), [])

        # Trailing whitespace or non-empty stderr must fail
        r_ws = _make_capture(b"pkl-rs 0.32.0 (macOS 27.2, native) \n")
        self.assertEqual(m5_differential.check_native_version_contract(j_pkl, r_ws), ["stdout"])
        r_err = _make_capture(b"pkl-rs 0.32.0 (macOS 27.2, native)\n", stderr=b"warn\n")
        self.assertEqual(m5_differential.check_native_version_contract(j_pkl, r_err), ["stderr"])

    def test_classify_result_no_normalization_and_idempotency(self) -> None:
        case = {"id": "c1", "expected_exit": [0], "comparison_mode": "raw"}
        exp = _make_capture(b"a = 1\n", files={"symlink/x/current": b"1.0.0"})
        # Extra trailing space is a mismatch (never trimmed!)
        act_ws = _make_capture(b"a = 1 \n", files={"symlink/x/current": b"1.0.0"})
        st, diffs = m5_differential.classify_result(case, exp, act_ws)
        self.assertEqual(st, "mismatch")
        self.assertEqual(diffs, ["stdout"])

        # Symlink target mismatch is caught
        act_sym = _make_capture(b"a = 1\n", files={"symlink/x/current": b"0.9.0"})
        st2, diffs2 = m5_differential.classify_result(case, exp, act_sym)
        self.assertEqual(st2, "mismatch")
        self.assertEqual(diffs2, ["files/symlink/x/current"])

        # Failed format idempotency is caught even when pass-1 matches
        st3, diffs3 = m5_differential.classify_result(case, exp, exp, idempotency_ok=False)
        self.assertEqual(st3, "mismatch")
        self.assertIn("format-not-idempotent", diffs3)

    def test_verify_report_passes_and_detects_tampering(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            rep = Path(td) / "good"
            _build_synthetic_sealed_report(rep)
            m5_differential.verify_report(rep, strict=False)
            m5_differential.verify_report(rep, strict=True)

            # Tamper 1 byte in rust stdout.bin
            c1_dir = compat.case_directory("m5:synth:raw")
            stdout_path = rep / "cases" / c1_dir / "rust/stdout.bin"
            orig = stdout_path.read_bytes()
            stdout_path.write_bytes(b"hell0\n")
            with self.assertRaisesRegex(ValueError, "evidence file modified"):
                m5_differential.verify_report(rep, strict=False)
            stdout_path.write_bytes(orig)

            # Inject an extra unsealed file
            extra = rep / "rogue.txt"
            extra.write_text("hi", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "evidence file set differs"):
                m5_differential.verify_report(rep, strict=False)
            extra.unlink()

            # Unbound caller-supplied candidate passes integrity, fails strict!
            rep_unbound = Path(td) / "unbound"
            _build_synthetic_sealed_report(rep_unbound, source_binding="unbound-caller-supplied")
            m5_differential.verify_report(rep_unbound, strict=False)
            with self.assertRaisesRegex(ValueError, "recorded-build"):
                m5_differential.verify_report(rep_unbound, strict=True)

    def test_pkldoc_progress_contract_positive_and_negative(self) -> None:
        files = {
            "generated/index.html": b"<html/>",
            "symlink/com.pkg1/current": b"1.0.0",
        }
        j_out = (
            b"Loaded current search index\n"
            b"Loading package data for file:///out/b/current/package-data.json\r"
            b"Loading package data for file:///out/a/current/package-data.json\r"
            b"Fetched latest packages\n"
            b"Wrote file file:/out/b.html\r"
            b"Wrote file file:///out/data/Module%20Containing%20Spaces/index.json\r"
            b"Wrote file file:///out/data/Class%20Two%20%7B%7D.json\r"
            b"Generated HTML for packages\n"
        )
        # Reordered async progress segments pass while recording raw_stdout_bytes_match=False
        r_reordered = (
            b"Loaded current search index\n"
            b"Loading package data for file:///out/a/current/package-data.json\r"
            b"Loading package data for file:///out/b/current/package-data.json\r"
            b"Fetched latest packages\n"
            b"Wrote file file:///out/data/Class%20Two%20%7B%7D.json\r"
            b"Wrote file file:/out/b.html\r"
            b"Wrote file file:///out/data/Module%20Containing%20Spaces/index.json\r"
            b"Generated HTML for packages\n"
        )
        diffs, detail = m5_differential.check_pkldoc_progress_contract(
            _make_capture(j_out, files=files),
            _make_capture(r_reordered, files=files),
        )
        self.assertEqual(diffs, [])
        self.assertFalse(detail["raw_stdout_bytes_match"])
        self.assertTrue(detail["progress_order_differs"])
        self.assertEqual(detail["wrote_file_segments"], 3)
        self.assertEqual(detail["loading_package_data_segments"], 2)

        # Identical stdout records raw_stdout_bytes_match=True
        diffs_same, detail_same = m5_differential.check_pkldoc_progress_contract(
            _make_capture(j_out, files=files),
            _make_capture(j_out, files=files),
        )
        self.assertEqual(diffs_same, [])
        self.assertTrue(detail_same["raw_stdout_bytes_match"])
        self.assertFalse(detail_same["progress_order_differs"])

        # Negative 1: unescaped URI (%20 / %7B%7D missing) MUST FAIL!
        r_unescaped = r_reordered.replace(b"Module%20Containing%20Spaces", b"Module Containing Spaces")
        diffs_uri, detail_uri = m5_differential.check_pkldoc_progress_contract(
            _make_capture(j_out, files=files),
            _make_capture(r_unescaped, files=files),
        )
        self.assertEqual(diffs_uri, ["stdout"])
        self.assertTrue(any("wrote_file_multiset_mismatch" in r for r in detail_uri["failure_reasons"]))

        # Negative 2: altered milestone line (including trailing space, never trimmed) MUST FAIL!
        r_milestone = r_reordered.replace(b"Loaded current search index\n", b"Loaded current search index \n")
        diffs_ms, detail_ms = m5_differential.check_pkldoc_progress_contract(
            _make_capture(j_out, files=files),
            _make_capture(r_milestone, files=files),
        )
        self.assertEqual(diffs_ms, ["stdout"])
        self.assertTrue(any("milestone_mismatch" in r for r in detail_ms["failure_reasons"]))

        r_final_ms = r_reordered.replace(b"Generated HTML for packages\n", b"Generated HTML for package\n")
        diffs_fms, detail_fms = m5_differential.check_pkldoc_progress_contract(
            _make_capture(j_out, files=files),
            _make_capture(r_final_ms, files=files),
        )
        self.assertEqual(diffs_fms, ["stdout"])
        self.assertTrue(any("final_milestone_mismatch" in r for r in detail_fms["failure_reasons"]))

        # Negative 3: Migrated package multiset / count sequence checks
        j_mig = (
            b"Migrated 2 packages (com.b@2.0.0)\r"
            b"Migrated 3 packages (com.a@1.0.0)\r"
            b"Finished migration, migrated 3 packages.\n"
        )
        r_mig_ok = (
            b"Migrated 2 packages (com.a@1.0.0)\r"
            b"Migrated 3 packages (com.b@2.0.0)\r"
            b"Finished migration, migrated 3 packages.\n"
        )
        diffs_mok, detail_mok = m5_differential.check_pkldoc_progress_contract(
            _make_capture(j_mig), _make_capture(r_mig_ok)
        )
        self.assertEqual(diffs_mok, [])
        self.assertFalse(detail_mok["raw_stdout_bytes_match"])
        self.assertEqual(detail_mok["migrated_package_segments"], 2)

        # Wrong package set fails
        r_mig_bad_pkg = (
            b"Migrated 2 packages (com.a@1.0.0)\r"
            b"Migrated 3 packages (com.c@3.0.0)\r"
            b"Finished migration, migrated 3 packages.\n"
        )
        diffs_mbp, detail_mbp = m5_differential.check_pkldoc_progress_contract(
            _make_capture(j_mig), _make_capture(r_mig_bad_pkg)
        )
        self.assertEqual(diffs_mbp, ["stdout"])
        self.assertTrue(any("migrated_package_multiset_mismatch" in r for r in detail_mbp["failure_reasons"]))

        # Wrong count sequence fails
        r_mig_bad_cnt = (
            b"Migrated 1 packages (com.a@1.0.0)\r"
            b"Migrated 2 packages (com.b@2.0.0)\r"
            b"Finished migration, migrated 3 packages.\n"
        )
        diffs_mbc, detail_mbc = m5_differential.check_pkldoc_progress_contract(
            _make_capture(j_mig), _make_capture(r_mig_bad_cnt)
        )
        self.assertEqual(diffs_mbc, ["stdout"])
        self.assertTrue(any("migrated_count_sequence_mismatch" in r for r in detail_mbc["failure_reasons"]))

        # Negative 4: non-progress \r segments outside allowed families must match in exact order
        j_other = b"Custom step A\rCustom step B\rDone\n"
        r_other = b"Custom step B\rCustom step A\rDone\n"
        diffs_oth, detail_oth = m5_differential.check_pkldoc_progress_contract(
            _make_capture(j_other), _make_capture(r_other)
        )
        self.assertEqual(diffs_oth, ["stdout"])
        self.assertTrue(any("other_cr_order_mismatch" in r for r in detail_oth["failure_reasons"]))

        # Negative 5: file/symlink/stderr diffs still fail raw-byte check
        bad_files = dict(files, **{"symlink/com.pkg1/current": b"0.9.0"})
        diffs_file, _ = m5_differential.check_pkldoc_progress_contract(
            _make_capture(j_out, files=files),
            _make_capture(r_reordered, files=bad_files),
        )
        self.assertEqual(diffs_file, ["files/symlink/com.pkg1/current"])

        diffs_err, _ = m5_differential.check_pkldoc_progress_contract(
            _make_capture(j_out, files=files),
            _make_capture(r_reordered, stderr=b"warning\n", files=files),
        )
        self.assertEqual(diffs_err, ["stderr"])

    def test_verify_report_catches_tampered_pkldoc_progress_fields(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            rep = Path(td) / "pkldoc_rep"
            rep.mkdir(parents=True)
            artifacts = rep / "artifacts"
            artifacts.mkdir()
            fake_bin = artifacts / "pkldoc"
            fake_bin.write_bytes(b"FAKE-PKLDOC")
            (rep / "build-pkldoc.stdout.bin").write_bytes(b"")
            (rep / "build-pkldoc.stderr.bin").write_bytes(b"Finished release\n")
            oracle_stub = {"exact_jdk_lock_match": True, "version": "0.32.0"}
            compat.write_json(rep / "oracle.json", oracle_stub)
            static_doc = {
                "schema_version": 1,
                "baseline": "0.32.0",
                "cases": [
                    {
                        "id": "m5:pkldoc:synth-progress",
                        "lane": "pkldoc",
                        "exec_mode": "pkldoc",
                        "command": ["--output-dir", "{output_dir}"],
                        "comparison_mode": "pkldoc-progress",
                        "expected_exit": [0],
                    }
                ],
            }
            compat.write_json(rep / "cases.snapshot.json", static_doc)
            compat.write_json(rep / "dynamic-cases.snapshot.json", {"schema_version": 1, "cases": []})
            compat.write_json(rep / "sources.snapshot.json", {"before": {"Cargo.toml": "a"}, "after": {"Cargo.toml": "a"}})

            cdir = compat.case_directory("m5:pkldoc:synth-progress")
            j_cap = _make_capture(
                b"Wrote file file:///out/a.html\rWrote file file:///out/b.html\rGenerated HTML for packages\n",
                files={"generated/a.html": b"A", "generated/b.html": b"B"},
            )
            r_cap = _make_capture(
                b"Wrote file file:///out/b.html\rWrote file file:///out/a.html\rGenerated HTML for packages\n",
                files={"generated/a.html": b"A", "generated/b.html": b"B"},
            )
            j_stored = compat.store_capture(rep / "cases" / cdir / "java", j_cap)
            r_stored = compat.store_capture(rep / "cases" / cdir / "rust", r_cap)
            diffs, detail = m5_differential.check_pkldoc_progress_contract(j_cap, r_cap)
            self.assertEqual(diffs, [])

            report = {
                "schema_version": 1,
                "baseline": "0.32.0",
                "mode": "differential",
                "cases_manifest_sha256": m5_differential.digest((rep / "cases.snapshot.json").read_bytes()),
                "dynamic_cases_sha256": m5_differential.digest((rep / "dynamic-cases.snapshot.json").read_bytes()),
                "sources_snapshot_sha256": m5_differential.digest((rep / "sources.snapshot.json").read_bytes()),
                "source_stability": True,
                "lanes": ["pkldoc"],
                "required_candidate_roles": ["pkldoc"],
                "oracle": oracle_stub,
                "candidates": {
                    "pkldoc": {
                        "name": "pkldoc",
                        "sha256": m5_differential.digest(fake_bin.read_bytes()),
                        "source_binding": "recorded-build",
                        "build": {
                            "bin": "pkldoc",
                            "stdout_sha256": m5_differential.digest(b""),
                            "stderr_sha256": m5_differential.digest(b"Finished release\n"),
                        },
                    }
                },
                "selected_count": 1,
                "status_counts": {"pass": 1},
                "lane_status_counts": {"pkldoc": {"pass": 1}},
                "results": [
                    {
                        "id": "m5:pkldoc:synth-progress",
                        "lane": "pkldoc",
                        "exec_mode": "pkldoc",
                        "directory": cdir,
                        "status": "pass",
                        "differences": [],
                        "expected_exit": [0],
                        "comparison_mode": "pkldoc-progress",
                        "raw_stdout_bytes_match": False,
                        "pkldoc_progress_detail": detail,
                        "java_assertion_failures": [],
                        "rust_assertion_failures": [],
                        "idempotency_ok": None,
                        "java": {"command": ["java"], "result": j_stored},
                        "rust": {"command": ["pkldoc"], "result": r_stored},
                    }
                ],
            }
            report["evidence_sha256"] = {
                str(p.relative_to(rep).as_posix()): m5_differential.digest(p.read_bytes())
                for p in sorted(m5_differential.evidence_paths(rep))
            }
            compat.write_json(rep / "report.json", report)
            m5_differential.verify_report(rep, strict=True)

            # Tamper raw_stdout_bytes_match False -> True
            bad_report = json.loads((rep / "report.json").read_bytes())
            bad_report["results"][0]["raw_stdout_bytes_match"] = True
            compat.write_json(rep / "report.json", bad_report)
            with self.assertRaisesRegex(ValueError, "raw_stdout_bytes_match disagrees"):
                m5_differential.verify_report(rep, strict=False)

            # Tamper pkldoc_progress_detail
            bad_report["results"][0]["raw_stdout_bytes_match"] = False
            bad_report["results"][0]["pkldoc_progress_detail"]["wrote_file_segments"] = 99
            compat.write_json(rep / "report.json", bad_report)
            with self.assertRaisesRegex(ValueError, "pkldoc_progress_detail disagrees"):
                m5_differential.verify_report(rep, strict=False)


if __name__ == "__main__":
    unittest.main()
