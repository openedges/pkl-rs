import gzip
import hashlib
import io
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import evidence


class EvidenceTests(unittest.TestCase):
    def test_exact_archive_replay_and_exclusions(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "inputs"
            source.mkdir()
            (source / "output.bin").write_bytes(b"\xff\r\n\x00")
            (source / "work").mkdir()
            (source / "work/copy").write_text("excluded copy")
            evidence.create(source, root / "one.tar.gz", ["work/*"])
            evidence.create(source, root / "two.tar.gz", ["work/*"])
            self.assertEqual((root / "one.tar.gz").read_bytes(), (root / "two.tar.gz").read_bytes())
            with tarfile.open(root / "one.tar.gz") as archive:
                self.assertEqual(["output.bin", "EVIDENCE.json"], archive.getnames())

    def test_missing_modified_or_unexpected_bytes_fail(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "bad.tar.gz"
            for files in ([], [("wrong", b"x")], [("expected", b"wrong")], [("../escape", b"x")]):
                with tarfile.open(path, "w:gz") as archive:
                    inventory = {"files": [{"path": "expected", "bytes": 1, "sha256": "0" * 64}]}
                    for name, data in files + [("EVIDENCE.json", json.dumps(inventory).encode())]:
                        info = tarfile.TarInfo(name)
                        info.size = len(data)
                        archive.addfile(info, io.BytesIO(data))
                with self.assertRaises(ValueError):
                    evidence.verify(path)

    def test_empty_source_cannot_pass(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            empty = root / "empty"
            empty.mkdir()
            with self.assertRaisesRegex(ValueError, "Empty"):
                evidence.create(empty, root / "archive.tar.gz", [])

    def test_inventory_order_does_not_change_member_identity(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "reordered.tar.gz"
            files = [("first.bin", b"\xff\r\n\x00"), ("second.bin", b"other")]
            inventory = {"files": [
                {"path": name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
                for name, data in reversed(files)
            ]}
            with tarfile.open(path, "w:gz") as archive:
                for name, data in files + [("EVIDENCE.json", json.dumps(inventory).encode())]:
                    info = tarfile.TarInfo(name)
                    info.size = len(data)
                    archive.addfile(info, io.BytesIO(data))
            evidence.verify(path)

    def test_duplicate_members_cannot_be_hidden_by_indexing(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "duplicate.tar.gz"
            data = b"expected"
            inventory = {"files": [{"path": "output.bin", "bytes": len(data),
                                    "sha256": hashlib.sha256(data).hexdigest()}]}
            with tarfile.open(path, "w:gz") as archive:
                for name, payload in [("output.bin", b"wrong"), ("output.bin", data),
                                      ("EVIDENCE.json", json.dumps(inventory).encode())]:
                    info = tarfile.TarInfo(name)
                    info.size = len(payload)
                    archive.addfile(info, io.BytesIO(payload))
            with self.assertRaisesRegex(ValueError, "duplicate"):
                evidence.verify(path)


if __name__ == "__main__":
    unittest.main()
