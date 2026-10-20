"""Selected historical fixture provenance and resource-closure regressions."""
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import regression_manifest as regressions


class SelectedRegressionTests(unittest.TestCase):
    def copied(self, directory):
        root = Path(directory)
        shutil.copytree(ROOT / regressions.PREFIX, root / regressions.PREFIX)
        (root / "compatibility").mkdir()
        shutil.copyfile(ROOT / "compatibility/rev1-inventory.json", root / "compatibility/rev1-inventory.json")
        return root

    def test_pinned_objects_and_licenses_validate_without_sibling_or_git(self):
        with patch.object(regressions.subprocess, "check_output", side_effect=AssertionError("normal checks must not invoke Git")):
            manifest = regressions.build()
        committed = json.loads((ROOT / "compatibility/regressions.json").read_bytes())
        self.assertEqual(committed, manifest)
        self.assertEqual(manifest["summary"]["unchanged_pkl_inputs"], 8)
        self.assertEqual(manifest["summary"]["command_cases"], 3)
        self.assertEqual(len(manifest["licenses"]), 2)
        for case in manifest["cases"]:
            self.assertIn(case["expected_outcome"], ("success", "failure"))
            self.assertEqual(case["expected_exit"], [case["oracle_observation"]["exit_code"]])
            self.assertEqual(case["expected_outcome"], "success" if case["oracle_observation"]["exit_code"] == 0 else "failure")
            self.assertIn(case["historical_assertion"]["expected_outcome"], ("success", "error"))
            self.assertEqual(case["source"]["sha256"], case["original"]["sha256"])

    def test_changed_pkl_license_or_project_settings_fail_closed(self):
        for name in ("class_authority/t40/repro.pkl", "LICENSE", "diagnostics/t36/app/PklProject.deps.json"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = self.copied(directory)
                path = root / regressions.PREFIX / name
                path.write_bytes(path.read_bytes() + b" ")
                with self.assertRaises(ValueError):
                    regressions.build(root)

    def test_unknown_files_and_wrong_historical_revision_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.copied(directory)
            (root / regressions.PREFIX / "old-result.stderr").write_bytes(b"not an oracle result")
            with self.assertRaisesRegex(ValueError, "uninventoried"):
                regressions.build(root)
        with tempfile.TemporaryDirectory() as directory:
            root = self.copied(directory)
            path = root / "compatibility/rev1-inventory.json"
            value = json.loads(path.read_bytes())
            value["revision"] = "0" * 40
            path.write_text(json.dumps(value))
            with self.assertRaisesRegex(ValueError, "revision"):
                regressions.build(root)

    def test_local_project_dependency_and_imported_class_closures_are_explicit(self):
        manifest = regressions.build()
        t36, positive, negative = manifest["cases"]
        paths = {resource["path"] for resource in t36["resources"]}
        self.assertEqual(len(paths), 5)
        self.assertIn(regressions.PREFIX + "diagnostics/t36/dep/form.pkl", paths)
        self.assertIn(regressions.PREFIX + "diagnostics/t36/app/PklProject.deps.json", paths)
        lock = json.loads(regressions.DEPS)
        self.assertEqual(next(iter(lock["resolvedDependencies"].values()))["path"], "../dep")
        self.assertEqual(len(positive["resources"]), 2)
        self.assertEqual(len(negative["resources"]), 3)
        self.assertEqual({case["id"] for case in manifest["cases"]}, {
            "rev1:t36-foreign-nominal-identity", "rev1:t40-imported-declaration-authority", "rev1:t40-foreign-namesake"})


if __name__ == "__main__":
    unittest.main()
