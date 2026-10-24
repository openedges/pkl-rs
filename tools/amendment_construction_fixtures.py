"""Offline integrity verifier for pinned Pkl 0.32.0 amendment construction authority."""
import argparse
import hashlib
import json
from pathlib import Path
import tarfile

from amendment_construction_oracle import SOURCES
from collection_conversions_fixtures import validate_capture, validate_rows, validate_row_channels

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "tests/fixtures/m2-amendment-construction"
ARCHIVE_PREFIX = "capture-01"
ARCHIVE_SHA256 = "77c4a0f285dacdbf56a6f47cc39e67fc164bd76457510a125d7396b3034e9214"
FILESET_SHA256 = "8d3d05eb597e98e3dee47afde497df054870dc59d054e8b6b1c36af14b762967"
MEMBERS = 7983
ROWS = 188
STATUSES = {"success": 102, "error": 86}
RAW_FILES = 3950
MODULE_URI = "file:///stage64/amendment-construction.pkl"
ROOT_FILES = {"README.md", "archive-identity.json", "oracle-evidence.tar.gz",
              "module.pkl", "queries.tsv", "cases.json"}
INPUT_SHA256 = {
    "CollectionConversionsOracle.java": "6488595d614869fb59f50fe8d175f18e1d695031f768a38bfdc85f7fe28c9cc0",
    "run.py": "a73e1fa1fd047ff4477197c3715571ce21499ac5905148ecc7b045a1c63438cb",
    "module.pkl": "28ed759288cd664505347ef6e649d95c9495901787bfbea160c1aa5e6bc8efc6",
    "queries.tsv": "0f8105fad96003a6bcebc71c5c9e235148f6b3d33a1132e06c3eb0e69fda10d9",
    "cases.json": "97fa9ae8551b14f39dea2b352c9e00d22eae574c6c5ba25e7840742700343f84",
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
                      "captures": {ARCHIVE_PREFIX: "stage64-amendment-oracle-01"},
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
