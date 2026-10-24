"""Offline integrity verifier for pinned Pkl 0.32.0 Set endpoint authority."""
import argparse
import hashlib
import json
from pathlib import Path
import tarfile

from set_endpoints_oracle import SOURCES
from collection_conversions_fixtures import validate_capture, validate_rows, validate_row_channels

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "tests/fixtures/m2-set-endpoints"
ARCHIVE_PREFIX = "capture-02"
ARCHIVE_SHA256 = "9a1326f0220b92607779bd02e2fc41d81d7a9cea8f40a3f404a55f3be03e1ae9"
FILESET_SHA256 = "2c09fa7ccef3c42e6ea8a12644440969ee575eef5e4701eb21886d93347c39fe"
MEMBERS = 2990
ROWS = 70
STATUSES = {"success": 47, "error": 23}
RAW_FILES = 1472
MODULE_URI = "file:///stage63/set-endpoints.pkl"
ROOT_FILES = {"README.md", "archive-identity.json", "oracle-evidence.tar.gz",
              "module.pkl", "queries.tsv", "cases.json"}
INPUT_SHA256 = {
    "CollectionConversionsOracle.java": "6488595d614869fb59f50fe8d175f18e1d695031f768a38bfdc85f7fe28c9cc0",
    "run.py": "cd8fa1d9f7ce2c73eeb4cd774f92c46a1633ac61a46d820170871bb104e5c1b3",
    "module.pkl": "add28c5d462e41c7c5d8fa273817669d6e1234f85adc2ac8fac5ea0b3160fe70",
    "queries.tsv": "eb9e0db1e0e62f87077a8c9c20b6bde4fa8636f85e0634b574f4a5fbd7dce7d4",
    "cases.json": "d70067c1ea0f45960e6b693db60165521a2b45b2ac35c8285f300446c2043c02",
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
                      "captures": {ARCHIVE_PREFIX: "stage63-set-endpoints-oracle-02"},
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
    need(files[ARCHIVE_PREFIX + "/run.py"] == (ROOT / "tools/set_endpoints_oracle.py").read_bytes(),
         "archived producer source differs")
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
