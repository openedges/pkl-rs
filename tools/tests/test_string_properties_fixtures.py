"""Adversarial integrity checks for the pinned Stage67 String property authority."""
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
import string_properties_fixtures as fixture
from collection_conversions_fixtures import ROW_CHANNELS, validate_capture
from string_properties_oracle import SOURCES


class StringPropertiesFixtureTests(unittest.TestCase):
    def test_offline_sealed_authority(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verification started Java")):
            result = fixture.verify()
        self.assertEqual(result["rows"], 146)
        self.assertEqual(result["statuses"], {"success": 117, "error": 29})
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_code_point_and_reference_raw_observations(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        index = {row["id"]: i for i, row in enumerate(rows)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def read(id, name):
                path = f"{fixture.ARCHIVE_PREFIX}/repeat-0/{index[id]:03d}-{id}/{name}.bin"
                return archive.extractfile(path).read()
            self.assertEqual(read("lastindex-public-empty", "result.stdout"), b"-1")
            self.assertEqual(read("lastindex-public-emoji", "result.stdout"), b"0")
            self.assertEqual(read("lastindex-public-mixed", "result.stdout"), b"2")
            self.assertEqual(read("lastindex-public-reverse-pair", "result.stdout"), b"1")
            self.assertEqual(read("length-codepoints-pair-literals", "result.stdout"), b"1")
            self.assertEqual(read("length-codepoints-reverse-pair", "result.stdout"), b"2")
            self.assertEqual(read("ref-blank-type", "result.stdout"), b"true")
            self.assertEqual(read("ref-blank-path", "result.stdout"), b"isBlank")
            self.assertEqual(read("ref-properties-lastindex-path", "result.stdout"), b"lastIndex")
            self.assertIn(b"Cannot find method `isBlank`",
                          read("typed-property-method", "result.stderr"))
            self.assertEqual(read("typed-property-method", "result.stderr").decode().splitlines()[1],
                             "Cannot find method `isBlank` in class `String`.")
            self.assertIn(b"Type constraint `isBlank` violated",
                          read("typed-failure-method", "result.stderr"))

    def test_bitset_distinguishes_pkl_from_jdk(self):
        pkl = (fixture.DEFAULT / "pkl-whitespace-bmp.bitset.bin").read_bytes()
        self.assertEqual(len(pkl), 8192)
        self.assertEqual(sum(byte.bit_count() for byte in pkl), 25)
        for cp in (0x85, 0xA0, 0x2007, 0x202F):
            self.assertTrue(pkl[cp >> 3] & (1 << (cp & 7)))
        for cp in range(0x1C, 0x20):
            self.assertFalse(pkl[cp >> 3] & (1 << (cp & 7)))

    def test_duplicate_id_query_order_and_missing_utf16_rejected(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        duplicate = copy.deepcopy(rows)
        duplicate[1]["id"] = duplicate[0]["id"]
        with self.assertRaisesRegex(ValueError, "case rows differ"):
            fixture.validate_rows(duplicate, (fixture.DEFAULT / "queries.tsv").read_bytes(),
                                  count=fixture.ROWS)
        with self.assertRaisesRegex(ValueError, "query order differs"):
            fixture.validate_rows(rows, b"string\tfake\t\"\".isBlank\n", count=fixture.ROWS)
        folder = f"{fixture.ARCHIVE_PREFIX}/repeat-0/000-empty/"
        files = {folder + name: b"" for name in ROW_CHANNELS}
        fixture.validate_row_channels(files, folder)
        files.pop(folder + "result.stdout.code-units.bin")
        with self.assertRaisesRegex(ValueError, "row channel set differs"):
            fixture.validate_row_channels(files, folder)

    def test_public_raw_jdk_identity_tamper_rejected(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            files = {member.name: archive.extractfile(member).read() for member in archive}
        name = f"{fixture.ARCHIVE_PREFIX}/java-identity.stderr.bin"
        files[name] = files[name].replace(b"21.0.12.1", b"21.0.12.2", 1)
        with self.assertRaisesRegex(ValueError, "raw Java identity differs"):
            validate_capture(files, fixture.ARCHIVE_PREFIX, mode="string",
                             rows_count=fixture.ROWS, expected_statuses=fixture.STATUSES,
                             expected_raw_files=fixture.RAW_FILES,
                             module_uri=fixture.MODULE_URI, source_names=SOURCES)

    def test_direct_source_and_bitset_tamper_rejected(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            files = {member.name: archive.extractfile(member).read() for member in archive}
        pkl = (fixture.DEFAULT / "pkl-whitespace-bmp.bitset.bin").read_bytes()
        source = f"{fixture.DIRECT_PREFIX}/sources/producer.py"
        files[source] += b"\n"
        with self.assertRaisesRegex(ValueError, "direct JDK source copy differs"):
            fixture.direct_capture(files, pkl)
        files[source] = files[source][:-1]
        with self.assertRaisesRegex(ValueError, "direct BMP bitset differs"):
            fixture.direct_capture(files, bytes(len(pkl)))

    def test_root_extras_and_authored_input_mutation_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            (root / "extra").mkdir()
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular"):
                fixture.verify(root)
            (root / "extra").rmdir()
            (root / "module.pkl").write_bytes((root / "module.pkl").read_bytes() + b"extra = 1\n")
            hashes = {p.name: fixture.sha(p.read_bytes()) for p in root.iterdir()
                      if p.name != "manifest.json"}
            (root / "manifest.json").write_text(json.dumps(
                {"schema": 1, "baseline": "Pkl 0.32.0", "files": hashes}))
            with self.assertRaisesRegex(ValueError, "archive identity differs|authored input differs"):
                fixture.verify(root)


if __name__ == "__main__":
    unittest.main()
