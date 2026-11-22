#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Unit, determinism, shrinking, coverage-gate, and tamper-detection tests for tools/m7_fuzz.py."""
from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

import compat
import m7_fuzz


def _cap(stdout: bytes, stderr: bytes = b"", exit_code: int = 0, timed_out: bool = False) -> dict:
    return {
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": exit_code,
        "files": {},
        "timed_out": timed_out,
        "workspace_changes": {},
        "elapsed_ns": 100,
    }


def _build_synthetic_fuzz_report(
    rep_dir: Path,
    *,
    count: int = 49,
    exact_jdk_lock_match: bool = True,
    source_stability: bool = True,
    inject_mismatch: bool = False,
    fail_format_name: str | None = None,
    decode_probe_binding: str = "recorded-build",
) -> None:
    rep_dir.mkdir(parents=True)
    art = rep_dir / "artifacts"
    art.mkdir()
    for name in ("pkl", m7_fuzz.DECODE_PROBE_EXAMPLE):
        (art / name).write_bytes(f"FAKE-{name}".encode())
        (rep_dir / f"build-{name}.stdout.bin").write_bytes(b"")
        (rep_dir / f"build-{name}.stderr.bin").write_bytes(b"ok\n")
    oracle_stub = {"exact_jdk_lock_match": exact_jdk_lock_match, "version": "0.32.0"}
    compat.write_json(rep_dir / "oracle.json", oracle_stub)
    compat.write_json(
        rep_dir / "sources.snapshot.json",
        {"before": {"Cargo.toml": "aaa"}, "after": {"Cargo.toml": "aaa" if source_stability else "bbb"}},
    )

    cases = m7_fuzz.generate_corpus(seed=0x1234, count=count)
    results = []
    for idx, case in enumerate(cases):
        cdir = compat.case_directory(case["id"])
        folder = rep_dir / "cases" / cdir
        folder.mkdir(parents=True)
        (folder / "input.pkl").write_bytes(case["source"].encode("utf-8"))
        rc = 11 if case["mode"] == "format" else 0
        if fail_format_name is not None and case["category"] == "multi-format-rendering" and case["format"] == fail_format_name:
            rc = 1
        j_cap = _cap(b"" if rc == 1 else b"x = 1\n", stderr=b"err\n" if rc == 1 else b"", exit_code=rc)
        r_cap = _cap(
            b"x = 2\n" if (inject_mismatch and idx == 0) else (b"" if rc == 1 else b"x = 1\n"),
            stderr=b"err\n" if rc == 1 else b"",
            exit_code=rc,
        )
        p2_cap = _cap(b"x = 1\n", exit_code=0) if case["mode"] == "format" else None
        dec_meta = None
        if case["mode"] == "eval" and case["format"] == "pkl-binary" and rc == 0:
            dec_meta = {"ok": True, "class_name": "input", "member_count": 1, "decoded_sha256": "0" * 64, "error": None}
            compat.write_json(folder / "decode_roundtrip.json", dec_meta)

        j_st = compat.store_capture(folder / "java", j_cap)
        r_st = compat.store_capture(folder / "candidate", r_cap)
        p2_st = compat.store_capture(folder / "candidate_pass2", p2_cap) if p2_cap is not None else None
        status, diffs = m7_fuzz.classify_fuzz_case(case, j_cap, r_cap, pass2_cap=p2_cap, decode_meta=dec_meta)
        results.append({
            "id": case["id"],
            "category": case["category"],
            "mode": case["mode"],
            "format": case["format"],
            "seed": case["seed"],
            "directory": cdir,
            "source_sha256": case["source_sha256"],
            "status": status,
            "differences": diffs,
            "java": j_st,
            "candidate": r_st,
            "candidate_pass2": p2_st,
            "decode_roundtrip": dec_meta,
            "minimized": None,
        })

    from collections import Counter
    sc = dict(sorted(Counter(r["status"] for r in results).items()))
    csc = {
        cat: dict(sorted(Counter(r["status"] for r in results if r["category"] == cat).items()))
        for cat in m7_fuzz.ALL_CATEGORIES
    }
    report = {
        "schema_version": 1,
        "suite": "m7-seeded-differential-fuzz",
        "baseline": "0.32.0",
        "prng_algorithm": "splitmix64",
        "seed": "0x0000000000001234",
        "requested_count": count,
        "total_count": len(results),
        "categories": list(m7_fuzz.ALL_CATEGORIES),
        "host": {"platform": "test", "system": "Darwin", "machine": "arm64", "python": "3.14"},
        "oracle": oracle_stub,
        "candidate": {
            "name": "pkl",
            "sha256": m7_fuzz.digest((art / "pkl").read_bytes()),
            "source_binding": "recorded-build",
            "build": {
                "bin": "pkl",
                "stdout_sha256": m7_fuzz.digest(b""),
                "stderr_sha256": m7_fuzz.digest(b"ok\n"),
            },
        },
        "decode_probe": {
            "name": m7_fuzz.DECODE_PROBE_EXAMPLE,
            "sha256": m7_fuzz.digest((art / m7_fuzz.DECODE_PROBE_EXAMPLE).read_bytes()),
            "source_binding": decode_probe_binding,
            "build": {
                "bin": m7_fuzz.DECODE_PROBE_EXAMPLE,
                "stdout_sha256": m7_fuzz.digest(b""),
                "stderr_sha256": m7_fuzz.digest(b"ok\n"),
            },
        },
        "sources_snapshot_sha256": m7_fuzz.digest((rep_dir / "sources.snapshot.json").read_bytes()),
        "sources_file_count": 1,
        "source_stability": source_stability,
        "pass_count": sc.get("pass", 0),
        "mismatch_count": sc.get("mismatch", 0),
        "error_count": 0,
        "status_counts": sc,
        "category_status_counts": csc,
        "minimized_regressions": [],
        "results": results,
    }
    report["evidence_sha256"] = {
        str(p.relative_to(rep_dir).as_posix()): m7_fuzz.digest(p.read_bytes())
        for p in sorted(m7_fuzz.evidence_paths(rep_dir))
    }
    compat.write_json(rep_dir / "report.json", report)


class M7FuzzTests(unittest.TestCase):
    def test_seeded_corpus_is_deterministic_and_covers_all_7_categories_and_formats(self) -> None:
        c1 = m7_fuzz.generate_corpus(seed=0xABCD1234, count=70)
        c2 = m7_fuzz.generate_corpus(seed=0xABCD1234, count=70)
        c3 = m7_fuzz.generate_corpus(seed=0xDEADBEEF, count=70)
        self.assertEqual(c1, c2)
        self.assertNotEqual([x["source"] for x in c1], [x["source"] for x in c3])
        self.assertEqual({x["category"] for x in c1}, set(m7_fuzz.ALL_CATEGORIES))
        render_fmts = {x["format"] for x in c1 if x["category"] == "multi-format-rendering"}
        self.assertEqual(render_fmts, set(m7_fuzz.RENDER_FORMATS))

    def test_raw_byte_classification_never_normalizes_whitespace_or_idempotency(self) -> None:
        eval_case = {"mode": "eval", "format": "pcf"}
        st, diffs = m7_fuzz.classify_fuzz_case(eval_case, _cap(b"a = 1\n"), _cap(b"a = 1 \n"))
        self.assertEqual(st, "mismatch")
        self.assertEqual(diffs, ["stdout"])

        st2, diffs2 = m7_fuzz.classify_fuzz_case(eval_case, _cap(b"a = 1\n"), _cap(b"a = 1"))
        self.assertEqual(st2, "mismatch")
        self.assertEqual(diffs2, ["stdout"])

        # Formatter 2nd-pass idempotency failure must never pass!
        fmt_case = {"mode": "format", "format": "format"}
        st3, diffs3 = m7_fuzz.classify_fuzz_case(
            fmt_case,
            _cap(b"a = 1\n", exit_code=11),
            _cap(b"a = 1\n", exit_code=11),
            pass2_cap=_cap(b"a  =  1\n", exit_code=11),
        )
        self.assertEqual(st3, "format-not-idempotent")
        self.assertIn("format-pass2-stdout-differs", diffs3)

        # pkl-binary decode round-trip failure must never pass!
        bin_case = {"mode": "eval", "format": "pkl-binary"}
        st4, diffs4 = m7_fuzz.classify_fuzz_case(
            bin_case,
            _cap(b"\x94\x01"),
            _cap(b"\x94\x01"),
            decode_meta={"ok": False, "error": "truncated"},
        )
        self.assertEqual(st4, "pkl-binary-decode-error")
        self.assertEqual(diffs4, ["decode:truncated"])

    def test_delta_shrinker_minimizes_failing_program(self) -> None:
        src = (
            "// irrelevant header\n"
            "a = 1\n"
            "b = 2\n"
            "c = 3\n"
            "bug = trigger_me(42)\n"
            "d = 4\n"
            "e = 5\n"
            "f = 6\n"
        )
        minimized, steps = m7_fuzz.shrink_source(src, lambda s: "trigger_me" in s, max_steps=40)
        self.assertIn("trigger_me", minimized)
        self.assertEqual(minimized.strip(), "bug = trigger_me(42)")
        self.assertGreater(steps, 0)
        self.assertLessEqual(steps, 40)

    def test_verify_report_strict_f2_coverage_and_tamper_detection(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            ok_dir = Path(tmp) / "ok"
            _build_synthetic_fuzz_report(ok_dir, count=49)
            m7_fuzz.verify_report(ok_dir, strict=False)
            m7_fuzz.verify_report(ok_dir, strict=True)

            # Tamper stdout of one case -> SHA-256 sealing rejects
            c0_dir = compat.case_directory("fuzz-0000-expr")
            (ok_dir / "cases" / c0_dir / "candidate/stdout.bin").write_bytes(b"TAMPERED\n")
            with self.assertRaisesRegex(ValueError, "evidence file modified"):
                m7_fuzz.verify_report(ok_dir, strict=False)

            # F2: only 7 cases (misses 6 of 7 RENDER_FORMATS!) -> verify_strict MUST reject
            tiny_dir = Path(tmp) / "tiny"
            _build_synthetic_fuzz_report(tiny_dir, count=7)
            m7_fuzz.verify_report(tiny_dir, strict=False)
            with self.assertRaisesRegex(ValueError, "exit-0 passing render for format"):
                m7_fuzz.verify_report(tiny_dir, strict=True)

            # F2: if `properties` format always exits 1 (even if Java==Rust), strict gate rejects!
            no_prop_dir = Path(tmp) / "no_prop"
            _build_synthetic_fuzz_report(no_prop_dir, count=49, fail_format_name="properties")
            m7_fuzz.verify_report(no_prop_dir, strict=False)
            with self.assertRaisesRegex(ValueError, "exit-0 passing render for format: properties"):
                m7_fuzz.verify_report(no_prop_dir, strict=True)

            # F2: if `pkl-binary` never exits 0 (no decode round-trip), strict gate rejects!
            no_bin_dir = Path(tmp) / "no_bin"
            _build_synthetic_fuzz_report(no_bin_dir, count=49, fail_format_name="pkl-binary")
            m7_fuzz.verify_report(no_bin_dir, strict=False)
            with self.assertRaisesRegex(ValueError, "exit-0 passing render for format: pkl-binary"):
                m7_fuzz.verify_report(no_bin_dir, strict=True)

            # F2: non-recorded-build decode_probe rejected by strict gate
            bad_probe_dir = Path(tmp) / "bad_probe"
            _build_synthetic_fuzz_report(bad_probe_dir, count=49, decode_probe_binding="workspace-bin")
            m7_fuzz.verify_report(bad_probe_dir, strict=False)
            with self.assertRaisesRegex(ValueError, "recorded-build decode_probe"):
                m7_fuzz.verify_report(bad_probe_dir, strict=True)

            # Honest mismatch: verify_report passes, verify_strict rejects!
            mm_dir = Path(tmp) / "mismatch"
            _build_synthetic_fuzz_report(mm_dir, count=49, inject_mismatch=True)
            rep = m7_fuzz.verify_report(mm_dir, strict=False)
            self.assertEqual(rep["mismatch_count"], 1)
            with self.assertRaisesRegex(ValueError, "strict fuzz gate failed"):
                m7_fuzz.verify_report(mm_dir, strict=True)

            # Local JDK mismatch rejected by strict gate
            jdk_dir = Path(tmp) / "jdk_bad"
            _build_synthetic_fuzz_report(jdk_dir, count=49, exact_jdk_lock_match=False)
            m7_fuzz.verify_report(jdk_dir, strict=False)
            with self.assertRaisesRegex(ValueError, "exact_jdk_lock_match"):
                m7_fuzz.verify_report(jdk_dir, strict=True)


if __name__ == "__main__":
    unittest.main()
