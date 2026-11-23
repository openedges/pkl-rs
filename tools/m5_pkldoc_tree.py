#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Recursive raw-byte + symlink tree comparator & offline package cache seeder for M5 Lane 5 (`pkldoc`).

Walks generated `pkldoc` output trees with `followlinks=False`, capturing:
  - every regular file under `generated/<rel>` with exact raw bytes
  - every symlink (file or dir symlink, e.g. `com.package1/current -> 1.2.3`)
    under `symlink/<rel>` with exact UTF-8 `os.readlink()` target bytes
Rejects unexpected special files and root symlinks.

Also deterministically seeds a `package-2/localhost(3a)0/` offline cache from the
pinned upstream packages under `external/pkl/pkl-commons-test/src/main/files/packages/`
(zero network calls, zero Gradle dependency) so `DocGeneratorTest` run-1 and run-2
execute identically against both pinned Java 0.32.0 `org.pkl.doc.Main` and Rust `pkldoc`.
"""
from __future__ import annotations

import hashlib
import io
import json
import os
from pathlib import Path
import stat
import zipfile


UPSTREAM_TEST_PACKAGES = Path("external/pkl/pkl-commons-test/src/main/files/packages")
PKLDOC_TEST_PACKAGE_ORDER = (
    # Seed dependencies before dependents so metadata JSON SHA-256s resolve cleanly!
    "fruit@1.0.5",
    "fruit@1.1.0",
    "birds@0.5.0",
    "birds@0.6.0",
    "birds@0.7.0",
    "unlisted@1.0.0",
    "deprecated@1.0.0",
)

# Original Gradle-built checksums embedded in birds@0.{5,6,7}.0.json
UPSTREAM_FRUIT_1_0_5_SHA = "8ff80e5ac882650e817f261c8af024bf0db9b07888c9d18c20017457e04ffe06"
UPSTREAM_FRUIT_1_1_0_SHA = "8d982761d182f2185e4180c82190791d9a60c721cb3393bb2e946fab90131e8c"


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def build_deterministic_zip(source_dir: Path) -> bytes:
    """Build a reproducible DEFLATE ZIP (sorted paths, fixed DOS 1980-02-01 mtime, no dir entries)."""
    buf = io.BytesIO()
    files: list[Path] = []
    for current, dirs, names in os.walk(source_dir, followlinks=False):
        dirs.sort()
        cur = Path(current)
        for name in sorted(names):
            p = cur / name
            if p.is_symlink() or not p.is_file():
                continue
            files.append(p)
    files.sort(key=lambda p: p.relative_to(source_dir).as_posix())
    with zipfile.ZipFile(buf, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as zf:
        for p in files:
            rel = p.relative_to(source_dir).as_posix()
            info = zipfile.ZipInfo(rel, date_time=(1980, 2, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            zf.writestr(info, p.read_bytes())
    return buf.getvalue()


def seed_pkldoc_package_cache(root: Path, cache_dir: Path) -> dict[str, dict[str, str]]:
    """Populate `<cache_dir>/package-2/localhost(3a)0/<pkg>/{<pkg>.json,<pkg>.zip,<pkg>.json.sha256}`."""
    src_root = root / UPSTREAM_TEST_PACKAGES
    base = cache_dir / "package-2" / "localhost(3a)0"
    base.mkdir(parents=True, exist_ok=True)

    meta_shas: dict[str, str] = {}
    records: dict[str, dict[str, str]] = {}

    for pkg in PKLDOC_TEST_PACKAGE_ORDER:
        pkg_src = src_root / pkg
        zip_bytes = build_deterministic_zip(pkg_src / "package")
        zip_sha = sha256_bytes(zip_bytes)

        meta_text = (pkg_src / f"{pkg}.json").read_text(encoding="utf-8")
        meta_text = meta_text.replace("$computedChecksum", zip_sha, 1)
        if "fruit@1.0.5" in meta_shas:
            meta_text = meta_text.replace(UPSTREAM_FRUIT_1_0_5_SHA, meta_shas["fruit@1.0.5"])
        if "fruit@1.1.0" in meta_shas:
            meta_text = meta_text.replace(UPSTREAM_FRUIT_1_1_0_SHA, meta_shas["fruit@1.1.0"])

        # Sanity-check valid JSON
        json.loads(meta_text)
        meta_bytes = meta_text.encode("utf-8")
        meta_sha = sha256_bytes(meta_bytes)
        meta_shas[pkg] = meta_sha

        dest = base / pkg
        dest.mkdir(parents=True, exist_ok=True)
        (dest / f"{pkg}.zip").write_bytes(zip_bytes)
        (dest / f"{pkg}.json").write_bytes(meta_bytes)
        (dest / f"{pkg}.json.sha256").write_bytes(meta_sha.encode("utf-8"))
        records[pkg] = {"zip_sha256": zip_sha, "meta_sha256": meta_sha}

    return records


def collect_pkldoc_tree(directory: Path) -> dict[str, bytes]:
    """Collect every regular file AND every symlink target in `directory` without following symlinks.

    Returns a sorted dict suitable for raw-byte differential storage via `compat.store_capture`:
      - Regular files -> `"generated/<rel_posix>": <raw file bytes>`
      - Symlinks      -> `"symlink/<rel_posix>": os.readlink(p).encode("utf-8")`
    """
    if directory.is_symlink():
        raise ValueError(f"pkldoc output root must not be a symlink: {directory}")
    if not directory.exists():
        return {}

    result: dict[str, bytes] = {}
    for current, dirs, files in os.walk(directory, followlinks=False):
        cur = Path(current)
        kept_dirs: list[str] = []
        for d in sorted(dirs):
            p = cur / d
            rel = p.relative_to(directory).as_posix()
            st = p.lstat()
            if stat.S_ISLNK(st.st_mode):
                # Directory symlink (e.g. `com.package1/current -> 1.2.3`) — record raw target, do not recurse!
                result[f"symlink/{rel}"] = os.readlink(p).encode("utf-8")
            elif stat.S_ISDIR(st.st_mode):
                kept_dirs.append(d)
            else:
                raise ValueError(f"unexpected non-directory entry in pkldoc tree: {p}")
        dirs[:] = kept_dirs

        for name in sorted(files):
            p = cur / name
            rel = p.relative_to(directory).as_posix()
            st = p.lstat()
            if stat.S_ISLNK(st.st_mode):
                result[f"symlink/{rel}"] = os.readlink(p).encode("utf-8")
            elif stat.S_ISREG(st.st_mode):
                result[f"generated/{rel}"] = p.read_bytes()
            else:
                raise ValueError(f"unexpected special file in pkldoc tree: {p}")

    return dict(sorted(result.items()))


# Explicit pkl-rs brand icon replacements (Apache-2.0 OpenEdges Inc., replacing
# Apple Pkl trademarked logo assets under Apache-2.0 Section 6).
PKLDOC_BRANDED_ICON_REL_PATHS: frozenset[str] = frozenset(
    {
        "images/apple-touch-icon.png",
        "images/favicon.svg",
        "images/favicon-16x16.png",
        "images/favicon-32x32.png",
    }
)


def compare_pkldoc_trees(
    expected: dict[str, bytes],
    actual: dict[str, bytes],
    root: Path,
) -> list[str]:
    """Compare Java/native-image pkldoc tree against Rust pkldoc tree.

    Enforces 100% raw-byte equality on every generated HTML/JSON/CSS/JS/font file
    and symlink, while verifying the 4 branded icon paths exist in both trees and
    match `crates/pkl-doc/assets/<rel>` byte-for-byte in Rust output.
    """
    diffs: list[str] = []
    branded_root = root / "crates" / "pkl-doc" / "assets"
    branded_keys = {f"generated/{rel}": rel for rel in PKLDOC_BRANDED_ICON_REL_PATHS}

    for name in sorted(set(expected) | set(actual)):
        if name in branded_keys:
            if name not in expected or name not in actual:
                diffs.append(f"files/{name}")
                continue
            expected_brand_bytes = (branded_root / branded_keys[name]).read_bytes()
            if actual[name] != expected_brand_bytes:
                diffs.append(f"files/{name}")
        elif expected.get(name) != actual.get(name):
            diffs.append(f"files/{name}")
    return diffs


def summarize_pkldoc_tree(tree: dict[str, bytes]) -> dict[str, int]:
    files = sum(1 for k in tree if k.startswith("generated/"))
    symlinks = sum(1 for k in tree if k.startswith("symlink/"))
    total_bytes = sum(len(v) for k, v in tree.items() if k.startswith("generated/"))
    return {"regular_files": files, "symlinks": symlinks, "total_bytes": total_bytes}
