"""Catalog-shaped offline verifier for the Pkl collection emptiness fixture."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from collection_predicate_fixtures import verify as verify_predicate

ARCHIVE_PREFIX = "capture-02"


def verify(root: Path | None = None) -> dict:
    return verify_predicate("collection-emptiness", root)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixture", type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture), sort_keys=True))
