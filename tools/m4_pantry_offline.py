#!/usr/bin/env python3
"""Offline Pantry dependency verification, staging, and dev/packaged case enumeration.

Extracts and cryptographically verifies the 7 remote Pantry dependencies already
retained inside `reports/m0/pantry-evidence.tar.gz` (all 3 `pkl-k8s/k8s@*` packages
needed by the 29 Pantry packages plus the 4 `.github/` lockfile packages), enabling
100% offline warm-cache AND cold-cache localhost HTTPS mirror execution against
pinned Java 0.32.0 and Rust without live internet access.
"""
from __future__ import annotations

import base64
import hashlib
import json
from pathlib import Path
import re
import shutil
import tarfile
from urllib.parse import urlsplit
import zipfile

ROOT = Path(__file__).resolve().parents[1]
PANTRY_EVIDENCE_ARCHIVE = Path("reports/m0/pantry-evidence.tar.gz")
CORPUS_PATH = Path("compatibility/corpus.json")
STRUCTURED_READ_EXPECTED_PCF = Path(
    "external/pkl-pantry/packages/pkl.experimental.structuredRead/tests/structuredRead.pkl-expected.pcf"
)

# Exact SHA-256 digests of the two pinned GitHub raw files referenced by
# `pkl.experimental.structuredRead` (`tests/structuredRead.pkl` and `examples/https/https.pkl`),
# verified against both `external/pkl-pantry` git commit 8fcea0e535d2eb473545305d8a573144f2b0d33d
# and `tests/structuredRead.pkl-expected.pcf` base64 payloads.
STRUCTURED_READ_PINNED_SHA256: dict[str, str] = {
    "apple/pkl-pantry/8fcea0e535d2eb473545305d8a573144f2b0d33d/README.adoc": (
        "a625dcd9a75787818463c896b5d0fd056f2355240b393bb64c6da331bc94fe30"
    ),
    "apple/pkl-pantry/8fcea0e535d2eb473545305d8a573144f2b0d33d/CONTRIBUTING.adoc": (
        "f0fabfd36a31ee3ae26c0b5dfd44156bae3f49169f5798983002d409481bc80c"
    ),
}

# The 2 configured Pantry generator evaluations (matches tools/pantry.py)
CONFIGURED_GENERATORS: tuple[tuple[str, str], ...] = (
    ("k8s.contrib.crd", "packages/k8s.contrib.crd/tests/fixtures/crds.yaml"),
    ("org.json_schema.contrib", "packages/org.json_schema.contrib/tests/fixtures/test_allOf.json"),
)

# Representative exit-0 cold-cache HTTPS mirror verification modules spanning standalone,
# local-dep-packaged (org.json_schema -> pkl.experimental.uri), and remote-k8s-dep
# (k8s.contrib.appEnvCluster -> pkl-k8s/k8s@1.0.1) packages.
COLD_CACHE_MIRROR_TARGETS: tuple[tuple[str, str], ...] = (
    ("pkl.toml", "toml.pkl"),
    ("pkl.tar", "tar.pkl"),
    ("k8s.contrib.appEnvCluster", "AppEnvCluster.pkl"),
    ("com.github.dependabot", "v2/Dependabot.pkl"),
    ("org.json_schema", "JsonSchema.pkl"),
)


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def load_pantry_remote_lock_expectations(root: Path = ROOT) -> dict[str, str]:
    """Return `{package_uri: expected_metadata_sha256}` from compatibility/corpus.json."""
    corpus = json.loads((root / CORPUS_PATH).read_bytes())
    remote: dict[str, str] = {}
    for lock in corpus.get("dependency_lockfiles", []):
        for dep in lock.get("resolved_dependencies", {}).values():
            if dep.get("type") == "remote":
                uri = dep["uri"].replace("projectpackage://", "package://", 1)
                expected = dep["checksums"]["sha256"]
                if uri in remote and remote[uri] != expected:
                    raise ValueError(f"conflicting remote lock checksum for {uri}")
                remote[uri] = expected
    return dict(sorted(remote.items()))


def load_archive_remote_packages(root: Path = ROOT) -> dict[str, dict]:
    """Read and cryptographically verify all remote Pantry deps from pantry-evidence.tar.gz.

    Verifies:
      1. Every lockfile remote dependency from compatibility/corpus.json is present.
      2. Metadata JSON SHA-256 matches lockfile `checksums.sha256`.
      3. Archive ZIP SHA-256 matches `metadata.packageZipChecksums.sha256`.
      4. Archive ZIP is a valid ZIP archive.
    Returns `{package_uri: {metadata_bytes, metadata_sha256, zip_bytes, zip_sha256, metadata}}`.
    """
    archive_path = root / PANTRY_EVIDENCE_ARCHIVE
    if not archive_path.is_file():
        raise ValueError(f"missing Pantry offline evidence archive: {archive_path}")

    expected_locks = load_pantry_remote_lock_expectations(root)
    if not expected_locks:
        raise ValueError("no remote dependency lockfiles found in compatibility/corpus.json")

    packages: dict[str, dict] = {}
    with tarfile.open(archive_path, "r:gz") as tf:
        evidence_member = tf.extractfile("EVIDENCE.json")
        if evidence_member is None:
            raise ValueError("pantry-evidence.tar.gz missing EVIDENCE.json")
        evidence = json.loads(evidence_member.read())
        evidence_by_path = {entry["path"]: entry["sha256"] for entry in evidence.get("files", [])}

        # Map package URIs to cache paths inside the archive
        for uri, expected_meta_sha in expected_locks.items():
            parsed = urlsplit(uri)
            if parsed.scheme != "package" or not parsed.netloc or not parsed.path:
                raise ValueError(f"invalid remote package URI: {uri}")
            rel_base = f"cache/package-2/{parsed.netloc}{parsed.path}"
            leaf = parsed.path.rsplit("/", 1)[1]
            meta_member_name = f"{rel_base}/{leaf}.json"
            zip_member_name = f"{rel_base}/{leaf}.zip"

            meta_f = tf.extractfile(meta_member_name)
            zip_f = tf.extractfile(zip_member_name)
            if meta_f is None or zip_f is None:
                raise ValueError(f"remote package missing from pantry-evidence.tar.gz: {uri}")
            meta_bytes = meta_f.read()
            zip_bytes = zip_f.read()

            meta_sha = sha256_bytes(meta_bytes)
            zip_sha = sha256_bytes(zip_bytes)
            if evidence_by_path.get(meta_member_name) != meta_sha:
                raise ValueError(f"EVIDENCE.json checksum mismatch for {meta_member_name}")
            if evidence_by_path.get(zip_member_name) != zip_sha:
                raise ValueError(f"EVIDENCE.json checksum mismatch for {zip_member_name}")
            if meta_sha != expected_meta_sha:
                raise ValueError(
                    f"remote metadata checksum mismatch for {uri}: expected {expected_meta_sha}, got {meta_sha}"
                )
            metadata = json.loads(meta_bytes)
            if metadata.get("packageUri") != uri:
                raise ValueError(f"metadata packageUri mismatch for {uri}: {metadata.get('packageUri')}")
            if metadata.get("packageZipChecksums", {}).get("sha256") != zip_sha:
                raise ValueError(f"remote ZIP checksum mismatch for {uri}")

            packages[uri] = {
                "uri": uri,
                "netloc": parsed.netloc,
                "path": parsed.path,
                "leaf": leaf,
                "metadata": metadata,
                "metadata_bytes": meta_bytes,
                "metadata_sha256": meta_sha,
                "zip_bytes": zip_bytes,
                "zip_sha256": zip_sha,
            }
    return packages


def seed_remote_cache(cache_dir: Path, root: Path = ROOT) -> dict[str, dict]:
    """Populate `<cache_dir>/package-2/...` with all verified remote Pantry dependencies."""
    packages = load_archive_remote_packages(root)
    summary: dict[str, dict] = {}
    for uri, pkg in packages.items():
        target_dir = cache_dir / "package-2" / pkg["netloc"] / pkg["path"].lstrip("/")
        target_dir.mkdir(parents=True, exist_ok=True)
        meta_path = target_dir / f"{pkg['leaf']}.json"
        zip_path = target_dir / f"{pkg['leaf']}.zip"
        meta_path.write_bytes(pkg["metadata_bytes"])
        zip_path.write_bytes(pkg["zip_bytes"])
        summary[uri] = {
            "metadata_sha256": pkg["metadata_sha256"],
            "zip_sha256": pkg["zip_sha256"],
            "metadata_path": str(meta_path),
            "zip_path": str(zip_path),
        }
    return summary


def seed_mirror_root(mirror_root: Path, generated_packages_dir: Path | None = None, root: Path = ROOT) -> dict:
    """Populate `<mirror_root>` for M4HttpFixtureServer `--http-rewrite` offline HTTPS serving.

    Layout served by M4HttpFixtureServer:
      - `pkg.pkl-lang.org/<ns>/<leaf>/<leaf>.json` (matches `https://pkg.pkl-lang.org/<ns>/<leaf>`)
      - `releases/<repo>/<leaf>/<leaf>.zip` (matches `https://github.com/apple/<repo>/releases/download/<leaf>/<leaf>.zip`)
    """
    mirror_root.mkdir(parents=True, exist_ok=True)
    seeded: dict[str, dict] = {}

    def _place_package(meta_bytes: bytes, zip_bytes: bytes) -> None:
        meta = json.loads(meta_bytes)
        uri = meta["packageUri"]
        zip_url = meta["packageZipUrl"]
        parsed_uri = urlsplit(uri)
        parsed_zip = urlsplit(zip_url)
        leaf = parsed_uri.path.rsplit("/", 1)[1]

        meta_dest = mirror_root / parsed_uri.netloc / parsed_uri.path.lstrip("/") / f"{leaf}.json"
        meta_dest.parent.mkdir(parents=True, exist_ok=True)
        meta_dest.write_bytes(meta_bytes)

        # Map `https://github.com/apple/<repo>/releases/download/<leaf>/<leaf>.zip`
        # into `<mirror_root>/releases/<repo>/<leaf>/<leaf>.zip`
        parts = [p for p in parsed_zip.path.split("/") if p]
        if parsed_zip.netloc == "github.com" and len(parts) >= 6 and parts[0] == "apple" and parts[2] == "releases" and parts[3] == "download":
            repo_name = parts[1]
            zip_dest = mirror_root / "releases" / repo_name / "/".join(parts[4:])
            zip_dest.parent.mkdir(parents=True, exist_ok=True)
            zip_dest.write_bytes(zip_bytes)
        else:
            zip_dest = mirror_root / parsed_zip.netloc / parsed_zip.path.lstrip("/")
            zip_dest.parent.mkdir(parents=True, exist_ok=True)
            zip_dest.write_bytes(zip_bytes)

        seeded[uri] = {
            "metadata_sha256": sha256_bytes(meta_bytes),
            "zip_sha256": sha256_bytes(zip_bytes),
            "zip_url": zip_url,
        }

    for pkg in load_archive_remote_packages(root).values():
        _place_package(pkg["metadata_bytes"], pkg["zip_bytes"])

    if generated_packages_dir is not None and generated_packages_dir.is_dir():
        for meta_file in sorted(generated_packages_dir.rglob("*")):
            if not meta_file.is_file() or meta_file.suffix in (".sha256", ".zip"):
                continue
            zip_file = meta_file.with_name(meta_file.name + ".zip")
            if not zip_file.is_file():
                continue
            _place_package(meta_file.read_bytes(), zip_file.read_bytes())

    return seeded


def seed_generated_package_into_cache(metadata_path: Path, cache_dir: Path) -> dict:
    """Verify a `pkl project package` output bundle and copy it into `package-2/` cache."""
    meta_bytes = metadata_path.read_bytes()
    metadata = json.loads(meta_bytes)
    archive = metadata_path.with_name(metadata_path.name + ".zip")
    meta_sha_file = metadata_path.with_name(metadata_path.name + ".sha256")
    zip_sha_file = archive.with_name(archive.name + ".sha256")

    actual_meta_sha = sha256_bytes(meta_bytes)
    actual_zip_sha = sha256_file(archive)
    if meta_sha_file.read_text(encoding="utf-8") != actual_meta_sha:
        raise ValueError(f"metadata .sha256 mismatch: {metadata_path}")
    if zip_sha_file.read_text(encoding="utf-8") != actual_zip_sha:
        raise ValueError(f"archive .zip.sha256 mismatch: {archive}")
    if metadata.get("packageZipChecksums", {}).get("sha256") != actual_zip_sha:
        raise ValueError(f"metadata packageZipChecksums.sha256 mismatch: {metadata_path}")

    uri = metadata["packageUri"]
    parsed = urlsplit(uri)
    if parsed.scheme != "package" or parsed.netloc != "pkg.pkl-lang.org" or not parsed.path.startswith("/pkl-pantry/"):
        raise ValueError(f"unexpected Pantry package URI: {uri}")
    if any(part in (".", "..") for part in parsed.path.split("/")):
        raise ValueError(f"unsafe package cache path: {uri}")

    target_dir = cache_dir / "package-2" / parsed.netloc / parsed.path.lstrip("/")
    target_dir.mkdir(parents=True, exist_ok=True)
    leaf = parsed.path.rsplit("/", 1)[1]
    for src, dst in ((metadata_path, target_dir / f"{leaf}.json"), (archive, target_dir / f"{leaf}.zip")):
        if dst.exists():
            if sha256_file(dst) != sha256_file(src):
                raise ValueError(f"conflicting generated package artifact: {dst}")
        else:
            shutil.copyfile(src, dst)

    with zipfile.ZipFile(archive) as zf:
        modules = sorted(name for name in zf.namelist() if name.endswith(".pkl"))

    return {
        "uri": uri,
        "name": metadata["name"],
        "version": metadata["version"],
        "metadata_sha256": actual_meta_sha,
        "archive_sha256": actual_zip_sha,
        "modules": [f"{uri}#/{m}" for m in modules],
    }


def enumerate_pantry_dev_cases(root: Path = ROOT) -> dict:
    """Enumerate all 125 tracked Pantry dev-tree cases + 1 tar-shell suite from corpus.json."""
    corpus = json.loads((root / CORPUS_PATH).read_bytes())
    pantry_cases = [c for c in corpus["cases"] if c["source"]["repository"] == "pkl-pantry"]
    by_suite = {
        "package-api-tests": [c for c in pantry_cases if c["suite"] == "package-api-tests"],
        "project-test": [c for c in pantry_cases if c["suite"] == "project-test"],
        "module-names": [c for c in pantry_cases if c["suite"] == "module-names"],
        "example": [c for c in pantry_cases if c["suite"] == "example"],
    }
    shell_suites = [s for s in corpus["suites"] if s["kind"] == "shell-integration-suite"]
    if (
        len(by_suite["package-api-tests"]) != 29
        or len(by_suite["project-test"]) != 44
        or len(by_suite["module-names"]) != 1
        or len(by_suite["example"]) != 51
        or len(shell_suites) != 1
    ):
        raise ValueError(
            "unexpected Pantry dev corpus counts: "
            + json.dumps({k: len(v) for k, v in by_suite.items()}, sort_keys=True)
        )
    return {
        "by_suite": by_suite,
        "all_cases": pantry_cases,
        "shell_suites": shell_suites,
        "external_inputs": corpus.get("external_inputs", []),
    }


def enumerate_m0_packaged_module_census(root: Path = ROOT) -> dict:
    """Read the deterministic list of 206 packaged Pantry module URIs from pantry-evidence.tar.gz."""
    archive_path = root / PANTRY_EVIDENCE_ARCHIVE
    with tarfile.open(archive_path, "r:gz") as tf:
        rep = json.loads(tf.extractfile("report.json").read())
    packages = sorted(
        {p["uri"]: sorted(p["modules"]) for p in rep.get("generated_packages", [])}.items()
    )
    all_modules = sorted(u for _, mods in packages for u in mods)
    multi_file_uris = sorted(r["module_uri"] for r in rep.get("packaged_multi_file_evaluations", []))
    return {
        "package_count": len(packages),
        "module_count": len(all_modules),
        "packages": dict(packages),
        "modules": all_modules,
        "multi_file_uris": multi_file_uris,
        "configured_generators": list(CONFIGURED_GENERATORS),
        "cold_cache_mirror_targets": list(COLD_CACHE_MIRROR_TARGETS),
    }


def seed_structured_read_fixtures(fixtures_root: Path, root: Path = ROOT) -> dict[str, str]:
    """Stage & verify the 2 pinned `raw.githubusercontent.com` blobs for `pkl.experimental.structuredRead`.

    Decoded directly from the committed `structuredRead.pkl-expected.pcf` golden and verified
    against the pinned SHA-256 digests matching commit `8fcea0e535d2eb473545305d8a573144f2b0d33d`,
    making `pantry-dev` 100% offline with zero internet access.
    """
    pcf_path = root / STRUCTURED_READ_EXPECTED_PCF
    if not pcf_path.is_file():
        raise ValueError(f"missing structuredRead expected PCF golden: {pcf_path}")
    text = pcf_path.read_text(encoding="utf-8")
    matches = re.findall(
        r'uri = "https://raw\.githubusercontent\.com/([^"]+)"[\s\S]*?base64 = "([A-Za-z0-9+/=]+)"',
        text,
    )
    found: dict[str, bytes] = {rel: base64.b64decode(b64) for rel, b64 in matches}
    seeded: dict[str, str] = {}
    for rel, expected_sha in STRUCTURED_READ_PINNED_SHA256.items():
        if rel not in found:
            raise ValueError(f"missing pinned structuredRead fixture in PCF golden: {rel}")
        data = found[rel]
        actual_sha = sha256_bytes(data)
        if actual_sha != expected_sha:
            raise ValueError(f"structuredRead fixture SHA-256 mismatch for {rel}: expected {expected_sha}, got {actual_sha}")
        dest = fixtures_root / "raw-gh" / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
        seeded[rel] = actual_sha
    return seeded


def raw_github_rewrite_args(https_port: int, ca_path: Path) -> list[str]:
    """Return `--ca-certificates` + `--http-rewrite` flags routing `raw.githubusercontent.com` to localhost HTTPS."""
    return [
        "--ca-certificates", str(ca_path),
        "--http-rewrite", f"https://raw.githubusercontent.com/=https://localhost:{https_port}/raw-gh/",
    ]


def http_rewrite_args(https_port: int) -> list[str]:
    """Return `--http-rewrite` CLI flags routing Pantry & k8s registries to localhost HTTPS."""
    base = f"https://localhost:{https_port}"
    return [
        "--http-rewrite", f"https://pkg.pkl-lang.org/={base}/pkg.pkl-lang.org/",
        "--http-rewrite", f"https://github.com/apple/pkl-pantry/releases/download/={base}/releases/pkl-pantry/",
        "--http-rewrite", f"https://github.com/apple/pkl-k8s/releases/download/={base}/releases/pkl-k8s/",
        "--http-rewrite", f"https://github.com/apple/pkl-project-commons/releases/download/={base}/releases/pkl-project-commons/",
    ]


if __name__ == "__main__":
    remotes = load_archive_remote_packages(ROOT)
    dev = enumerate_pantry_dev_cases(ROOT)
    pkg = enumerate_m0_packaged_module_census(ROOT)
    print(
        json.dumps(
            {
                "verified_remote_packages": len(remotes),
                "pantry_dev_cases": len(dev["all_cases"]),
                "pantry_shell_suites": len(dev["shell_suites"]),
                "pantry_packaged_packages": pkg["package_count"],
                "pantry_packaged_modules": pkg["module_count"],
                "pantry_packaged_multi_file": len(pkg["multi_file_uris"]),
                "pantry_configured_generators": len(pkg["configured_generators"]),
                "pantry_cold_cache_mirror_targets": len(pkg["cold_cache_mirror_targets"]),
            },
            sort_keys=True,
        )
    )
