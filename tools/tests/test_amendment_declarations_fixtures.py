"""Separate Stage64 declaration-module authority and integrity regressions."""
import json
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import amendment_declarations_fixtures as fixture
from collection_conversions_fixtures import ROW_CHANNELS, validate_row_channels


class AmendmentDeclarationsFixtureTests(unittest.TestCase):
    def test_offline_sealed_capture(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = fixture.verify()
        self.assertEqual(result["programs"], 11)
        self.assertEqual(result["statuses"], {"success": 3, "error": 8})
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_distinct_module_first_errors_and_successes(self):
        programs = json.loads((fixture.DEFAULT / "programs.json").read_bytes())["programs"]
        names = {program["id"]: index for index, program in enumerate(programs)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def read(id, channel):
                index = names[id]
                path = (f"{fixture.ARCHIVE_PREFIX}/programs/{index:02d}-{id}/"
                        f"repeat-0/000-{id}/{channel}.bin")
                return archive.extractfile(path).read()
            self.assertIn(b"Missing modifier `fixed`.",
                          read("fixed-class-property-amend", "result.stderr"))
            self.assertIn(b"Missing modifier `const`.",
                          read("const-class-property-amend", "result.stderr"))
            self.assertIn(b"Missing `}` delimiter.",
                          read("later-syntax-versus-fixed", "result.stderr"))
            self.assertIn(b"Properties with type annotations cannot have object bodies.",
                          read("typed-annotation-amend", "result.stderr"))
            self.assertEqual(read("abstract-class-property-amend", "result.status"), b"success\n")
            self.assertEqual(read("local-class-property-amend", "result.status"), b"success\n")
            self.assertEqual(read("valid-repeated-class-bodies", "result.status"), b"success\n")

    def test_exact_pinned_module_bytes_and_order(self):
        programs = json.loads((fixture.DEFAULT / "programs.json").read_bytes())["programs"]
        self.assertEqual(len({program["id"] for program in programs}), 11)
        for index, program in enumerate(programs):
            self.assertEqual(program["module_file"], f"modules/{index:02d}-{program['id']}.pkl")
            self.assertEqual(fixture.sha((fixture.DEFAULT / program["module_file"]).read_bytes()),
                             program["module_sha256"])
            self.assertEqual(program["query"], "test")

    def test_missing_utf16_or_exception_channel_rejected(self):
        folder = f"{fixture.ARCHIVE_PREFIX}/programs/00-abstract-class-property-amend/repeat-0/000-abstract-class-property-amend/"
        files = {folder + name: b"" for name in ROW_CHANNELS}
        validate_row_channels(files, folder)
        files.pop(folder + "result.cause-class.code-units.bin")
        with self.assertRaisesRegex(ValueError, "row channel set differs"):
            validate_row_channels(files, folder)

    def test_undeclared_directory_or_nested_manifest_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            (root / "extra").mkdir()
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular"):
                fixture.verify(root)
            (root / "extra" / "manifest.json").write_text("{}")
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular"):
                fixture.verify(root)

    def test_mutated_module_with_rewritten_root_manifest_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            module = root / "modules/00-abstract-class-property-amend.pkl"
            module.write_bytes(module.read_bytes() + b"other = 1\n")
            hashes = {p.relative_to(root).as_posix(): fixture.sha(p.read_bytes())
                      for p in root.rglob("*") if p.is_file() and p.name != "manifest.json"}
            (root / "manifest.json").write_text(json.dumps(
                {"schema": 1, "baseline": "Pkl 0.32.0", "files": hashes}))
            with self.assertRaisesRegex(ValueError, "authored module bytes/order differ"):
                fixture.verify(root)


if __name__ == "__main__":
    unittest.main()
