"""Adversarial tests for the sealed collection predicate fixtures."""
from __future__ import annotations

import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent if HERE.name == "tests" else HERE))
from collection_predicate_fixtures import ROOT, default_fixture, verify, verify_emptiness, verify_every
from collection_emptiness_fixtures import verify as verify_emptiness_profile
from collection_every_fixtures import verify as verify_every_profile


def archived(kind):
    with tarfile.open(default_fixture(kind) / "oracle-evidence.tar.gz", "r:gz") as stream:
        return {entry.name: stream.extractfile(entry).read() for entry in stream}


class CollectionPredicateFixtureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.emptiness = archived("collection-emptiness")
        cls.every = archived("collection-every")

    def test_offline_full_readback_both(self):
        self.assertEqual(verify("collection-emptiness")["statuses"],
                         {"success": 34, "error": 13})
        self.assertEqual(verify("collection-every")["statuses"],
                         {"success": 36, "error": 28})

    def test_catalog_shaped_verifier_entrypoints(self):
        self.assertEqual(verify_emptiness_profile()["rows"], 47)
        self.assertEqual(verify_every_profile()["rows"], 64)

    def test_root_extra_file_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            target = Path(temp)
            for source in default_fixture("collection-every").iterdir():
                shutil.copy2(source, target / source.name)
            (target / "undeclared.bin").write_bytes(b"x")
            with self.assertRaisesRegex(ValueError, "root file set"):
                verify("collection-every", target)

    def test_emptiness_nonpass_cannot_be_promoted(self):
        changed = self.emptiness.copy()
        path = "capture-02/repeat-0/004-list-method/result.status.bin"
        self.assertEqual(changed[path], b"error\n")
        changed[path] = b"success\n"
        with self.assertRaises(ValueError):
            verify_emptiness(changed)

    def test_every_query_order_bound(self):
        changed = self.every.copy()
        lines = changed["capture-03/queries.tsv"].splitlines(keepends=True)
        lines[0], lines[1] = lines[1], lines[0]
        changed["capture-03/queries.tsv"] = b"".join(lines)
        with self.assertRaisesRegex(ValueError, "ordered queries"):
            verify_every(changed)

    def test_every_missing_logger_utf16_rejected(self):
        changed = self.every.copy()
        key = "capture-03/repeat-0/002-list-order-all/log-000.message.code-units.bin"
        self.assertIn(key, changed)
        del changed[key]
        with self.assertRaisesRegex(ValueError, "row/logger channel"):
            verify_every(changed)

    def test_every_logger_order_is_byte_exact(self):
        changed = self.every.copy()
        key = "capture-03/repeat-0/002-list-order-all/log-000.message.bin"
        self.assertIn(key, changed)
        changed[key] = changed[key].replace(b"3", b"1")
        with self.assertRaisesRegex(ValueError, "UTF-8/UTF-16"):
            verify_every(changed)

    def test_every_complete_logger_event_swap_rejected(self):
        changed = self.every.copy()
        folder = "capture-03/repeat-0/002-list-order-all/"
        for suffix in (".bin", ".code-units.bin"):
            left = folder + "log-000.message" + suffix
            right = folder + "log-001.message" + suffix
            changed[left], changed[right] = changed[right], changed[left]
        with self.assertRaisesRegex(ValueError, "raw repetitions"):
            verify_every(changed)

    def test_every_zero_logger_count_is_bound(self):
        changed = self.every.copy()
        key = "capture-03/repeat-0/000-list-empty/logs.count.bin"
        self.assertEqual(changed[key], b"0\n")
        changed[key] = b"1\n"
        with self.assertRaisesRegex(ValueError, "row/logger channel"):
            verify_every(changed)

    def test_every_error_status_cannot_be_promoted(self):
        changed = self.every.copy()
        key = "capture-03/repeat-0/056-listing-empty-arity-two/result.status.bin"
        self.assertEqual(changed[key], b"error\n")
        changed[key] = b"success\n"
        with self.assertRaises(ValueError):
            verify_every(changed)

    def test_every_source_copy_bound(self):
        changed = self.every.copy()
        key = "capture-03/sources/external/pkl/stdlib/base.pkl"
        changed[key] += b"\n"
        with self.assertRaisesRegex(ValueError, "source closure"):
            verify_every(changed)

    def test_installed_tools_layout_uses_tracked_fixture_roots(self):
        proposed = HERE.parent if HERE.name == "tests" else HERE
        with tempfile.TemporaryDirectory() as temp:
            installed = Path(temp)
            (installed / "tools").mkdir()
            (installed / "tests/fixtures").mkdir(parents=True)
            shutil.copy2(ROOT / "oracle.lock.json", installed / "oracle.lock.json")
            for name in ("collection_predicate_fixtures.py", "collection_emptiness_fixtures.py",
                         "collection_every_fixtures.py"):
                shutil.copy2(proposed / name, installed / "tools" / name)
            for name in ("collection_conversion_controls_fixtures.py",
                         "collection_conversion_controls_oracle.py"):
                shutil.copy2(ROOT / "tools" / name, installed / "tools" / name)
            for kind in ("collection-emptiness", "collection-every"):
                shutil.copytree(default_fixture(kind), installed / "tests/fixtures" / f"m2-{kind}-oracle")
                script = installed / "tools" / f"{kind.replace('-', '_')}_fixtures.py"
                run = subprocess.run([sys.executable, "-B", str(script)], cwd=installed,
                                     capture_output=True, timeout=30)
                self.assertEqual(run.returncode, 0, run.stderr.decode(errors="replace"))
                self.assertEqual(json.loads(run.stdout)["rows"], 47 if kind == "collection-emptiness" else 64)
            self.assertFalse((installed / "target").exists())


if __name__ == "__main__":
    unittest.main()
