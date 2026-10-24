"""Offline integrity and semantic boundary checks for prototype diagnostics."""
import copy
import json
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import prototype_diagnostics_fixtures as fixture
from collection_conversions_fixtures import ROW_CHANNELS, validate_capture
from prototype_diagnostics_oracle import SOURCES


class PrototypeDiagnosticFixtureTests(unittest.TestCase):
    def test_offline_sealed_capture(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = fixture.verify()
        self.assertEqual(result["rows"], 39)
        self.assertEqual(result["statuses"], {"success": 4, "error": 35})
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_empty_builtin_and_user_class_raw_diagnostics(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        positions = {row["id"]: index for index, row in enumerate(rows)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def read(identifier, channel):
                name = (f"{fixture.ARCHIVE_PREFIX}/repeat-0/"
                        f"{positions[identifier]:03d}-{identifier}/{channel}.bin")
                return archive.extractfile(name).read()
            for kind in ("dynamic", "listing", "mapping"):
                empty = read(f"pipe-{kind}-empty-class", "result.stderr")
                nonempty = read(f"pipe-{kind}-nonempty", "result.stderr")
                self.assertIn(f"and `{kind.title()}`".encode(), empty)
                self.assertIn(f"and `{kind.title()}`".encode(), nonempty)
                self.assertEqual(read(f"class-{kind}-empty", "result.stdout"), kind.title().encode())
            self.assertIn(b"prototype-diagnostics#C", read("pipe-user-lazy", "result.stderr"))
            self.assertIn(b"x = ?", read("pipe-user-lazy", "result.stderr"))
            self.assertIn(b"in class `Dynamic`", read("method-dynamic-empty", "result.stderr"))

    def test_duplicate_or_reordered_input_rejected(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        queries = (fixture.DEFAULT / "queries.tsv").read_bytes()
        duplicate = copy.deepcopy(rows)
        duplicate[1]["id"] = duplicate[0]["id"]
        with self.assertRaisesRegex(ValueError, "case rows differ"):
            fixture.validate_rows(duplicate, queries, count=39)
        with self.assertRaisesRegex(ValueError, "query order differs"):
            fixture.validate_rows(rows, b"string\tfake\t1\n", count=39)

    def test_missing_utf16_channel_rejected(self):
        prefix = f"{fixture.ARCHIVE_PREFIX}/repeat-0/000-pipe-dynamic-empty-class/"
        files = {prefix + name: b"" for name in ROW_CHANNELS}
        fixture.validate_row_channels(files, prefix)
        files.pop(prefix + "result.stderr.code-units.bin")
        with self.assertRaisesRegex(ValueError, "row channel set differs"):
            fixture.validate_row_channels(files, prefix)

    def test_raw_java_identity_mutation_rejected(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            files = {member.name: archive.extractfile(member).read() for member in archive}
        name = fixture.ARCHIVE_PREFIX + "/java-identity.stderr.bin"
        files[name] = files[name].replace(b"21.0.12.1", b"21.0.12.2", 1)
        with self.assertRaisesRegex(ValueError, "raw Java identity differs"):
            validate_capture(files, fixture.ARCHIVE_PREFIX, mode="string", rows_count=39,
                             expected_statuses={"success": 4, "error": 35},
                             expected_raw_files=821, module_uri=fixture.MODULE_URI,
                             source_names=SOURCES)

    def test_extra_path_and_rewritten_manifest_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            (root / "extra").mkdir()
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular"):
                fixture.verify(root)
            (root / "extra").rmdir()
            (root / "module.pkl").write_bytes((root / "module.pkl").read_bytes() + b"other = 1\n")
            files = {path.name: fixture.sha(path.read_bytes()) for path in root.iterdir()
                     if path.name != "manifest.json"}
            (root / "manifest.json").write_text(json.dumps({"schema": 1,
                "baseline": "Pkl 0.32.0", "files": files}))
            with self.assertRaisesRegex(ValueError, "authored input differs"):
                fixture.verify(root)


if __name__ == "__main__":
    unittest.main()
