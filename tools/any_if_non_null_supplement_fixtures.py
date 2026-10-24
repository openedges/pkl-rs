"""Verify the sealed additive Any.ifNonNull Java authority without rerunning Java."""
import argparse
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from tools.any_if_non_null_fixtures import verify as verify_common

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "tests/fixtures/m2-any-if-non-null-supplement"
ARCHIVE_SHA256 = "5f6d6a155ab3df97ab26fd9ea994609c349d8484b3a899f13239820e21f26745"
FILESET_SHA256 = "604636263b7a977f5f91fceb0a82cd4a4e881521f0cafaaa9a9d36c446f5d9f7"


def verify(root=DEFAULT, *, expected_archive=ARCHIVE_SHA256, expected_fileset=FILESET_SHA256):
    return verify_common(root, expected_archive=expected_archive, expected_fileset=expected_fileset,
                         expected_capture="stage58-any-oracle-supplement-02",
                         archive_prefix="capture-supplement-02", expected_rows=13,
                         expected_members=255, expected_raw_files=119,
                         expected_module_uri="file:///stage58/any-if-non-null-supplement.pkl",
                         expected_statuses={"success": 12, "error": 1})


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture), sort_keys=True))
