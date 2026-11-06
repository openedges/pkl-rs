"""Integrity tests for the bounded metadata capture; never compatibility expectations."""
import tempfile
import unittest
from pathlib import Path
import sys
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m2_package_metadata as metadata


class MetadataCaptureTests(unittest.TestCase):
    def error_result(self, path, message):
        path.mkdir()
        (path / "status.txt").write_bytes(b"error\n")
        (path / "error-class.utf8").write_bytes(b"example.Error")
        (path / "error-class.utf16be").write_bytes("example.Error".encode("utf-16-be"))
        (path / "message.utf8").write_bytes(message)
        (path / "message.utf16be").write_bytes(message.decode().encode("utf-16-be"))

    def test_raw_whitespace_difference_is_mismatch(self):
        with tempfile.TemporaryDirectory() as temp:
            left, right = Path(temp) / "java", Path(temp) / "rust"
            self.error_result(left, b"message\n")
            self.error_result(right, b"message")
            result = metadata.compare(left, right)
            self.assertFalse(result["equal"])
            self.assertEqual(set(result["different"]), {"message.utf8", "message.utf16be"})

    def test_equal_complete_error_channels(self):
        with tempfile.TemporaryDirectory() as temp:
            left, right = Path(temp) / "java", Path(temp) / "rust"
            self.error_result(left, b"message\n")
            self.error_result(right, b"message\n")
            self.assertTrue(metadata.compare(left, right)["equal"])
            (right / "message.utf16be").unlink()
            with self.assertRaisesRegex(ValueError, "incomplete"):
                metadata.compare(left, right)

    def test_protocol_upgrade_does_not_hide_extra_class_channel(self):
        with tempfile.TemporaryDirectory() as temp:
            left, right = Path(temp) / "java", Path(temp) / "rust"
            self.error_result(left, b"message")
            self.error_result(right, b"message")
            (left / "error-class.utf16be").unlink()
            result = metadata.compare(left, right, oracle_protocol=1)
            self.assertFalse(result["equal"])
            self.assertEqual(result["extra"], ["error-class.utf16be"])

    def test_timeout_retains_both_raw_channels(self):
        with tempfile.TemporaryDirectory() as temp:
            error = metadata.subprocess.TimeoutExpired(["probe"], 1, b"partial", b"error")
            with mock.patch.object(metadata.subprocess, "run", side_effect=error):
                result = metadata.run(["probe"], Path(temp) / "run", {}, timeout=1)
            self.assertTrue(result["timeout"])
            self.assertIsNone(result["exit_code"])
            self.assertEqual((Path(temp) / "run/stdout.bin").read_bytes(), b"partial")
            self.assertEqual((Path(temp) / "run/stderr.bin").read_bytes(), b"error")

    def test_unknown_or_missing_status_is_not_success(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)
            with self.assertRaisesRegex(ValueError, "status"):
                metadata.result_files(path)
            (path / "status.txt").write_bytes(b"ok")
            with self.assertRaisesRegex(ValueError, "status"):
                metadata.result_files(path)

    def test_prohibited_paths_rejected_before_resolution(self):
        with mock.patch.object(Path, "resolve", side_effect=AssertionError("filesystem touched")):
            for path in ("/tmp/Stage86-data/file", "/tmp/private8-run/file",
                         "/tmp/stage65-b-native-regressions-evidence.tar.gz.partial",
                         "/tmp/stage136-old/evidence.tar.gz"):
                with self.assertRaises(ValueError):
                    metadata.checked(path)

    def test_cases_keep_input_bytes_and_independent_modes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for name in metadata.PACKAGES:
                (root / name).mkdir()
                (root / name / (name + ".json")).write_bytes(b'{"raw": 1}\r\n')
            cases = metadata.cases({"fixture_root": str(root)})
            self.assertEqual(len(cases), len({case["id"] for case in cases}))
            self.assertTrue(all(case["body"] == b'{"raw": 1}\r\n' for case in cases[:5]))
            cases = {case["id"]: case for case in cases}
            self.assertEqual(cases["malformed-wrong-checksum"]["body"],
                             cases["malformed-correct-checksum"]["body"])
            self.assertNotEqual(cases["malformed-wrong-checksum"]["expected"],
                                cases["malformed-correct-checksum"]["expected"])
            for mode in ("on", "off"):
                self.assertEqual(cases["skip-valid-" + mode]["expected"], metadata.SKIP)
                self.assertEqual(cases["skip-valid-" + mode]["test_mode"], mode == "on")
            self.assertEqual(cases["missing-name"]["body"], b"{}")


if __name__ == "__main__":
    unittest.main()
