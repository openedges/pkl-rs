"""Authenticate the complete Java policy calls, event order and counters."""
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
import module_admission_policy_fixtures as tool

FIXTURE = Path(__file__).resolve().parents[2] / "tests/fixtures/m2-module-admission-policy"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class ModuleAdmissionPolicyFixtureTests(unittest.TestCase):
    def setUp(self):
        self.before = {name: digest(FIXTURE / name) for name in ["manifest.json", "fixtures.json"]}
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.clone = Path(self.temporary.name) / "fixture"
        shutil.copytree(FIXTURE, self.clone, copy_function=os.link)

    def tearDown(self):
        self.assertEqual(self.before, {name: digest(FIXTURE / name) for name in self.before})

    def rewrite(self, relative, data):
        path = self.clone / relative
        path.unlink()
        path.write_bytes(data)
        manifest = self.clone / "manifest.json"
        value = json.loads(manifest.read_bytes())
        value["files"][relative] = {"sha256": digest(path), "bytes": len(data)}
        manifest.unlink()
        manifest.write_text(json.dumps(value, indent=2) + "\n")

    def test_complete_api_and_event_contract_retains_encoding_limits(self):
        result = tool.verify(self.clone)
        self.assertEqual(result["fixtures"], 12)
        self.assertEqual(result["javaApiCalls"], 84)
        self.assertEqual(result["javaOrderedEvents"], 280)
        self.assertEqual(result["statusCounts"], {"success": 80, "error": 4})
        self.assertFalse(result["eventUtf16IndependentCapture"])

    def test_reordered_events_cannot_be_reblessed(self):
        data = json.loads((self.clone / "fixtures.json").read_bytes())
        events = data["fixtures"][0]["orderedEvents"]
        events[0], events[1] = events[1], events[0]
        self.rewrite("fixtures.json", (json.dumps(data) + "\n").encode())
        with self.assertRaisesRegex(ValueError, "fixture differs"):
            tool.verify(self.clone)

    def test_changed_raw_counters_cannot_be_reblessed(self):
        path = next(self.clone.glob("provenance/*/cases/*/0/calls/session-0/outer-0.counters-after.bin"))
        data = path.read_bytes()
        changed = data.replace(b"resolve=3", b"resolve=2")
        self.assertNotEqual(data, changed)
        self.rewrite(str(path.relative_to(self.clone)), changed)
        with self.assertRaisesRegex(ValueError, "pinnedinventory payload"):
            tool.verify(self.clone)

    def test_missing_primary_event_cannot_be_reblessed(self):
        data = json.loads((self.clone / "fixtures.json").read_bytes())
        data["fixtures"][0]["orderedEvents"].pop(0)
        self.rewrite("fixtures.json", (json.dumps(data) + "\n").encode())
        with self.assertRaisesRegex(ValueError, "fixture differs"):
            tool.verify(self.clone)

    def test_verification_reads_only_installed_fixture(self):
        original = Path.read_bytes

        def confined(path):
            self.assertTrue(path.is_relative_to(self.clone))
            return original(path)

        with mock.patch.object(Path, "read_bytes", confined):
            tool.verify(self.clone)

    def test_unknown_directory_is_rejected_before_descent(self):
        unknown = self.clone / "unlisted-cache"
        unknown.mkdir()
        visited = []
        original = os.scandir

        def observed(path):
            visited.append(Path(path))
            return original(path)

        with mock.patch("os.scandir", side_effect=observed):
            with self.assertRaises(ValueError):
                tool.verify(self.clone)
        self.assertNotIn(unknown, visited)

    def test_provenance_symlink_is_rejected_before_read(self):
        link = self.clone / tool.PREFIX / "report.json"
        link.unlink()
        link.symlink_to(FIXTURE / tool.PREFIX / "report.json")
        original = Path.read_bytes

        def guarded(path):
            self.assertNotEqual(path, link)
            return original(path)

        with mock.patch.object(Path, "read_bytes", guarded):
            with self.assertRaises(ValueError):
                tool.verify(self.clone)

    def test_forged_source_mapping_is_rejected(self):
        manifest = self.clone / "manifest.json"
        data = json.loads(manifest.read_bytes())
        next(iter(data["sourceProvenance"].values()))["originalRoot"] = "/forged/oracle"
        manifest.unlink()
        manifest.write_text(json.dumps(data) + "\n")
        with self.assertRaisesRegex(ValueError, "sourceprovenance mapping"):
            tool.verify(self.clone)


if __name__ == "__main__":
    unittest.main()
