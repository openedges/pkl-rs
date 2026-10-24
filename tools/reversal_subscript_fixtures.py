"""Offline integrity verifier for pinned Pkl 0.32.0 reversed List subscript diagnostic authority."""
import argparse
import hashlib
import json
from pathlib import Path
import tarfile

from reversal_subscript_oracle import SOURCES
from reversal_values_oracle import JDK_SOURCE_SHA256
from collection_conversions_fixtures import validate_capture, validate_rows, validate_row_channels

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "tests/fixtures/m2-reversal-subscript"
ARCHIVE_PREFIX = "capture-02"
ARCHIVE_SHA256 = "11a400a303c07dcc81dd53848a3590696c32ab889c605924e468faf8e5f4366b"
FILESET_SHA256 = "93e93f0ea83745f16ac9f7dc1e6eae461202eb8e0b39e8ea88725f5535f0a24d"
MEMBERS = 858
ROWS = 19
STATUSES = {"success": 1, "error": 18}
RAW_FILES = 401
MODULE_URI = "file:///stage66/reversal-subscript.pkl"
ROOT_FILES = {"README.md", "archive-identity.json", "oracle-evidence.tar.gz",
              "module.pkl", "queries.tsv", "cases.json"}
INPUT_SHA256 = {
    "CollectionConversionsOracle.java": "6488595d614869fb59f50fe8d175f18e1d695031f768a38bfdc85f7fe28c9cc0",
    "run.py": "48c24e95b209311a0472ff733c3941b553c18b2ce6025c36900806ea79ad54ce",
    "module.pkl": "a9045c2bd08d9097c9893fe60cb017a7b17bd7efbdaa801bc247bc103a5ff168",
    "queries.tsv": "f755443fea6b0646a306da03b6da2641b37d4bb544d59f0c47af5a46e416bc0b",
    "cases.json": "ea9f15510c8da63180c73d6a8cfacbd01159fe9b79e6ba72911bd776389f7899",
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
                      "captures": {ARCHIVE_PREFIX: "stage66-reversal-subscript-oracle-02"},
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
    need(files[ARCHIVE_PREFIX + "/run.py"] == (ROOT / "tools/reversal_subscript_oracle.py").read_bytes(),
         "archived producer differs from tracked producer")
    provenance = json.loads(files[ARCHIVE_PREFIX + "/provenance.json"])
    need(provenance["jdk"]["source_archive_sha256"] == JDK_SOURCE_SHA256
         and provenance["jdk_source_after_sha256"] == JDK_SOURCE_SHA256,
         "pinned JDK source archive before/after differs")
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
