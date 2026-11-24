#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Verify release version lockstep across Cargo.toml, npm packages, and Git tags.

Checks that:
1. Root `Cargo.toml` (`[workspace.package].version`),
   `packages/pkl-wasm/package.json` (`version`), and
   `packages/pkl-napi/package.json` (`version`) all declare valid SemVer and
   match each other identically.
2. When `--tag` is given (e.g. `v0.32.0`, `v0.32.0-rc.1`, or `refs/tags/v0.32.0`),
   the tag obeys `^v<MAJOR>.<MINOR>.<PATCH>(-<prerelease>)?$` and its version
   exactly matches every package manifest.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]

SEMVER_RE = re.compile(
    r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-((?:0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*)"
    r"(?:\.(?:0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*))*))?$"
)


def parse_workspace_cargo_version(cargo_toml_text: str) -> str:
    """Extract `version` from `[workspace.package]` in root Cargo.toml."""
    in_section = False
    for raw_line in cargo_toml_text.splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            in_section = line == "[workspace.package]"
            continue
        if in_section:
            m = re.match(r'^version\s*=\s*"([^"]+)"$', line)
            if m:
                return m.group(1)
    raise ValueError("Could not find `[workspace.package].version` in Cargo.toml")


def parse_npm_version(package_json_path: Path) -> str:
    data = json.loads(package_json_path.read_text(encoding="utf-8"))
    version = data.get("version")
    if not isinstance(version, str) or not version:
        raise ValueError(f"Missing string `version` in {package_json_path}")
    return version


def normalize_tag(raw_tag: str) -> str:
    tag = raw_tag.strip()
    if tag.startswith("refs/tags/"):
        tag = tag[len("refs/tags/") :]
    if not tag.startswith("v"):
        raise ValueError(f"Release tag must start with 'v' (got {raw_tag!r})")
    version = tag[1:]
    if not SEMVER_RE.match(version):
        raise ValueError(
            f"Release tag {raw_tag!r} must match vMAJOR.MINOR.PATCH[-prerelease] "
            "(build metadata '+' is not allowed in release tags)"
        )
    return version


def parse_workspace_internal_dep_versions(cargo_toml_text: str) -> dict[str, str]:
    """Extract `version` for internal workspace path dependencies under `[workspace.dependencies]`."""
    in_section = False
    versions: dict[str, str] = {}
    for raw_line in cargo_toml_text.splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            in_section = line == "[workspace.dependencies]"
            continue
        if in_section:
            m = re.match(r'^([A-Za-z0-9_-]+)\s*=\s*\{(.+)\}$', line)
            if not m:
                continue
            dep_name, body = m.group(1), m.group(2)
            path_m = re.search(r'\bpath\s*=\s*"([^"]+)"', body)
            if not path_m or not path_m.group(1).startswith("crates/"):
                continue
            ver_m = re.search(r'\bversion\s*=\s*"([^"]+)"', body)
            if not ver_m:
                raise ValueError(
                    f"Workspace internal dependency {dep_name!r} in `[workspace.dependencies]` "
                    "must declare an explicit `version` for crates.io publishing"
                )
            versions[f"Cargo.toml:[workspace.dependencies].{dep_name}"] = ver_m.group(1)
    return versions


def verify_versions(root: Path = ROOT, tag: str | None = None) -> dict[str, str]:
    cargo_toml_text = (root / "Cargo.toml").read_text(encoding="utf-8")
    cargo_version = parse_workspace_cargo_version(cargo_toml_text)
    internal_dep_versions = parse_workspace_internal_dep_versions(cargo_toml_text)
    wasm_version = parse_npm_version(root / "packages" / "pkl-wasm" / "package.json")
    napi_version = parse_npm_version(root / "packages" / "pkl-napi" / "package.json")

    versions = {
        "Cargo.toml": cargo_version,
        **internal_dep_versions,
        "packages/pkl-wasm/package.json": wasm_version,
        "packages/pkl-napi/package.json": napi_version,
    }

    for path_label, ver in versions.items():
        if not SEMVER_RE.match(ver):
            raise ValueError(f"Invalid SemVer in {path_label}: {ver!r}")

    distinct = set(versions.values())
    if len(distinct) != 1:
        details = ", ".join(f"{k}={v}" for k, v in versions.items())
        raise ValueError(f"Package version mismatch across manifests: {details}")

    unified = cargo_version
    if tag is not None and tag.strip():
        tag_version = normalize_tag(tag)
        if tag_version != unified:
            raise ValueError(
                f"Release tag {tag!r} (version {tag_version}) does not match "
                f"manifest version {unified}"
            )
        versions["tag"] = tag_version
    return versions


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=ROOT,
        help="Repository root directory (defaults to repo root)",
    )
    parser.add_argument(
        "--tag",
        default=None,
        help="Optional Git release tag (e.g. v0.32.0, v0.32.0-rc.1, or refs/tags/v0.32.0)",
    )
    args = parser.parse_args(argv)
    try:
        result = verify_versions(root=args.root, tag=args.tag)
    except ValueError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1
    print(json.dumps({"status": "ok", "versions": result}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
