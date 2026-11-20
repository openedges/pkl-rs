"""Unit, negative, and tamper-detection tests for tools/m3_differential.py."""
from __future__ import annotations

from collections import Counter
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compat
import m3_differential
import oracle

ROOT = m3_differential.ROOT


def make_capture(
    stdout: bytes = b"ok\n",
    stderr: bytes = b"",
    exit_code: int = 0,
    files: dict[str, bytes] | None = None,
    timed_out: bool = False,
) -> dict:
    return {
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": exit_code,
        "files": {} if files is None else dict(files),
        "timed_out": timed_out,
        "workspace_changes": {},
        "elapsed_ns": 1000,
    }


def write_sealed_report(
    directory: Path,
    cases: list[dict],
    captures: dict[str, tuple[dict, dict | None]],
    *,
    exact_jdk_lock_match: bool = True,
    source_binding: str = "recorded-build",
) -> dict:
    directory.mkdir(parents=True, exist_ok=True)
    oracle_meta = {
        "schema": 1,
        "version": "0.32.0",
        "exact_jdk_lock_match": exact_jdk_lock_match,
        "jdk": {"exact_jdk_lock_match": exact_jdk_lock_match, "properties": {"java.version": "21.0.13.1"}},
    }
    cases_doc = {"schema_version": 1, "baseline": "0.32.0", "cases": cases}
    compat.write_json(directory / "oracle.json", oracle_meta)
    cases_bytes = compat.encoded(cases_doc)
    (directory / "cases.snapshot.json").write_bytes(cases_bytes)

    lanes = list(dict.fromkeys(c["lane"] for c in cases))
    results = []
    for case in cases:
        cdir = compat.case_directory(case["id"])
        folder = directory / "cases" / cdir
        java_raw, rust_raw = captures[case["id"]]
        java_stored = compat.store_capture(folder / "java", java_raw)
        rust_stored = compat.store_capture(folder / "rust", rust_raw) if rust_raw is not None else None
        status, diffs = m3_differential.classify_result(case, java_raw, rust_raw)
        results.append(
            {
                "id": case["id"],
                "lane": case["lane"],
                "directory": cdir,
                "status": status,
                "differences": diffs,
                "expected_exit": case["expected_exit"],
                "java": {"command": ["java"], "result": java_stored},
                "rust": None if rust_stored is None else {"command": ["rust"], "result": rust_stored},
            }
        )

    by_status = dict(sorted(Counter(r["status"] for r in results).items()))
    by_lane = {
        lane: dict(sorted(Counter(r["status"] for r in results if r["lane"] == lane).items()))
        for lane in lanes
    }
    report = {
        "schema_version": 1,
        "baseline": "0.32.0",
        "cases_manifest_sha256": compat.digest(cases_bytes),
        "lanes": lanes,
        "workers": 1,
        "timeout_seconds": 30.0,
        "oracle": oracle_meta,
        "candidates": {
            "module_eval_probe": {"name": "module_eval_probe", "source_binding": source_binding},
            "m3_pklbinary_decode_probe": None,
        },
        "selected_count": len(cases),
        "status_counts": by_status,
        "lane_status_counts": by_lane,
        "results": results,
    }
    report["evidence_sha256"] = {
        str(p.relative_to(directory).as_posix()): oracle.sha256(p)
        for p in sorted(m3_differential.evidence_paths(directory))
    }
    compat.write_json(directory / "report.json", report)
    return report


class M3DifferentialTests(unittest.TestCase):
    def test_apply_setup_and_collect_generated_files_symlinks_and_escapes(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            sandbox = Path(td) / "sandbox"
            gen = sandbox / "generated"
            gen.mkdir(parents=True)
            setup = {
                "dirs": ["out", "outside_target", "real_out"],
                "symlinks": [
                    ["out/outside", "../outside_target"],
                    ["link_out", "real_out"],
                ],
            }
            declared = m3_differential.apply_setup(gen, setup)
            self.assertEqual(declared, {"out/outside", "link_out"})
            (gen / "real_out/nested").mkdir(parents=True)
            (gen / "real_out/nested/bin.dat").write_bytes(b"\x00\x01\xff\r\n")

            # Declared symlinks are skipped cleanly; regular raw bytes are captured
            collected = m3_differential.collect_generated_files(gen, declared, sandbox_parent=sandbox)
            self.assertEqual(collected, {"generated/real_out/nested/bin.dat": b"\x00\x01\xff\r\n"})

            # An escaped write outside `generated` inside `sandbox` is captured as `escaped/...`
            (sandbox / "foo.txt").write_bytes(b"escaped!")
            collected_esc = m3_differential.collect_generated_files(gen, declared, sandbox_parent=sandbox)
            self.assertEqual(
                collected_esc,
                {
                    "escaped/foo.txt": b"escaped!",
                    "generated/real_out/nested/bin.dat": b"\x00\x01\xff\r\n",
                },
            )
            (sandbox / "foo.txt").unlink()

            # Any undeclared symlink inside `generated` fails closed
            (gen / "rogue_link").symlink_to("real_out")
            with self.assertRaisesRegex(ValueError, "Unexpected output symlink"):
                m3_differential.collect_generated_files(gen, declared, sandbox_parent=sandbox)

    def test_classify_result_raw_bytes_and_statuses(self) -> None:
        cli_case = {"id": "m3:test:cli", "lane": "output-files", "expected_exit": [0]}
        dec_case = {"id": "m3:test:dec", "lane": "pklbinary-decode", "expected_exit": [0]}

        exp = make_capture(stdout=b"a\n", files={"generated/f.txt": b"\x00\x01"})
        self.assertEqual(m3_differential.classify_result(cli_case, exp, make_capture(stdout=b"a\n", files={"generated/f.txt": b"\x00\x01"})), ("pass", []))

        # No trimming / whitespace normalization
        self.assertEqual(
            m3_differential.classify_result(cli_case, exp, make_capture(stdout=b"a\r\n", files={"generated/f.txt": b"\x00\x01"})),
            ("mismatch", ["stdout"]),
        )
        self.assertEqual(
            m3_differential.classify_result(cli_case, exp, make_capture(stdout=b"a\n", stderr=b" ", files={"generated/f.txt": b"\x00\x01"})),
            ("mismatch", ["stderr"]),
        )
        # Missing, extra, or 1-byte file diffs
        self.assertEqual(
            m3_differential.classify_result(cli_case, exp, make_capture(stdout=b"a\n", files={"generated/f.txt": b"\x00\x02"})),
            ("mismatch", ["files/generated/f.txt"]),
        )
        self.assertEqual(
            m3_differential.classify_result(cli_case, exp, make_capture(stdout=b"a\n", files={})),
            ("mismatch", ["files/generated/f.txt"]),
        )
        self.assertEqual(
            m3_differential.classify_result(cli_case, exp, make_capture(stdout=b"a\n", files={"generated/f.txt": b"\x00\x01", "escaped/x": b"1"})),
            ("mismatch", ["files/escaped/x"]),
        )
        # Candidate exit codes 2/3/4 on CLI lanes vs pklbinary-decode lane
        self.assertEqual(
            m3_differential.classify_result(cli_case, exp, make_capture(exit_code=2)),
            ("candidate-unsupported", ["candidate_exit_2"]),
        )
        self.assertEqual(
            m3_differential.classify_result(cli_case, exp, make_capture(exit_code=3)),
            ("candidate-limit-exceeded", ["candidate_exit_3"]),
        )
        self.assertEqual(
            m3_differential.classify_result(cli_case, exp, make_capture(exit_code=4)),
            ("candidate-internal-error", ["candidate_exit_4"]),
        )
        self.assertEqual(
            m3_differential.classify_result(dec_case, make_capture(), make_capture(exit_code=2)),
            ("mismatch", ["exit_code"]),
        )
        # Missing candidate, timeout, and oracle failures never pass
        self.assertEqual(
            m3_differential.classify_result(cli_case, exp, None),
            ("candidate-unavailable", ["candidate_unavailable"]),
        )
        self.assertEqual(
            m3_differential.classify_result(cli_case, make_capture(timed_out=True), exp),
            ("oracle-error", ["oracle_timed_out"]),
        )
        self.assertEqual(
            m3_differential.classify_result(cli_case, make_capture(exit_code=1), exp),
            ("oracle-contract-mismatch", ["oracle_exit_1"]),
        )
        self.assertEqual(
            m3_differential.classify_result(cli_case, exp, make_capture(timed_out=True)),
            ("candidate-timeout", ["candidate_timed_out"]),
        )

    def test_verify_report_strict_vs_integrity_and_tamper_detection(self) -> None:
        cases = [
            {"id": "m3:c1", "lane": "formats-renderers", "expected_exit": [0]},
            {"id": "m3:c2", "lane": "output-files", "expected_exit": [0]},
        ]
        passing = {
            "m3:c1": (make_capture(stdout=b"x = 1\n"), make_capture(stdout=b"x = 1\n")),
            "m3:c2": (
                make_capture(stdout=b"out.txt\n", files={"generated/out.txt": b"hi"}),
                make_capture(stdout=b"out.txt\n", files={"generated/out.txt": b"hi"}),
            ),
        }

        with tempfile.TemporaryDirectory() as td:
            root = Path(td)

            # 1. Clean passing report passes both integrity & strict
            d_pass = root / "pass"
            write_sealed_report(d_pass, cases, passing)
            m3_differential.verify_report(d_pass, strict=False)
            m3_differential.verify_report(d_pass, strict=True)

            # 2. Recorded mismatch passes integrity (preserves failure) and FAILS strict
            d_mis = root / "mismatch"
            mis_caps = dict(passing)
            mis_caps["m3:c2"] = (
                make_capture(stdout=b"out.txt\n", files={"generated/out.txt": b"hi"}),
                make_capture(stdout=b"out.txt\n", files={"generated/out.txt": b"HI"}),
            )
            write_sealed_report(d_mis, cases, mis_caps)
            rep = m3_differential.verify_report(d_mis, strict=False)
            self.assertEqual(rep["status_counts"], {"mismatch": 1, "pass": 1})
            with self.assertRaisesRegex(ValueError, "strict gate failed"):
                m3_differential.verify_report(d_mis, strict=True)

            # 3. Missing candidate (`candidate-unavailable`) passes integrity and FAILS strict
            d_unavail = root / "unavail"
            unavail_caps = dict(passing)
            unavail_caps["m3:c2"] = (make_capture(), None)
            write_sealed_report(d_unavail, cases, unavail_caps)
            m3_differential.verify_report(d_unavail, strict=False)
            with self.assertRaisesRegex(ValueError, "strict gate failed"):
                m3_differential.verify_report(d_unavail, strict=True)

            # 4. Non-exact JDK lock or unbound caller candidate FAILS strict
            d_jdk = root / "local-jdk"
            write_sealed_report(d_jdk, cases, passing, exact_jdk_lock_match=False)
            m3_differential.verify_report(d_jdk, strict=False)
            with self.assertRaisesRegex(ValueError, "exact_jdk_lock_match"):
                m3_differential.verify_report(d_jdk, strict=True)

            d_unbound = root / "unbound"
            write_sealed_report(d_unbound, cases, passing, source_binding="unbound-caller-supplied")
            m3_differential.verify_report(d_unbound, strict=False)
            with self.assertRaisesRegex(ValueError, "recorded-build"):
                m3_differential.verify_report(d_unbound, strict=True)

            # 5. Tamper stdout.bin -> fails integrity
            c1_dir = compat.case_directory("m3:c1")
            (d_pass / "cases" / c1_dir / "rust" / "stdout.bin").write_bytes(b"x = 2\n")
            with self.assertRaisesRegex(ValueError, "evidence file modified"):
                m3_differential.verify_report(d_pass, strict=False)

            # 6. Extra unrecorded file -> fails integrity
            d_extra = root / "extra"
            write_sealed_report(d_extra, cases, passing)
            (d_extra / "rogue.txt").write_bytes(b"sneaky")
            with self.assertRaisesRegex(ValueError, "evidence file set differs"):
                m3_differential.verify_report(d_extra, strict=False)

            # 7. Relabeled status in report.json -> fails integrity
            d_relabel = root / "relabel"
            write_sealed_report(d_relabel, cases, mis_caps)
            rep_path = d_relabel / "report.json"
            rep_json = json.loads(rep_path.read_bytes())
            rep_json["results"][1]["status"] = "pass"
            rep_json["results"][1]["differences"] = []
            rep_json["status_counts"] = {"pass": 2}
            rep_json["lane_status_counts"]["output-files"] = {"pass": 1}
            compat.write_json(rep_path, rep_json)
            with self.assertRaisesRegex(ValueError, "recomputed status/differences disagree"):
                m3_differential.verify_report(d_relabel, strict=False)

    def test_run_single_case_reuses_identical_output_path_and_catches_escape(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            td_path = Path(td)
            # Tiny executable helper that simulates `eval -m <dir>` writing `a.txt` and echoing `<dir>/a.txt`
            good_script = td_path / "good_eval.py"
            good_script.write_text(
                f"#!{sys.executable}\n"
                "import sys\n"
                "from pathlib import Path\n"
                "idx = sys.argv.index('-m')\n"
                "out = Path(sys.argv[idx + 1])\n"
                "f = out / 'a.txt'\n"
                "f.write_bytes(b'hello\\n')\n"
                "sys.stdout.write(str(f) + '\\n')\n",
                encoding="utf-8",
            )
            good_script.chmod(good_script.stat().st_mode | stat.S_IXUSR)

            bad_escape_script = td_path / "escape_eval.py"
            bad_escape_script.write_text(
                f"#!{sys.executable}\n"
                "import sys\n"
                "from pathlib import Path\n"
                "idx = sys.argv.index('-m')\n"
                "out = Path(sys.argv[idx + 1])\n"
                "(out / 'a.txt').write_bytes(b'hello\\n')\n"
                "(out.parent / 'escaped.txt').write_bytes(b'oops')\n"
                "sys.stdout.write(str(out / 'a.txt') + '\\n')\n",
                encoding="utf-8",
            )
            bad_escape_script.chmod(bad_escape_script.stat().st_mode | stat.S_IXUSR)

            case = {
                "id": "m3:output-files:synthetic-same-path",
                "lane": "output-files",
                "command": ["eval", "-m", "{output_dir}", "dummy.pkl"],
                "environment": {},
                "expected_exit": [0],
            }
            oracle_meta = {
                "command": [str(good_script)],
                "environment": {"PATH": os.environ.get("PATH", os.defpath)},
                "jdk": {"properties": {"java.version": "21.0.13.1"}},
            }

            # Identical path re-staging lets raw stdout path match byte-for-byte with zero normalization
            out_ok = td_path / "run_ok"
            row_ok = m3_differential.run_single_case(
                td_path, case, out_ok, oracle_meta, None, good_script, None, 10.0
            )
            self.assertEqual(row_ok["status"], "pass", row_ok)
            self.assertEqual(row_ok["differences"], [])

            # Candidate writing `../escaped.txt` fails differential check via `files/escaped/escaped.txt`
            out_esc = td_path / "run_esc"
            row_esc = m3_differential.run_single_case(
                td_path, case, out_esc, oracle_meta, None, bad_escape_script, None, 10.0
            )
            self.assertEqual(row_esc["status"], "mismatch")
            self.assertEqual(row_esc["differences"], ["files/escaped/escaped.txt"])

    def test_java_pklbinary_decode_probe_live_smoke(self) -> None:
        pinned_java = ROOT / ".cache/oracle/temurin-21.0.13.1+2-LTS/jdk-21.0.13.1+2/Contents/Home/bin/java"
        jar = ROOT / ".cache/oracle/pkl-tools-0.32.0.jar"
        if not pinned_java.is_file() or not jar.is_file():
            self.skipTest("pinned oracle JDK or pkl-tools-0.32.0.jar not present")
        javac = pinned_java.with_name("javac")
        with tempfile.TemporaryDirectory() as td:
            classes = Path(td)
            comp = subprocess.run(
                [str(javac), "-encoding", "UTF-8", "-cp", str(jar), "-d", str(classes), str(ROOT / m3_differential.JAVA_DECODE_PROBE)],
                capture_output=True,
                text=True,
                timeout=60,
            )
            self.assertEqual(comp.returncode, 0, comp.stderr)

            cp = f"{jar}{os.pathsep}{classes}"
            # Valid upstream msgpack fixture
            upstream = ROOT / "external/pkl/pkl-core/src/test/resources/org/pkl/core/pklBinaryDecoderTest.msgpack"
            ok = subprocess.run(
                [str(pinned_java), "-cp", cp, "M3PklBinaryDecodeProbe", str(upstream)],
                capture_output=True,
                timeout=30,
            )
            self.assertEqual(ok.returncode, 0, ok.stderr.decode("utf-8", "replace"))
            decoded = json.loads(ok.stdout.decode("utf-8"))
            self.assertEqual(decoded["type"], "object")
            self.assertEqual(decoded["class"]["simpleName"], "Foo")

            # Synthetic valid PModule fixture
            mod_vec = ROOT / "tests/fixtures/m3/pklbinary_decode/valid_primitives_and_containers.msgpack"
            mod_ok = subprocess.run(
                [str(pinned_java), "-cp", cp, "M3PklBinaryDecodeProbe", str(mod_vec)],
                capture_output=True,
                timeout=30,
            )
            self.assertEqual(mod_ok.returncode, 0, mod_ok.stderr.decode("utf-8", "replace"))
            self.assertEqual(json.loads(mod_ok.stdout.decode("utf-8"))["type"], "module")

            # Unsupported 0x20 REFERENCE code vector
            ref_vec = ROOT / "tests/fixtures/m3/pklbinary_decode/err_reference_code_0x20.msgpack"
            err = subprocess.run(
                [str(pinned_java), "-cp", cp, "M3PklBinaryDecodeProbe", str(ref_vec)],
                capture_output=True,
                timeout=30,
            )
            self.assertEqual(err.returncode, 1)
            self.assertIn(b"REFERENCE", err.stderr)


if __name__ == "__main__":
    unittest.main()
