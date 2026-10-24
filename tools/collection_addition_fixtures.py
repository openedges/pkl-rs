"""Offline integrity verifier for pinned Pkl 0.32.0 collection addition authority."""
import argparse
import hashlib
import json
from pathlib import Path
import tarfile

from collection_addition_oracle import SOURCES
from collection_conversions_fixtures import validate_capture, validate_rows, validate_row_channels

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "tests/fixtures/m2-collection-addition"
ARCHIVE_PREFIX = "capture-04"
ARCHIVE_SHA256 = "5e14a08075c7f488ca0a5dd2632956366a1a5f368428b5e325da584c499f801d"
FILESET_SHA256 = "2dd13812ecaedbcb2ca874ddfb2de3cf0df2c85e6760a46bb514bfdcdfbc0ac7"
MEMBERS = 5308
ROWS = 125
STATUSES = {"success": 86, "error": 39}
RAW_FILES = 2627
MODULE_URI = "file:///stage63/collection-addition.pkl"
ROOT_FILES = {"README.md", "archive-identity.json", "oracle-evidence.tar.gz",
              "module.pkl", "queries.tsv", "cases.json"}
INPUT_SHA256 = {
    "CollectionConversionsOracle.java": "6488595d614869fb59f50fe8d175f18e1d695031f768a38bfdc85f7fe28c9cc0",
    "run.py": "4b415302fad2f295ce368fa4c81a5330414e4f23bcd7c0ee6ce409a9e2d8fac8",
    "module.pkl": "50fc462c877bebd2a06bb9a295c2c9e89874705b5246e69aba7b7e9fbf41a3eb",
    "queries.tsv": "2081dfb9db79a5695e58c4149783fbd4b46c4c045b764de314ff68300d1d532f",
    "cases.json": "3a79ba3caa4ff3cd3180271e659b7323a7ca0f3b75fba9e15172ecb7d36cafd2",
}


def need(ok, message):
    if not ok:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def verify(root=DEFAULT, *, expected_archive=ARCHIVE_SHA256, expected_fileset=FILESET_SHA256):
    root = Path(root)
    need(root.is_dir() and not root.is_symlink(), "fixture directory missing or symlinked")
    entries = list(root.iterdir())
    need(all(path.is_file() and not path.is_symlink() for path in entries),
         "undeclared or nonregular fixture path")
    need({path.name for path in entries} == ROOT_FILES | {"manifest.json"},
         "fixture root file set differs")
    root_hashes = {path.name: sha(path.read_bytes()) for path in entries if path.name != "manifest.json"}
    manifest = json.loads((root / "manifest.json").read_bytes())
    need(manifest == {"schema": 1, "baseline": "Pkl 0.32.0", "files": root_hashes},
         "fixture manifest differs")
    need(root_hashes["oracle-evidence.tar.gz"] == expected_archive, "archive SHA differs")
    identity = json.loads((root / "archive-identity.json").read_bytes())
    need(identity == {"schema": 1, "archive_sha256": expected_archive,
                      "fileset_sha256": expected_fileset, "members": MEMBERS,
                      "captures": {ARCHIVE_PREFIX: "stage63-collection-addition-oracle-04"},
                      "oracle_input_sha256": INPUT_SHA256},
         "archive identity differs")
    files = {}
    digest = hashlib.sha256()
    with tarfile.open(root / "oracle-evidence.tar.gz", "r:gz") as archive:
        for member in archive:
            name = member.name
            need(member.isfile() and not Path(name).is_absolute()
                 and ".." not in Path(name).parts and name.startswith(ARCHIVE_PREFIX + "/")
                 and name not in files, "unsafe or duplicate archive member")
            raw = archive.extractfile(member).read()
            need(len(raw) == member.size, "archive member size differs")
            files[name] = raw
            digest.update(name.encode() + b"\0" + sha(raw).encode() + b"\n")
    need(len(files) == MEMBERS and identity["members"] == MEMBERS
         and digest.hexdigest() == expected_fileset, "archive file set differs")
    for name in ("cases.json", "queries.tsv", "module.pkl"):
        need(files[ARCHIVE_PREFIX + "/" + name] == (root / name).read_bytes(),
             "authored input differs")
    for name, expected in identity["oracle_input_sha256"].items():
        need(sha(files[ARCHIVE_PREFIX + "/" + name]) == expected,
             "archive input identity differs")
    statuses = validate_capture(files, ARCHIVE_PREFIX, mode="string", rows_count=ROWS,
                                expected_statuses=STATUSES, expected_raw_files=RAW_FILES,
                                module_uri=MODULE_URI, source_names=SOURCES)
    return {"baseline": "Pkl 0.32.0", "rows": ROWS, "statuses": statuses,
            "archive_sha256": expected_archive, "fileset_sha256": expected_fileset,
            "raw_repetitions_equal": True, "candidate_compatibility_passes_claimed": 0}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture), indent=2, sort_keys=True))
