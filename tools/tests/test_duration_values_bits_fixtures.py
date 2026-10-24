"""Separate exported Duration bit observation integrity regressions."""
import json
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import duration_values_bits_fixtures as fixture


class DurationBitsFixtureTests(unittest.TestCase):
    def test_separate_offline_java_authority(self):
        with patch("subprocess.Popen", side_effect=AssertionError("launched Java")):
            result = fixture.verify()
        self.assertEqual(result["statuses"], {"success": 15})
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_same_unit_overflow_and_signed_zero_are_raw_bits(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            audit = json.load(archive.extractfile(fixture.ARCHIVE_PREFIX + "/audit.json"))
        by_id = {row["id"]: row for row in audit["observations"]}
        self.assertEqual(by_id["same-unit-huge-day"]["raw_bits_hex"], "7ff0000000000000")
        self.assertEqual(by_id["negative-zero-convert"]["raw_bits_hex"], "8000000000000000")
        self.assertEqual(by_id["min-subnormal-to-day"]["raw_bits_hex"], "0000000000000000")

    def test_missing_raw_bits_channel_rejected(self):
        folder = "repeat-0/000-int-beyond-f64-exact/"
        files = {folder + name: b"" for name in fixture.ROW_CHANNELS}
        fixture.validate_row_channels(files, folder)
        files.pop(folder + "result.raw-bits-hex.bin")
        with self.assertRaisesRegex(ValueError, "row channel set differs"):
            fixture.validate_row_channels(files, folder)

    def test_resigned_archive_append_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            archive = root / "oracle-evidence.tar.gz"
            archive.write_bytes(archive.read_bytes() + b"fake")
            manifest = json.loads((root / "manifest.json").read_bytes())
            manifest["files"]["oracle-evidence.tar.gz"] = fixture.sha(archive.read_bytes())
            (root / "manifest.json").write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "archive SHA differs"):
                fixture.verify(root)


if __name__ == "__main__":
    unittest.main()
