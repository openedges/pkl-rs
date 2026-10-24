"""Integrity and completeness regressions for the sealed Any.ifNonNull authority."""
import gzip
import hashlib
import io
import json
from pathlib import Path
import shutil
import tarfile
import tempfile
import unittest

from tools.any_if_non_null_fixtures import DEFAULT, verify


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def manifest(root):
    files = {p.relative_to(root).as_posix(): sha(p.read_bytes()) for p in root.rglob("*")
             if p.is_file() and p.name != "manifest.json"}
    (root / "manifest.json").write_text(json.dumps({"schema": 1, "baseline": "Pkl 0.32.0",
                                                     "files": files}, indent=2, sort_keys=True) + "\n")


class AnyIfNonNullFixtureTests(unittest.TestCase):
    def test_sealed_authority(self):
        result = verify()
        self.assertEqual(result["rows"], 86)
        self.assertEqual(result["statuses"], {"success": 61, "error": 25})
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_changed_authored_query_rejected_even_with_new_manifest(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "fixture"
            shutil.copytree(DEFAULT, root)
            queries = root / "queries.tsv"
            queries.write_bytes(queries.read_bytes().replace(b"10.ifNonNull((x) -> x)",
                                                            b"11.ifNonNull((x) -> x)", 1))
            manifest(root)
            with self.assertRaisesRegex(ValueError, "authored input differs"):
                verify(root)

    def test_missing_raw_channel_rejected_with_new_archive_identity(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "fixture"
            shutil.copytree(DEFAULT, root)
            archive = root / "oracle-evidence.tar.gz"
            with tarfile.open(archive, "r:gz") as stream:
                members = [(item.name, stream.extractfile(item).read()) for item in stream]
            name_to_remove = "capture-04/repeat-0/000-scalar-int-identity/result.stderr.code-units.bin"
            self.assertIn(name_to_remove, [name for name, _ in members])
            members = [(name, raw) for name, raw in members if name != name_to_remove]
            file_set = hashlib.sha256()
            with archive.open("wb") as target:
                with gzip.GzipFile(fileobj=target, mode="wb", mtime=0, filename="") as compressed:
                    with tarfile.open(fileobj=compressed, mode="w") as stream:
                        for name, raw in members:
                            file_set.update(name.encode() + b"\0" + sha(raw).encode() + b"\n")
                            info = tarfile.TarInfo(name)
                            info.size = len(raw)
                            info.mode = 0o644
                            stream.addfile(info, io.BytesIO(raw))
            identity_file = root / "archive-identity.json"
            identity = json.loads(identity_file.read_bytes())
            identity.update(archive_sha256=sha(archive.read_bytes()),
                            fileset_sha256=file_set.hexdigest(), members=len(members))
            identity_file.write_text(json.dumps(identity, indent=2, sort_keys=True) + "\n")
            manifest(root)
            with self.assertRaisesRegex(ValueError, "archive identity differs"):
                verify(root, expected_archive=identity["archive_sha256"],
                       expected_fileset=identity["fileset_sha256"])

    def test_duplicate_archive_member_rejected_after_resealing(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "fixture"
            shutil.copytree(DEFAULT, root)
            archive = root / "oracle-evidence.tar.gz"
            with tarfile.open(archive, "r:gz") as stream:
                members = [(item.name, stream.extractfile(item).read()) for item in stream]
            members[-1] = members[0]
            file_set = hashlib.sha256()
            with archive.open("wb") as target:
                with gzip.GzipFile(fileobj=target, mode="wb", mtime=0, filename="") as compressed:
                    with tarfile.open(fileobj=compressed, mode="w") as stream:
                        for name, raw in members:
                            file_set.update(name.encode() + b"\0" + sha(raw).encode() + b"\n")
                            info = tarfile.TarInfo(name)
                            info.size = len(raw)
                            info.mode = 0o644
                            stream.addfile(info, io.BytesIO(raw))
            identity_file = root / "archive-identity.json"
            identity = json.loads(identity_file.read_bytes())
            identity.update(archive_sha256=sha(archive.read_bytes()), fileset_sha256=file_set.hexdigest())
            identity_file.write_text(json.dumps(identity, indent=2, sort_keys=True) + "\n")
            manifest(root)
            with self.assertRaisesRegex(ValueError, "unsafe or duplicate archive member"):
                verify(root, expected_archive=identity["archive_sha256"],
                       expected_fileset=identity["fileset_sha256"])


if __name__ == "__main__":
    unittest.main()
