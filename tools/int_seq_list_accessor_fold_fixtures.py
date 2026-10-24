"""Offline verifier for independent Java List accessor/fold prerequisite evidence."""
import argparse
import hashlib
import json
from pathlib import Path
import tarfile

from int_seq_list_accessor_fold_oracle import SOURCES
import int_seq_values_fixtures as common

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "tests/fixtures/m2-int-seq-list-accessor-fold"
ARCHIVE_PREFIX = "capture-04"
ARCHIVE_SHA256 = "e6ce4f1ba66f6b8cc241fd768309862dadc449a5b1652cc31d8d61e1ebebb90c"
FILESET_SHA256 = "1eddaeaf4fcf6572de3174642e5afb15fb82f3c71d66cc20a75ed0ce0a97b027"
MODULE_URI = "file:///stage61/list-accessor-fold.pkl"
MEMBERS = 2755


def verify(root=DEFAULT, *, expected_archive=ARCHIVE_SHA256, expected_fileset=FILESET_SHA256):
    root = Path(root)
    common.need(root.is_dir() and not root.is_symlink(), "fixture directory missing or symlinked")
    entries = list(root.iterdir())
    common.need(all(p.is_file() and not p.is_symlink() for p in entries),
                "undeclared or nonregular fixture path")
    root_files = {p.name: common.sha(p.read_bytes()) for p in entries if p.name != "manifest.json"}
    common.need(set(root_files) == {"README.md", "archive-identity.json", "oracle-evidence.tar.gz",
                                    "module.pkl", "queries.tsv", "cases.json"}
                and len(entries) == 7, "fixture root file set differs")
    common.need(common.load_json((root / "manifest.json").read_bytes()) ==
                {"schema": 1, "baseline": "Pkl 0.32.0", "files": root_files},
                "fixture manifest differs")
    common.need(root_files["oracle-evidence.tar.gz"] == expected_archive, "archive SHA differs")
    identity = common.load_json((root / "archive-identity.json").read_bytes())
    common.need(identity["schema"] == 1 and identity["capture"] == "stage61-intseq-list-accessor-fold-04"
                and identity["prefix"] == ARCHIVE_PREFIX and identity["members"] == MEMBERS
                and identity["archive_sha256"] == expected_archive
                and identity["fileset_sha256"] == expected_fileset,
                "archive identity differs")
    files = {}
    fileset = hashlib.sha256()
    with tarfile.open(root / "oracle-evidence.tar.gz", "r:gz") as archive:
        for member in archive:
            name = member.name
            common.need(member.isfile() and name.startswith(ARCHIVE_PREFIX + "/")
                        and ".." not in Path(name).parts and not Path(name).is_absolute()
                        and name not in files, "unsafe or duplicate archive member")
            raw = archive.extractfile(member).read()
            common.need(len(raw) == member.size, "archive member size differs")
            files[name] = raw
            fileset.update(name.encode() + b"\0" + common.sha(raw).encode() + b"\n")
    common.need(len(files) == MEMBERS and fileset.hexdigest() == expected_fileset,
                "archive file set differs")
    def capture(name):
        return files[ARCHIVE_PREFIX + "/" + name]
    for name in ("module.pkl", "queries.tsv", "cases.json"):
        common.need(capture(name) == (root / name).read_bytes(), "authored input differs")
    common.need(set(identity["input_sha256"]) == common.INPUT_NAMES | {"provenance.json", "audit.json"},
                "archive input list differs")
    for name, expected in identity["input_sha256"].items():
        common.need(common.sha(capture(name)) == expected, "archive input hash differs")
    statuses = common.validate_capture(files, ARCHIVE_PREFIX, mode="string", rows_count=65,
                                       expected_statuses={"success": 37, "error": 28},
                                       expected_raw_files=1367, module_uri=MODULE_URI,
                                       source_names=SOURCES)
    return {"rows": 65, "statuses": statuses, "archive_sha256": expected_archive,
            "fileset_sha256": expected_fileset, "candidate_compatibility_passes_claimed": 0}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture), indent=2, sort_keys=True))
