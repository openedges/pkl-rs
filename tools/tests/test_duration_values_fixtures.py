"""Offline integrity and protocol regressions for Stage 59 Java Duration authority."""
import copy
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import duration_values_fixtures as fixture


class DurationFixtureTests(unittest.TestCase):
    def test_sealed_authority_is_offline_and_claims_no_candidate_pass(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = fixture.verify()
        self.assertEqual(result["rows"], 274)
        self.assertEqual(result["statuses"], {"success": 240, "error": 34})
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_resigned_root_query_drift_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            query = root / "queries.tsv"
            query.write_bytes(query.read_bytes() + b"fake\t1.s\n")
            manifest = json.loads((root / "manifest.json").read_bytes())
            manifest["files"]["queries.tsv"] = fixture.sha(query.read_bytes())
            (root / "manifest.json").write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "authored input differs"):
                fixture.verify(root)

    def test_archive_append_and_manifest_refresh_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            archive = root / "oracle-evidence.tar.gz"
            archive.write_bytes(archive.read_bytes() + b"fake-pass")
            manifest = json.loads((root / "manifest.json").read_bytes())
            manifest["files"]["oracle-evidence.tar.gz"] = fixture.sha(archive.read_bytes())
            (root / "manifest.json").write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "archive SHA differs"):
                fixture.verify(root)

    def test_duplicate_id_and_query_reorder_rejected(self):
        cases = json.loads((fixture.DEFAULT / "cases.json").read_bytes())
        queries = (fixture.DEFAULT / "queries.tsv").read_bytes()
        duplicate = copy.deepcopy(cases["rows"])
        duplicate[1]["id"] = duplicate[0]["id"]
        with self.assertRaisesRegex(ValueError, "case rows differ"):
            fixture.validate_rows(duplicate, queries)
        with self.assertRaisesRegex(ValueError, "query order differs"):
            fixture.validate_rows(cases["rows"], b"fake\t1.s\n")

    def test_missing_and_extra_raw_channel_rejected(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            prefix = fixture.ARCHIVE_PREFIX + "/repeat-0/000-int-ns/"
            files = {member.name.removeprefix(fixture.ARCHIVE_PREFIX + "/"): b""
                     for member in archive if member.name.startswith(prefix)}
        folder = "repeat-0/000-int-ns/"
        fixture.validate_row_channels(files, folder)
        missing = dict(files)
        missing.pop(folder + "result.stderr.bin")
        with self.assertRaisesRegex(ValueError, "row channel set differs"):
            fixture.validate_row_channels(missing, folder)
        extra = dict(files)
        extra[folder + "fake-pass.bin"] = b""
        with self.assertRaisesRegex(ValueError, "row channel set differs"):
            fixture.validate_row_channels(extra, folder)

    def test_full_process_protocol_detects_missing_call(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            raw = archive.extractfile(fixture.ARCHIVE_PREFIX + "/repeat-0/process.stdout.bin").read()
        lines = raw.decode().splitlines()
        fixture.validate_process_protocol(raw, lines)
        with self.assertRaisesRegex(ValueError, "complete process protocol differs"):
            fixture.validate_process_protocol(raw.replace(b"call-enter=string-0-int-ns\n", b"", 1), lines)

    def test_optimized_python_still_rejects_duplicate_ids(self):
        code = ("import json,sys;sys.path.insert(0,sys.argv[1]);"
                "import duration_values_fixtures as f;"
                "c=json.load(open(sys.argv[2]));r=c['rows'];r[1]['id']=r[0]['id'];"
                "f.validate_rows(r,open(sys.argv[3],'rb').read())")
        result = subprocess.run([sys.executable, "-O", "-c", code,
                                 str(Path(__file__).resolve().parents[1]),
                                 str(fixture.DEFAULT / "cases.json"),
                                 str(fixture.DEFAULT / "queries.tsv")], capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"case rows differ", result.stderr)


if __name__ == "__main__":
    unittest.main()
