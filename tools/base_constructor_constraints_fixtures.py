"""Offline integrity verifier for pinned Pkl 0.32.0 base constructor constraint authority."""
import argparse
import hashlib
import json
from pathlib import Path
import tarfile

from base_constructor_constraints_oracle import SOURCES
from reversal_values_oracle import JDK_SOURCE_SHA256
from collection_conversions_fixtures import validate_capture, validate_rows, validate_row_channels

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "tests/fixtures/m2-base-constructor-constraints"
ARCHIVE_PREFIX = "capture-06"
ARCHIVE_SHA256 = "3f5a609f62a40e004dd208bc078be9e88a3a4a94c30ee4a04ebb5450d5eb8c6e"
FILESET_SHA256 = "8fdeee6ec76ff6f777048008ca815e8e00693330f16240d9c6c7d04d06c0a6ec"
MEMBERS = 1588
ROWS = 36
STATUSES = {"success": 20, "error": 16}
RAW_FILES = 758
MODULE_URI = "file:///stage66/base-constructors-in-constraints.pkl"
ROOT_FILES = {"README.md", "archive-identity.json", "oracle-evidence.tar.gz",
              "module.pkl", "queries.tsv", "cases.json"}
INPUT_SHA256 = {
    "CollectionConversionsOracle.java": "6488595d614869fb59f50fe8d175f18e1d695031f768a38bfdc85f7fe28c9cc0",
    "run.py": "9ec297ccb2248e99578110b4d5b4824764e70a149df967a2cb78cd1db8227120",
    "module.pkl": "818236da99270a29e3326829fd85dfed08ce39a1db2a1ccf2553ff077f650bb1",
    "queries.tsv": "40298bcc992b9c035267816faef9b76cb990d9dae3eabfcabd94215bf758fcaf",
    "cases.json": "d0517af9355f95acb2d14daf83680781761b86b2732adf8dbbf6b45dfe47dea5",
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
                      "captures": {ARCHIVE_PREFIX: "stage66-base-constructor-constraint-oracle-06"},
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
    need(files[ARCHIVE_PREFIX + "/run.py"] == (ROOT / "tools/base_constructor_constraints_oracle.py").read_bytes(),
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
