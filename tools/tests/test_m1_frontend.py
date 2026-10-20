"""Failure detection for the bounded M1 differential evidence contract."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m1_frontend


class FrontendEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.case = self.root / "one"
        self.case.mkdir()
        for name, data in {"java.stdout.bin": b"ok\n", "java.stderr.bin": b"",
                           "rust.result.bin": b"ok\n", "rust.stdout.bin": b"libtest progress\n",
                           "rust.stderr.bin": b""}.items():
            (self.case / name).write_bytes(data)
        (self.root / "sources").mkdir()
        (self.root / "sources/source.rs").write_bytes(b"source")
        self.contract = [{"id": "one", "input_sha256": "fixture"}]
        self.report = {"source_stability": True, "contract": self.contract,
                       "source_sha256": {"source.rs": m1_frontend.digest(b"source")},
                       "results": [{"id": "one", "java_exit_code": 0, "rust_exit_code": 0, "passed": True}]}
        self.refresh_inventory()
        self.addCleanup(patch.stopall)
        patch.object(m1_frontend, "CASES", [("one", "parse", "fixture")]).start()
        patch.object(m1_frontend, "contract", return_value=self.contract).start()

    def refresh_inventory(self):
        self.report["evidence_sha256"] = {path.relative_to(self.root).as_posix(): m1_frontend.oracle.sha256(path)
                                         for path in self.root.rglob("*")
                                         if path.is_file() and path.name != "report.json"}
        self.save()

    def save(self):
        (self.root / "report.json").write_text(json.dumps(self.report))

    def test_exact_complete_evidence_passes(self):
        m1_frontend.check_report(self.root)

    def test_missing_duplicate_and_unknown_cases_fail(self):
        for results in [[], self.report["results"] * 2, [{"id": "unexpected"}]]:
            with self.subTest(results=results):
                original = copy.deepcopy(self.report)
                self.report["results"] = results
                self.save()
                with self.assertRaises(ValueError):
                    m1_frontend.check_report(self.root)
                self.report = original

    def test_mutated_capture_fails(self):
        (self.case / "java.stdout.bin").write_bytes(b"changed")
        with self.assertRaises(ValueError):
            m1_frontend.check_report(self.root)

    def test_empty_captures_cannot_pass_even_with_matching_hashes(self):
        (self.case / "java.stdout.bin").write_bytes(b"")
        (self.case / "rust.result.bin").write_bytes(b"")
        self.refresh_inventory()
        with self.assertRaises(ValueError):
            m1_frontend.check_report(self.root)

    def test_missing_inventory_and_unrecorded_file_fail(self):
        self.report["evidence_sha256"] = {}
        self.save()
        with self.assertRaises(ValueError):
            m1_frontend.check_report(self.root)
        self.refresh_inventory()
        (self.root / "extra.bin").write_bytes(b"unrecorded")
        with self.assertRaises(ValueError):
            m1_frontend.check_report(self.root)

    def test_nonzero_exit_and_byte_difference_fail(self):
        self.report["results"][0]["rust_exit_code"] = 1
        self.save()
        with self.assertRaises(ValueError):
            m1_frontend.check_report(self.root)
        self.report["results"][0]["rust_exit_code"] = 0
        (self.case / "rust.result.bin").write_bytes(b"ok")
        self.refresh_inventory()
        with self.assertRaises(ValueError):
            m1_frontend.check_report(self.root)

    def test_source_drift_or_missing_provenance_fail(self):
        self.report["source_stability"] = False
        self.save()
        with self.assertRaises(ValueError):
            m1_frontend.check_report(self.root)
        self.report["source_stability"] = True
        self.report["source_sha256"] = {}
        self.save()
        with self.assertRaises(ValueError):
            m1_frontend.check_report(self.root)
