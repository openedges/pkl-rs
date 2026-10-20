"""Generated package integrity and raw recording regressions (not Pkl assertions)."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
import zipfile

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import pantry


class PantryTests(unittest.TestCase):
    def test_empty_partial_and_duplicate_package_inventories_fail_closed(self):
        source = {"repository": "pkl-pantry", "path": "external/pkl-pantry", "resources": [
            {"path": "external/pkl-pantry/packages/a/PklProject"},
            {"path": "external/pkl-pantry/packages/b/PklProject"}]}
        a = {"suite": "package-api-tests", "source": {"path": "packages/a/PklProject"}}
        b = {"suite": "package-api-tests", "source": {"path": "packages/b/PklProject"}}
        for manifest in ({}, {"sources": [source], "cases": []},
                         {"sources": [source], "cases": [a]}, {"sources": [source], "cases": [a, b, b]}):
            with self.subTest(manifest=manifest), self.assertRaises(ValueError):
                pantry.required_package_cases(manifest)
        self.assertEqual(pantry.required_package_cases({"sources": [source], "cases": [a, b]}), [a, b])

    def test_timeout_and_signal_are_not_accepted_observations(self):
        for status, exit_code in (("timeout", None), ("executed", -9), ("executed", 2)):
            report = {"packaged_evaluations": [{"status": status, "exit_code": exit_code}]}
            self.assertEqual(len(pantry.invalid_observation_processes(report)), 1)
        self.assertEqual(pantry.invalid_observation_processes({"packaged_evaluations": [
            {"status": "executed", "exit_code": 0}, {"status": "executed", "exit_code": 1}]}), [])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            result = pantry.capture([sys.executable, "-c", "import os,time; os.write(1,b'partial'); time.sleep(60)"],
                                    root, {}, root / "process", timeout=0.1)
            self.assertEqual(result["status"], "timeout")
            self.assertIsNone(result["exit_code"])
            self.assertEqual(Path(result["stdout"]["path"]).read_bytes(), b"partial")

    def make_package(self, folder):
        folder.mkdir(parents=True)
        metadata = folder / "demo@1.0.0"
        archive = folder / "demo@1.0.0.zip"
        with zipfile.ZipFile(archive, "w") as zipped:
            zipped.writestr("Demo.pkl", "answer = 42\n")
            zipped.writestr("LICENSE.txt", "fixture")
        metadata.write_text(json.dumps({"packageUri": "package://pkg.pkl-lang.org/pkl-pantry/demo@1.0.0",
            "packageZipChecksums": {"sha256": pantry.sha(archive)}}))
        for path in (metadata, archive):
            path.with_name(path.name + ".sha256").write_text(pantry.sha(path))
        return metadata, archive

    def test_seed_requires_all_checksums_and_lists_only_pkl_members(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            metadata, archive = self.make_package(root / "generated")
            result = pantry.seed_package(metadata, root / "cache")
            self.assertEqual(result["modules"], ["package://pkg.pkl-lang.org/pkl-pantry/demo@1.0.0#/Demo.pkl"])
            cached = root / "cache/package-2/pkg.pkl-lang.org/pkl-pantry/demo@1.0.0/demo@1.0.0.zip"
            self.assertEqual(cached.read_bytes(), archive.read_bytes())
            cached.chmod(0o444)
            self.assertEqual(pantry.seed_package(metadata, root / "cache"), result)
            cached.chmod(0o644)
            archive.write_bytes(archive.read_bytes() + b"drift")
            with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                pantry.seed_package(metadata, root / "cache")

    def test_checksum_newline_is_not_silently_trimmed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            metadata, archive = self.make_package(root / "generated")
            checksum = archive.with_name(archive.name + ".sha256")
            checksum.write_bytes(checksum.read_bytes() + b"\n")
            with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                pantry.seed_package(metadata, root / "cache")

    def test_capture_preserves_stream_bytes_and_nonzero_exit(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            code = "import os; os.write(1,b' x \\r\\n\\x00'); os.write(2,b'err\\t'); raise SystemExit(7)"
            result = pantry.capture([sys.executable, "-c", code], root, {}, root / "process")
            self.assertEqual(result["exit_code"], 7)
            self.assertEqual(Path(result["stdout"]["path"]).read_bytes(), b" x \r\n\x00")
            self.assertEqual(Path(result["stderr"]["path"]).read_bytes(), b"err\t")
            self.assertEqual(result["stdout"]["sha256"], hashlib.sha256(b" x \r\n\x00").hexdigest())


if __name__ == "__main__":
    unittest.main()
