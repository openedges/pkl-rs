"""Authenticate frame-identity expectations against immutable Java captures."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import renderer_identity_fixtures as tool

FIXTURE = Path(__file__).resolve().parents[2] / "tests/fixtures/m2-renderer-frame-identity"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class RendererIdentityFixtureTests(unittest.TestCase):
    def setUp(self):
        self.golden_before = {
            name: digest(FIXTURE / name)
            for name in ["manifest.json", "fixtures.json", "cases.tsv", "expected.tsv"]
        }
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.clone = Path(self.temporary.name) / "fixture"
        shutil.copytree(FIXTURE, self.clone, copy_function=os.link)

    def tearDown(self):
        self.assertEqual(
            self.golden_before,
            {name: digest(FIXTURE / name) for name in self.golden_before},
        )

    def rewrite(self, relative, data):
        path = self.clone / relative
        path.unlink()
        path.write_bytes(data)
        manifest = self.clone / "manifest.json"
        value = json.loads(manifest.read_bytes())
        value["files"][relative] = {"sha256": digest(path), "bytes": len(data)}
        manifest.unlink()
        manifest.write_text(json.dumps(value, indent=2) + "\n")

    def test_three_programs_twelve_sessions_and_240_complete_rows(self):
        result = tool.verify(self.clone)
        self.assertEqual(result["javaApiCalls"], 240)
        self.assertEqual(result["fixtures"], 12)
        self.assertEqual(len((self.clone / "cases.tsv").read_text().splitlines()), 3)
        rows = (self.clone / "expected.tsv").read_text().splitlines()
        self.assertEqual(len(rows), 240)
        self.assertTrue(all(len(row.split("\t")) == 12 for row in rows))

    def test_recomputed_manifest_cannot_authorize_changed_expectations(self):
        original = (self.clone / "expected.tsv").read_bytes()
        self.rewrite("expected.tsv", original.replace(b"\tsuccess\t", b"\terror\t", 1))
        with self.assertRaisesRegex(ValueError, "TSV differs"):
            tool.verify(self.clone)

    def test_recomputed_manifest_cannot_authorize_changed_reader_counters(self):
        path = next(self.clone.glob("provenance/*/cases/*/0/calls/session-0/outer-0.reads-after.bin"))
        self.rewrite(str(path.relative_to(self.clone)), b"1")
        with self.assertRaisesRegex(ValueError, "inventory payload mismatch"):
            tool.verify(self.clone)

    def test_verification_reads_only_the_copied_fixture(self):
        original_read = Path.read_bytes

        def confined_read(path):
            self.assertTrue(path.is_relative_to(self.clone))
            return original_read(path)

        with mock.patch.object(Path, "read_bytes", confined_read):
            tool.verify(self.clone)

    def test_unknown_directory_is_rejected_before_descent(self):
        unknown = self.clone / "unlisted-cache"
        unknown.mkdir()
        visited = []
        original_scan = os.scandir

        def observed_scan(path):
            visited.append(Path(path))
            return original_scan(path)

        with mock.patch("os.scandir", side_effect=observed_scan):
            with self.assertRaises(ValueError):
                tool.verify(self.clone)
        self.assertNotIn(unknown, visited)

    def test_provenance_symlink_is_rejected_before_read(self):
        path = self.clone / "provenance/stage47-renderer-api-oracle-10/report.json"
        path.unlink()
        path.symlink_to(FIXTURE / path.relative_to(self.clone))
        with self.assertRaisesRegex(ValueError, "regular file"):
            tool.verify(self.clone)

    def test_source_uri_cannot_be_rewritten_with_a_new_manifest_hash(self):
        original = (self.clone / "cases.tsv").read_bytes()
        changed = original.replace(b"file:", b"other:", 1)
        self.assertNotEqual(original, changed)
        self.rewrite("cases.tsv", changed)
        with self.assertRaisesRegex(ValueError, "TSV differs"):
            tool.verify(self.clone)


if __name__ == "__main__":
    unittest.main()
