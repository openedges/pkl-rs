"""Corpus coverage and stale-inventory regression tests."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("discover", Path(__file__).resolve().parents[1] / "discover.py")
discover = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(discover)


class DiscoveryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = discover.build_manifest()

    def test_every_tracked_pkl_and_project_is_accounted_for(self):
        for repository in self.manifest["sources"]:
            expected = {p for p in discover.tracked(discover.ROOT / repository["path"])
                        if p.endswith(".pkl") or Path(p).name == "PklProject"}
            actual = {f["source"]["path"] for f in self.manifest["files"]
                      if f["source"]["repository"] == repository["repository"]}
            self.assertEqual(expected, actual)
        self.assertGreater(self.manifest["summary"]["pkl_files"], 0)

    def test_cases_resolve_to_pinned_inputs_and_explicit_execution_contracts(self):
        identifiers = [c["id"] for c in self.manifest["cases"]]
        self.assertEqual(len(identifiers), len(set(identifiers)))
        inventory = {(f["source"]["repository"], f["source"]["path"]): f for f in self.manifest["files"]}
        for case in self.manifest["cases"]:
            record = inventory[case["source"]["repository"], case["source"]["path"]]
            self.assertEqual(record["source"], case["source"])
            self.assertIn(case["id"], record["case_ids"])
            if case["execution"]["status"] == "ready":
                self.assertTrue(case["command"])
            else:
                self.assertTrue(case["execution"]["reason"])
            self.assertEqual(set(case["channels"]), {"stdout", "stderr", "exit_code", "files"})
        self.assertIsNone(self.manifest["summary"]["assertions"])
        self.assertEqual(self.manifest["summary"]["compatibility_passes"], 0)

    def test_resource_census_hashes_all_tracked_files(self):
        for source in self.manifest["sources"]:
            self.assertEqual(source["resource_set_sha256"], discover.digest(discover.encoded(source["resources"])))
            hashes = {r["path"]: r["sha256"] for r in source["resources"]}
            for case in self.manifest["cases"]:
                if case["source"]["repository"] == source["repository"]:
                    self.assertEqual(hashes[source["path"] + "/" + case["source"]["path"]], case["source"]["sha256"])
                    for resource in case["resources"]:
                        self.assertEqual(hashes[resource["path"]], resource["sha256"])

    def test_symlink_hash_preserves_literal_trailing_slash(self):
        with tempfile.TemporaryDirectory() as temporary:
            link = Path(temporary) / "link"
            link.symlink_to("child/")
            self.assertEqual(discover.tracked_bytes(link), b"child/")
            self.assertNotEqual(discover.digest(discover.tracked_bytes(link)), discover.digest(b"child"))

    def test_hidden_renderer_extension_and_error_golden_selection(self):
        base = discover.SNIPPETS
        paths = {base + "output/x/sample.json", base + "output/x/bad.err", base + "output/x/日本語.pcf"}
        self.assertEqual(discover.golden_paths(base + "input/x/sample.json.pkl", paths), [base + "output/x/sample.json"])
        self.assertEqual(discover.golden_paths(base + "input/x/bad.pkl", paths), [base + "output/x/bad.err"])
        self.assertEqual(discover.golden_paths(base + "input/x/日本語.pkl", paths), [base + "output/x/日本語.pcf"])

    def test_local_dependency_closure_handles_cycles_and_rejects_escape(self):
        dependencies = {"packages/a": {"resolvedDependencies": {"b": {"type": "local", "path": "../b"}}},
                        "packages/b": {"resolvedDependencies": {"a": {"type": "local", "path": "../a"}}}}
        self.assertEqual(discover.local_project_closure("packages/a", dependencies), ["packages/a", "packages/b"])
        dependencies["packages/b"]["resolvedDependencies"]["a"]["path"] = "../../escape"
        with self.assertRaises(ValueError):
            discover.local_project_closure("packages/a", dependencies)

    def test_check_rejects_absence_and_one_byte_drift_without_rewriting(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "corpus.json"
            value = {"summary": {"cases": 1}}
            with patch.object(discover, "MANIFEST", path), patch.object(discover, "build_manifest", return_value=value), \
                    patch.object(sys, "argv", ["discover.py", "--check"]), contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(discover.main(), 1)
                path.write_bytes(discover.encoded(value))
                self.assertEqual(discover.main(), 0)
                changed = path.read_bytes() + b" "
                path.write_bytes(changed)
                self.assertEqual(discover.main(), 1)
                self.assertEqual(path.read_bytes(), changed)

    def test_optional_history_does_not_require_sibling_checkout(self):
        original = discover.git
        def guarded(repository, *arguments):
            self.assertIn(Path(repository), [discover.ROOT / source["path"] for source in self.manifest["sources"]])
            return original(repository, *arguments)
        with patch.object(discover, "git", side_effect=guarded):
            manifest = discover.build_manifest()
        self.assertEqual(manifest, self.manifest)
        for source in manifest["optional_sources"]:
            snapshot = discover.ROOT / source["snapshot"]
            self.assertEqual(discover.digest(snapshot.read_bytes()), source["sha256"])
            self.assertEqual(source["execution"]["status"], "blocked")


if __name__ == "__main__":
    unittest.main()
