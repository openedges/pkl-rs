#!/usr/bin/env python3
"""Package reviewable raw evidence with a checked file inventory and deterministic archive headers."""
import argparse
import fnmatch
import gzip
import hashlib
import io
import json
from pathlib import Path
import sys
import tarfile


def create(source: Path, destination: Path, excludes: list[str]):
    source = source.resolve()
    if not source.is_dir() or destination.resolve().is_relative_to(source):
        raise ValueError("Evidence source must be a directory outside the destination archive")
    rows = []
    paths = []
    for path in sorted(source.rglob("*")):
        name = path.relative_to(source).as_posix()
        if any(fnmatch.fnmatchcase(name, pattern) for pattern in excludes):
            continue
        if path.is_symlink():
            raise ValueError(f"Evidence must contain regular files, not links: {name}")
        if path.is_file():
            if name == "EVIDENCE.json":
                raise ValueError("Reserved evidence inventory path")
            data = path.read_bytes()
            rows.append({"path": name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()})
            paths.append((name, data))
    if not paths:
        raise ValueError("Empty evidence cannot be archived")
    inventory = {"schema_version": 1, "excluded_patterns": excludes, "files": rows}
    paths.append(("EVIDENCE.json", (json.dumps(inventory, indent=2) + "\n").encode()))
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open("wb") as raw, gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w") as archive:
            for name, data in paths:
                info = tarfile.TarInfo(name)
                info.size, info.mode, info.mtime = len(data), 0o644, 0
                archive.addfile(info, io.BytesIO(data))
    verify(destination)


def verify(path: Path):
    with tarfile.open(path, "r:gz") as archive:
        members = archive.getmembers()
        names = [member.name for member in members]
        if len(names) != len(set(names)) or "EVIDENCE.json" not in names:
            raise ValueError("Missing inventory or duplicate archive members")
        if any(not member.isfile() or Path(member.name).is_absolute() or ".." in Path(member.name).parts for member in members):
            raise ValueError("Unsafe archive member")
        # TarFile's string lookup scans member names. Keep the checked members
        # indexed so large evidence inventories do not repeat that scan per file.
        indexed = {member.name: member for member in members}
        inventory = json.load(archive.extractfile(indexed["EVIDENCE.json"]))
        rows = inventory["files"]
        expected = {row["path"] for row in rows}
        if not rows or len(expected) != len(rows) or expected != set(names) - {"EVIDENCE.json"}:
            raise ValueError("Incomplete or empty evidence inventory")
        for row in rows:
            data = archive.extractfile(indexed[row["path"]]).read()
            if len(data) != row["bytes"] or hashlib.sha256(data).hexdigest() != row["sha256"]:
                raise ValueError(f"Evidence bytes do not match inventory: {row['path']}")
    print(f"Verified {len(rows)} evidence files: {path}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    build = sub.add_parser("create")
    build.add_argument("source", type=Path)
    build.add_argument("destination", type=Path)
    build.add_argument("--exclude", action="append", default=[])
    check = sub.add_parser("verify")
    check.add_argument("archive", type=Path)
    args = parser.parse_args()
    try:
        if args.action == "create":
            create(args.source, args.destination, args.exclude)
        else:
            verify(args.archive)
        return 0
    except (OSError, ValueError, KeyError, tarfile.TarError) as error:
        print(f"Evidence verification failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
