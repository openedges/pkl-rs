import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("module_inheritance_fixtures", ROOT / "tools/module_inheritance_fixtures.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class InheritanceFixtureIntegrity(unittest.TestCase):
    def package(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        destination = Path(temporary.name) / "fixtures"
        shutil.copytree(module.DEFAULT, destination)
        return destination

    def update(self, root, name, change):
        path = root / name
        data = json.loads(path.read_bytes())
        change(data)
        path.write_text(json.dumps(data, indent=2) + "\n")
        manifest = json.loads((root / "manifest.json").read_bytes())
        manifest["files"][name] = hashlib.sha256(path.read_bytes()).hexdigest()
        (root / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")

    def test_original_actual_java_package(self):
        report = module.verify_package()
        self.assertEqual(report["fixtures"], 3)
        self.assertEqual(report["javaProcesses"], 6)
        self.assertEqual(report["expectedChannelBindings"], 120)

    def test_changed_source_rejected_after_refreshing_manifest(self):
        root = self.package()
        self.update(root, "cases.json", lambda data: data["cases"][0].update(source="result=7"))
        with self.assertRaisesRegex(ValueError, "source/API context"):
            module.verify_package(root)

    def test_changed_utf16_rejected_after_refreshing_manifest(self):
        root = self.package()
        def change(data):
            data["cases"][2]["expected"]["results"][0]["javaValueUnits"][0] = 56
        self.update(root, "cases.json", change)
        with self.assertRaisesRegex(ValueError, "Java result channels"):
            module.verify_package(root)

    def test_missing_or_reordered_call_and_callback_cannot_pass(self):
        for change in [
            lambda data: data["cases"][0]["expected"].update(results=[]),
            lambda data: data["cases"][2]["actions"].reverse(),
            lambda data: data["cases"][1]["sources"][0].update(callbackPaths=[]),
            lambda data: data["cases"][1]["expected"].update(callbacks=[]),
        ]:
            with self.subTest(change=change):
                root = self.package()
                self.update(root, "cases.json", change)
                with self.assertRaises(ValueError):
                    module.verify_package(root)

    def test_changed_uri_and_fake_identity_rejected(self):
        root = self.package()
        self.update(root, "cases.json", lambda data: data["cases"][1].update(uri="file:///rewritten.pkl"))
        with self.assertRaisesRegex(ValueError, "source/API context"):
            module.verify_package(root)

    def test_oracle_bug_classification_cannot_be_invented(self):
        root = self.package()
        def change(data):
            case = data["cases"][0]
            case["expected"]["results"][0]["javaErrorClass"] = "org.pkl.core.PklBugException"
            case["oracleError"] = True
        self.update(root, "cases.json", change)
        with self.assertRaisesRegex(ValueError, "Java result channels"):
            module.verify_package(root)

    def test_rehashed_origin_report_is_still_pinned(self):
        root = self.package()
        provenance = json.loads((root / "origins.json").read_bytes())
        key = next(iter(provenance["origins"]))
        name = provenance["origins"][key]["reportFile"]
        self.update(root, name, lambda data: data.update(scope="forged origin"))
        digest = hashlib.sha256((root / name).read_bytes()).hexdigest()
        self.update(root, "origins.json", lambda data: data["origins"][key].update(reportSha256=digest))
        with self.assertRaisesRegex(ValueError, "pinned oracle identity"):
            module.verify_package(root)


if __name__ == "__main__":
    unittest.main()
