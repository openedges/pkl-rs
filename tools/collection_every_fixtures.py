"""Catalog-shaped offline verifier for the logging Pkl collection every fixture."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from collection_predicate_fixtures import verify as verify_predicate


def verify(root: Path | None = None) -> dict:
    return verify_predicate("collection-every", root)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixture", type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture), sort_keys=True))
