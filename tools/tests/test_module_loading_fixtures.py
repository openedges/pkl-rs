"""Synthetic integrity tampering; these checks are not Java compatibility cases."""
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import module_loading_fixtures as module


class ModuleFixtureIntegrityTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name) / "fixtures"
        shutil.copytree(module.DEFAULT, self.root)

    def update(self, filename, value):
        (self.root / filename).write_bytes(module.encoded(value))
        manifest = module.load(self.root / "manifest.json")
        manifest["files"][filename] = module.digest((self.root / filename).read_bytes())
        (self.root / "manifest.json").write_bytes(module.encoded(manifest))

    def test_complete_package_is_integrity_only(self):
        result = module.verify_package(self.root)
        self.assertEqual(result["fixtures"], 22)
        self.assertEqual(result["originalCliCases"], 3)
        self.assertIn("no evaluator execution", result["scope"])

    def test_changed_expected_and_manifest_cannot_rewrite_java(self):
        cases = module.load(self.root / "cases.json")
        cases["cases"][0]["expected"]["results"][0]["bytesHex"] = "3f"
        self.update("cases.json", cases)
        with self.assertRaisesRegex(ValueError, "expected CLI bytes differ"):
            module.verify_package(self.root)

    def test_self_consistent_payload_metadata_still_binds_closed_inventory(self):
        origins = module.load(self.root / "origins.json")
        binding = next(row for row in origins["bindings"] if row["path"].endswith("/0/stdout.bin"))
        (self.root / binding["path"]).write_bytes(b"forged")
        binding["sha256"] = module.digest(b"forged")
        manifest = module.load(self.root / "manifest.json")
        manifest["files"][binding["path"]] = binding["sha256"]
        (self.root / "manifest.json").write_bytes(module.encoded(manifest))
        self.update("origins.json", origins)
        with self.assertRaisesRegex(ValueError, "original closed inventory"):
            module.verify_package(self.root)

    def test_missing_original_is_not_a_smaller_passing_set(self):
        cases = module.load(self.root / "cases.json")
        cases["cases"] = [row for row in cases["cases"] if row["id"] != "original-import3"]
        self.update("cases.json", cases)
        manifest = module.load(self.root / "manifest.json")
        manifest["fixtureCount"] = 21
        (self.root / "manifest.json").write_bytes(module.encoded(manifest))
        with self.assertRaisesRegex(ValueError, "original import1/2/3"):
            module.verify_package(self.root)

    def test_self_consistent_identity_table_still_binds_java_raw_metadata(self):
        origins = module.load(self.root / "origins.json")
        path = origins["identityMetadata"]["path"]
        identity = module.load(self.root / path)
        identity["original-import1:main.pkl:primary"]["resolvedUri"] = "file:///invented.pkl"
        self.update(path, identity)
        origins["identityMetadata"]["sha256"] = module.digest((self.root / path).read_bytes())
        self.update("origins.json", origins)
        with self.assertRaisesRegex(ValueError, "raw Java observation"):
            module.verify_package(self.root)

    def test_callback_or_log_cannot_be_dropped_from_expected_coverage(self):
        for field in ["callbacks", "logs"]:
            with self.subTest(field=field):
                cases = module.load(self.root / "cases.json")
                case = next(row for row in cases["cases"] if row["id"] == "init-empty-publication")
                original = case["expected"][field]
                case["expected"][field] = []
                self.update("cases.json", cases)
                with self.assertRaisesRegex(ValueError, "missing Java"):
                    module.verify_package(self.root)
                case["expected"][field] = original
                self.update("cases.json", cases)

    def test_null_log_member_is_not_an_empty_or_invented_string(self):
        cases = module.load(self.root / "cases.json")
        case = next(row for row in cases["cases"] if row["id"] == "init-empty-publication")
        self.assertIsNone(case["expected"]["logs"][0]["memberName"])
        case["expected"]["logs"][0]["memberName"] = []
        self.update("cases.json", cases)
        with self.assertRaisesRegex(ValueError, "log member"):
            module.verify_package(self.root)


if __name__ == "__main__":
    unittest.main()
