#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Verify ELF binaries/shared libraries require no GLIBC symbol newer than --max-glibc."""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

GLIBC_RE = re.compile(r"GLIBC_(\d+)\.(\d+)(?:\.(\d+))?")


def parse_version(text: str) -> tuple[int, int, int]:
    parts = [int(p) for p in text.split(".")]
    while len(parts) < 3:
        parts.append(0)
    return (parts[0], parts[1], parts[2])


def max_glibc_version(readelf_output: str) -> tuple[int, int, int]:
    versions = [
        (int(m.group(1)), int(m.group(2)), int(m.group(3) or 0))
        for m in GLIBC_RE.finditer(readelf_output)
    ]
    return max(versions, default=(0, 0, 0))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--max-glibc", default="2.28", help="Maximum allowed GLIBC version (default: 2.28)")
    parser.add_argument("binaries", nargs="+", type=Path)
    args = parser.parse_args()
    limit = parse_version(args.max_glibc)
    failed = False
    for binary in args.binaries:
        out = subprocess.check_output(["readelf", "-V", str(binary)], text=True)
        observed = max_glibc_version(out)
        print(f"{binary}: max GLIBC = {observed[0]}.{observed[1]}" + (f".{observed[2]}" if observed[2] else ""))
        if observed > limit:
            print(f"ERROR: {binary} exceeds max allowed GLIBC {args.max_glibc}", file=sys.stderr)
            failed = True
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
