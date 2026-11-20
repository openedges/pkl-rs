#!/usr/bin/env python3
"""5-lane offline raw-byte Java 0.32.0 differential runner for Milestone M4.

Lanes:
  1. host-io-http-security   : HTTP/HTTPS/CA/redirects/headers/rewrites/proxy/noProxy/security/rootDir/modulepath/analyze/timeout
  2. external-readers        : MessagePack 0x26-0x32 external module & resource readers, PklProject path/cwd/override, lifecycle/orphans
  3. projects-packages-cache : PklProject/settings precedence, project resolve, project package, download-package, corrupt cache
  4. pantry-dev              : All 29 Pantry packages in development tree form (29 package-api-tests, 44 project-test, 1 module-names, 51 examples, 1 pkl.tar shell suite)
  5. pantry-packaged         : All 29 generated Pantry packages in distributed ZIP form (206 package:// modules, 4 -m retries, 2 configured generators, 5 cold-cache HTTPS mirror targets)

Reuses tools/oracle.py (pinned Eclipse Adoptium Temurin 21.0.13.1+2-LTS and
pkl-tools-0.32.0.jar) and tools/compat.py. 100% offline on 127.0.0.1/localhost
with ephemeral ports, zero internet access, and zero Gradle dependency.
Never normalizes stdout/stderr, trims whitespace, or counts Java-only recordings as passes.
"""
from __future__ import annotations

import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import copy
import hashlib
import http.client
import io
import json
import os
from pathlib import Path
import re
import shutil
import signal
import ssl
import subprocess
import sys
import time
import zipfile

import compat
import m4_pantry_offline as pantry_offline
import oracle

ROOT = Path(__file__).resolve().parents[1]
CASES_PATH = Path("tests/fixtures/m4/cases.json")
SERVER_SOURCE = Path("tools/java/M4HttpFixtureServer.java")
READER_BIN = Path("tests/fixtures/m4/readers/m4_ext_reader.py")
STATIC_LANES = ("host-io-http-security", "external-readers", "projects-packages-cache")
PANTRY_LANES = ("pantry-dev", "pantry-packaged")
ALL_LANES = (*STATIC_LANES, *PANTRY_LANES)
DEFAULT_TIMEOUT = 60.0
EMPTY_ZIP_SHA256 = "8739c76e681f900923b900c9df0ef75cf421d39cabb54650c4b9ad19b6a76d85"


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def capture_devnull(command: list[str], cwd: Path, env: dict[str, str], timeout: float) -> dict:
    """Capture process stdout/stderr/exit with stdin=DEVNULL and POSIX process-group cleanup.

    Prevents stdin-reading modules (e.g. Pantry `pkl.pipe`) from blocking on an inherited parent stdin.
    """
    if os.name != "posix":
        raise RuntimeError("Process-tree cleanup is currently supported only on POSIX")
    before = time.perf_counter_ns()
    process = subprocess.Popen(
        command,
        cwd=cwd,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    timed_out = False
    try:
        try:
            stdout, stderr = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            try:
                stdout, stderr = process.communicate(timeout=5)
            except subprocess.TimeoutExpired as err:
                stdout, stderr = err.output or b"", err.stderr or b""
                if process.stdout:
                    process.stdout.close()
                if process.stderr:
                    process.stderr.close()
                process.wait(timeout=5)
    finally:
        if not timed_out:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
    return {
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": process.returncode,
        "files": {},
        "timed_out": timed_out,
        "workspace_changes": {},
        "elapsed_ns": time.perf_counter_ns() - before,
    }


def evidence_paths(directory: Path):
    for current, dirs, files in os.walk(directory, followlinks=False):
        dirs.sort()
        for name in dirs:
            p = Path(current) / name
            if p.is_symlink():
                raise ValueError(f"evidence directory contains symlink: {p}")
        for name in sorted(files):
            p = Path(current) / name
            if p.is_symlink() or not p.is_file():
                raise ValueError(f"evidence must contain regular files: {p}")
            if p != directory / "report.json":
                yield p


# ---------------------------------------------------------------------------
# Owned Offline HTTP + HTTPS (Custom CA) + HTTP/CONNECT Proxy Fixture Server
# ---------------------------------------------------------------------------

class M4FixtureServers:
    """Manages one pinned-JDK M4HttpFixtureServer instance (HTTP, HTTPS+CA, Proxy)."""

    def __init__(
        self,
        oracle_meta: dict,
        fixtures_root: Path,
        packages_root: Path | None,
        work_dir: Path,
    ):
        self.oracle_meta = oracle_meta
        self.fixtures_root = fixtures_root.resolve()
        self.packages_root = packages_root.resolve() if packages_root else None
        self.work_dir = work_dir.resolve()
        self.process: subprocess.Popen | None = None
        self.streams: list = []
        self.http_port: int = 0
        self.https_port: int = 0
        self.proxy_port: int = 0
        self.ca_path: Path = self.work_dir / "localhost.pem"
        self.request_log: Path = self.work_dir / "requests.jsonl"
        self.record: dict = {}

    def __enter__(self) -> "M4FixtureServers":
        self.work_dir.mkdir(parents=True, exist_ok=False)
        try:
            self._start()
            return self
        except BaseException as err:
            self.record["startup_error"] = str(err)
            self.close()
            raise

    def _run_tool(self, cmd: list[str], env: dict[str, str], label: str) -> None:
        proc = subprocess.run(cmd, cwd=self.work_dir, env=env, capture_output=True, timeout=60)
        (self.work_dir / f"{label}.stdout.bin").write_bytes(proc.stdout)
        (self.work_dir / f"{label}.stderr.bin").write_bytes(proc.stderr)
        if proc.returncode != 0:
            raise RuntimeError(
                f"M4 fixture server setup '{label}' failed (rc={proc.returncode}): "
                f"{proc.stderr.decode('utf-8', errors='replace')}"
            )

    def _start(self) -> None:
        java_home = Path(self.oracle_meta["jdk"]["properties"]["java.home"])
        java_bin = java_home / "bin/java"
        if not java_bin.is_file():
            java_bin = Path(self.oracle_meta["java"])
        javac_bin = java_home / "bin/javac"
        keytool_bin = java_home / "bin/keytool"

        env = dict(self.oracle_meta["environment"])
        tmp = self.work_dir / "tmp"
        tmp.mkdir()
        env.update(TMPDIR=str(tmp), TMP=str(tmp), TEMP=str(tmp))

        classes = self.work_dir / "classes"
        classes.mkdir()
        keystore = self.work_dir / "localhost.p12"

        self._run_tool(
            [
                str(keytool_bin),
                "-genkeypair",
                "-keyalg", "RSA",
                "-keysize", "2048",
                "-validity", "30",
                "-alias", "m4_localhost",
                "-keystore", str(keystore),
                "-storetype", "PKCS12",
                "-storepass", "password",
                "-dname", "CN=localhost",
                "-ext", "SAN=dns:localhost,ip:127.0.0.1",
            ],
            env,
            "keytool-genkey",
        )
        self._run_tool(
            [
                str(keytool_bin),
                "-exportcert",
                "-alias", "m4_localhost",
                "-storepass", "password",
                "-keystore", str(keystore),
                "-rfc",
                "-file", str(self.ca_path),
            ],
            env,
            "keytool-export",
        )
        self._run_tool(
            [
                str(javac_bin),
                "-encoding", "UTF-8",
                "-d", str(classes),
                str(ROOT / SERVER_SOURCE),
            ],
            env,
            "javac-server",
        )

        readiness = self.work_dir / "ready.json"
        cmd = [
            str(java_bin),
            "-Xmx128m",
            "-cp", str(classes),
            "M4HttpFixtureServer",
            str(self.fixtures_root),
            str(self.packages_root) if self.packages_root else "-",
            str(keystore),
            str(readiness),
            str(self.request_log),
        ]
        self.record = {
            "schema_version": 1,
            "server_source_sha256": oracle.sha256(ROOT / SERVER_SOURCE),
            "ca_cert_sha256": oracle.sha256(self.ca_path),
            "command": cmd,
        }
        self.streams = [
            (self.work_dir / "server.stdout.bin").open("wb"),
            (self.work_dir / "server.stderr.bin").open("wb"),
        ]
        self.process = subprocess.Popen(
            cmd,
            cwd=self.work_dir,
            env=env,
            stdin=subprocess.PIPE,
            stdout=self.streams[0],
            stderr=self.streams[1],
            start_new_session=True,
        )
        deadline = time.monotonic() + 20.0
        while not readiness.is_file():
            if self.process.poll() is not None:
                raise RuntimeError("M4HttpFixtureServer exited before readiness")
            if time.monotonic() >= deadline:
                raise RuntimeError("M4HttpFixtureServer readiness timed out")
            time.sleep(0.05)

        ready = json.loads(readiness.read_bytes())
        self.http_port = int(ready["http_port"])
        self.https_port = int(ready["https_port"])
        self.proxy_port = int(ready["proxy_port"])
        self.record["ports"] = {
            "http_port": self.http_port,
            "https_port": self.https_port,
            "proxy_port": self.proxy_port,
        }

    def log_offset(self) -> int:
        return self.request_log.stat().st_size if self.request_log.is_file() else 0

    def read_log_since(self, offset: int) -> list[dict]:
        if not self.request_log.is_file():
            return []
        with self.request_log.open("rb") as fh:
            fh.seek(offset)
            raw = fh.read().decode("utf-8", errors="replace")
        entries = []
        for line in raw.splitlines():
            line = line.strip()
            if line:
                entries.append(json.loads(line))
        return entries

    def self_test(self) -> dict:
        """Directly probe HTTP, HTTPS+custom CA, redirect, proxy plain, proxy CONNECT, and packages."""
        checks: list[dict] = []
        ctx = ssl.create_default_context(cafile=str(self.ca_path))

        # 1. Plain HTTP 200
        conn = http.client.HTTPConnection("127.0.0.1", self.http_port, timeout=10)
        try:
            conn.request("GET", "/modules/hello.pkl")
            resp = conn.getresponse()
            body = resp.read()
            checks.append({"name": "http-200", "status": resp.status, "passed": resp.status == 200 and b"hello-http" in body})
        finally:
            conn.close()

        # 2. HTTP Redirect 301
        conn = http.client.HTTPConnection("127.0.0.1", self.http_port, timeout=10)
        try:
            conn.request("GET", "/redirect/301/modules/redirect_target.pkl")
            resp = conn.getresponse()
            resp.read()
            checks.append({
                "name": "http-301",
                "status": resp.status,
                "location": resp.getheader("Location"),
                "passed": resp.status == 301 and resp.getheader("Location") == "/modules/redirect_target.pkl",
            })
        finally:
            conn.close()

        # 3. HTTPS 200 with custom CA
        sconn = http.client.HTTPSConnection("localhost", self.https_port, context=ctx, timeout=10)
        try:
            sconn.request("GET", "/modules/https_only.pkl")
            resp = sconn.getresponse()
            body = resp.read()
            checks.append({"name": "https-custom-ca-200", "status": resp.status, "passed": resp.status == 200 and b"verified" in body})
        finally:
            sconn.close()

        # 4. HTTPS->HTTP downgrade route emits 308 -> http://
        sconn = http.client.HTTPSConnection("localhost", self.https_port, context=ctx, timeout=10)
        try:
            sconn.request("GET", "/redirect-to-http/modules/hello.pkl")
            resp = sconn.getresponse()
            resp.read()
            loc = resp.getheader("Location") or ""
            checks.append({"name": "https-downgrade-route", "status": resp.status, "passed": resp.status == 308 and loc.startswith("http://")})
        finally:
            sconn.close()

        # 5. Plain HTTP forward proxy to unresolvable host
        pconn = http.client.HTTPConnection("127.0.0.1", self.proxy_port, timeout=10)
        try:
            pconn.request("GET", "http://not.a.valid.host/bar.pkl")
            resp = pconn.getresponse()
            body = resp.read()
            checks.append({"name": "proxy-plain-http", "status": resp.status, "passed": resp.status == 200 and b"via-forward-proxy" in body})
        finally:
            pconn.close()

        # 6. HTTPS CONNECT tunnel through proxy
        tconn = http.client.HTTPSConnection("127.0.0.1", self.proxy_port, context=ctx, timeout=10)
        try:
            tconn.set_tunnel("localhost", self.https_port)
            tconn.request("GET", "/modules/https_only.pkl")
            resp = tconn.getresponse()
            body = resp.read()
            checks.append({"name": "proxy-https-connect", "status": resp.status, "passed": resp.status == 200 and b"verified" in body})
        finally:
            tconn.close()

        # 7. Package route (when packages_root provided)
        if self.packages_root is not None and (self.packages_root / "alpha@1.0.0/alpha@1.0.0.json").is_file():
            sconn = http.client.HTTPSConnection("localhost", self.https_port, context=ctx, timeout=10)
            try:
                sconn.request("GET", "/alpha@1.0.0")
                resp = sconn.getresponse()
                body = resp.read()
                checks.append({"name": "https-package-meta", "status": resp.status, "passed": resp.status == 200 and b"alpha@1.0.0" in body})
            finally:
                sconn.close()

        return {"passed": all(c["passed"] for c in checks), "checks": checks}

    def close(self) -> None:
        if self.process is not None:
            try:
                if self.process.stdin:
                    self.process.stdin.close()
                self.process.wait(timeout=5)
            except (BrokenPipeError, subprocess.TimeoutExpired):
                self.record["forced_shutdown"] = True
                try:
                    os.killpg(self.process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                self.process.wait(timeout=5)
            self.record["exit_code"] = self.process.returncode
            self.process = None
        for s in self.streams:
            s.close()
        self.streams = []

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.close()


# ---------------------------------------------------------------------------
# Package-Archive Honest Structural + Cross-Interop Verification (#150)
# ---------------------------------------------------------------------------

def inspect_zip_deterministic(zip_bytes: bytes) -> dict:
    """Parse ZIP and verify CRC-32, fixed DOS mtime 1980-02-01, no dir entries, and exact contents."""
    entries = []
    with zipfile.ZipFile(io.BytesIO(zip_bytes), "r") as zf:
        bad = zf.testzip()
        if bad is not None:
            raise ValueError(f"corrupt ZIP member CRC: {bad}")
        for info in zf.infolist():
            if info.is_dir():
                raise ValueError(f"unexpected directory entry in package ZIP: {info.filename}")
            if info.date_time != (1980, 2, 1, 0, 0, 0):
                raise ValueError(f"non-deterministic DOS timestamp in {info.filename}: {info.date_time}")
            data = zf.read(info.filename)
            entries.append({
                "name": info.filename,
                "size": len(data),
                "sha256": digest(data),
            })
    return {
        "zip_sha256": digest(zip_bytes),
        "entry_count": len(entries),
        "entries": entries,
    }


def verify_package_bundles(files: dict[str, bytes], expect_empty_zip_sha256: str | None = None) -> dict[str, dict]:
    """Validate every 4-file package bundle inside a captured file map."""
    zips = sorted(k for k in files if k.endswith(".zip"))
    if not zips:
        raise ValueError("package-archive output contains no .zip files")
    bundles: dict[str, dict] = {}
    accounted: set[str] = set()
    for zip_key in zips:
        meta_key = zip_key[:-4]
        meta_sha_key = meta_key + ".sha256"
        zip_sha_key = zip_key + ".sha256"
        for req in (meta_key, meta_sha_key, zip_key, zip_sha_key):
            if req not in files:
                raise ValueError(f"missing package bundle artifact: {req}")
            accounted.add(req)
        meta_bytes = files[meta_key]
        zip_bytes = files[zip_key]
        meta_sha = digest(meta_bytes)
        zip_sha = digest(zip_bytes)
        if files[meta_sha_key] != meta_sha.encode("utf-8"):
            raise ValueError(f"metadata .sha256 sidecar mismatch: {meta_sha_key}")
        if files[zip_sha_key] != zip_sha.encode("utf-8"):
            raise ValueError(f"archive .zip.sha256 sidecar mismatch: {zip_sha_key}")
        if meta_bytes.endswith(b"\n"):
            raise ValueError(f"metadata JSON has unexpected trailing newline: {meta_key}")
        meta = json.loads(meta_bytes)
        if meta.get("packageZipChecksums", {}).get("sha256") != zip_sha:
            raise ValueError(f"metadata packageZipChecksums.sha256 mismatch: {meta_key}")
        zip_info = inspect_zip_deterministic(zip_bytes)
        if zip_info["entry_count"] == 0 and zip_sha != EMPTY_ZIP_SHA256:
            raise ValueError(f"empty ZIP SHA-256 mismatch: expected {EMPTY_ZIP_SHA256}, got {zip_sha}")
        if expect_empty_zip_sha256 and zip_sha != expect_empty_zip_sha256:
            raise ValueError(f"expected empty ZIP SHA-256 {expect_empty_zip_sha256}, got {zip_sha}")
        bundles[meta_key] = {
            "meta_key": meta_key,
            "zip_key": zip_key,
            "meta": meta,
            "meta_bytes": meta_bytes,
            "meta_sha256": meta_sha,
            "zip_sha256": zip_sha,
            "zip_info": zip_info,
            "package_uri": meta["packageUri"],
        }
    extra = sorted(set(files) - accounted)
    if extra:
        raise ValueError(f"unexpected extra package output files: {extra}")
    # Verify internal checksum consistency for every co-packaged local dependency within this bundle set.
    by_uri = {b["package_uri"]: b for b in bundles.values()}
    for meta_key, bundle in bundles.items():
        deps = bundle["meta"].get("dependencies")
        if not isinstance(deps, dict):
            continue
        for dep_name, dep_obj in deps.items():
            if not isinstance(dep_obj, dict):
                continue
            dep_uri = dep_obj.get("uri")
            if dep_uri in by_uri:
                recorded_sha = (dep_obj.get("checksums") or {}).get("sha256")
                actual_dep_sha = by_uri[dep_uri]["meta_sha256"]
                if recorded_sha != actual_dep_sha:
                    raise ValueError(
                        f"co-packaged local dependency checksum mismatch in {meta_key} "
                        f"for {dep_name} ({dep_uri}): recorded {recorded_sha} != {actual_dep_sha}"
                    )
    return bundles


def compare_package_archives(
    expected_files: dict[str, bytes],
    actual_files: dict[str, bytes],
    assertions: dict | None = None,
) -> tuple[list[str], dict]:
    """Compare Java vs Rust `pkl project package` bundles honestly without hiding DEFLATE diffs."""
    expect_empty = (assertions or {}).get("expect_empty_zip_sha256")
    diffs: list[str] = []
    meta_info: dict = {"raw_zip_bytes_match": True, "bundles": {}}
    if sorted(expected_files) != sorted(actual_files):
        return ["files/path-set"], {"raw_zip_bytes_match": False}
    try:
        exp_bundles = verify_package_bundles(expected_files, expect_empty_zip_sha256=expect_empty)
    except ValueError as err:
        return [f"oracle-bundle-invalid:{err}"], {"raw_zip_bytes_match": False}
    try:
        act_bundles = verify_package_bundles(actual_files, expect_empty_zip_sha256=expect_empty)
    except ValueError as err:
        return [f"candidate-bundle-invalid:{err}"], {"raw_zip_bytes_match": False}

    exp_by_uri = {eb["package_uri"]: eb for eb in exp_bundles.values()}
    act_by_uri = {ab["package_uri"]: ab for ab in act_bundles.values()}

    for meta_key in sorted(exp_bundles):
        eb = exp_bundles[meta_key]
        ab = act_bundles[meta_key]
        raw_zip_same = eb["zip_sha256"] == ab["zip_sha256"]
        if not raw_zip_same:
            meta_info["raw_zip_bytes_match"] = False
        if eb["zip_info"]["entries"] != ab["zip_info"]["entries"]:
            diffs.append(f"files/{eb['zip_key']}:entries")

        # Build an explicit, field-scoped alignment of:
        #   1. this bundle's verified self-consistent packageZipChecksums.sha256
        #   2. verified co-packaged local dependency metadata SHA-256s (NEVER remote deps!)
        aligned_meta_obj = copy.deepcopy(ab["meta"])
        byte_replacements: dict[bytes, bytes] = {}
        aligned_local_deps: dict[str, dict[str, str]] = {}

        if ab["zip_sha256"] != eb["zip_sha256"]:
            if isinstance(aligned_meta_obj.get("packageZipChecksums"), dict):
                aligned_meta_obj["packageZipChecksums"]["sha256"] = eb["zip_sha256"]
            byte_replacements[ab["zip_sha256"].encode("utf-8")] = eb["zip_sha256"].encode("utf-8")

        act_deps = aligned_meta_obj.get("dependencies")
        exp_deps = eb["meta"].get("dependencies")
        if isinstance(act_deps, dict) and isinstance(exp_deps, dict):
            for dep_name, dep_obj in act_deps.items():
                if not isinstance(dep_obj, dict):
                    continue
                dep_uri = dep_obj.get("uri")
                if dep_uri not in act_by_uri or dep_uri not in exp_by_uri:
                    continue
                exp_dep_obj = exp_deps.get(dep_name)
                if not isinstance(exp_dep_obj, dict) or exp_dep_obj.get("uri") != dep_uri:
                    continue
                act_dep_sha = act_by_uri[dep_uri]["meta_sha256"]
                exp_dep_sha = exp_by_uri[dep_uri]["meta_sha256"]
                if (
                    (dep_obj.get("checksums") or {}).get("sha256") == act_dep_sha
                    and (exp_dep_obj.get("checksums") or {}).get("sha256") == exp_dep_sha
                    and act_dep_sha != exp_dep_sha
                ):
                    dep_obj["checksums"]["sha256"] = exp_dep_sha
                    byte_replacements[act_dep_sha.encode("utf-8")] = exp_dep_sha.encode("utf-8")
                    aligned_local_deps[dep_name] = {
                        "uri": dep_uri,
                        "java_meta_sha256": exp_dep_sha,
                        "rust_meta_sha256": act_dep_sha,
                    }

        # Verify BOTH exact JSON semantics (field-scoped, so remote deps can never be rewritten)
        # AND exact byte-for-byte JSON formatting (key order, indentation, escaping, no trailing newline).
        if byte_replacements:
            pattern = re.compile(b"|".join(re.escape(k) for k in sorted(byte_replacements, key=len, reverse=True)))
            aligned_actual_meta = pattern.sub(lambda m: byte_replacements[m.group(0)], ab["meta_bytes"])
        else:
            aligned_actual_meta = ab["meta_bytes"]

        meta_ok = aligned_meta_obj == eb["meta"] and aligned_actual_meta == eb["meta_bytes"]
        if not meta_ok:
            diffs.append(f"files/{meta_key}:metadata-json")
        meta_info["bundles"][meta_key] = {
            "package_uri": eb["package_uri"],
            "java_zip_sha256": eb["zip_sha256"],
            "rust_zip_sha256": ab["zip_sha256"],
            "raw_zip_bytes_match": raw_zip_same,
            "entries_match": eb["zip_info"]["entries"] == ab["zip_info"]["entries"],
            "metadata_structure_and_formatting_match": meta_ok,
            "aligned_local_dep_shas": aligned_local_deps,
        }
    return diffs, meta_info


def verify_java_cross_interop(
    oracle_meta: dict,
    java_files: dict[str, bytes],
    rust_files: dict[str, bytes],
    module_uris: list[str],
    scratch: Path,
    timeout: float,
) -> tuple[bool, dict]:
    """Verify pinned Java 0.32.0 loads & evaluates Rust-built package ZIP+JSON identically to Java-built."""
    details: dict = {"modules": {}}
    all_ok = True
    for side, files in (("java_pkg", java_files), ("rust_pkg", rust_files)):
        cache = scratch / f"interop_{side}" / "cache"
        cache.mkdir(parents=True, exist_ok=True)
        bundles = verify_package_bundles(files)
        for b in bundles.values():
            uri = b["package_uri"]
            # package://localhost:0/samplepkg@1.0.0 -> package-2/localhost(3a)0/samplepkg@1.0.0/...
            rest = uri.removeprefix("package://")
            auth, path = rest.split("/", 1)
            enc_auth = auth.replace("(", "((").replace(":", "(3a)")
            leaf = path.rsplit("/", 1)[-1]
            dest = cache / "package-2" / enc_auth / path
            dest.mkdir(parents=True, exist_ok=True)
            (dest / f"{leaf}.json").write_bytes(files[b["meta_key"]])
            (dest / f"{leaf}.zip").write_bytes(files[b["zip_key"]])
        for idx, mod_uri in enumerate(module_uris):
            state = scratch / f"interop_{side}" / f"state_{idx}"
            env = build_case_env(oracle_meta["environment"], {}, state)
            cmd = [
                *java_prefix_for_state(oracle_meta["command"], state),
                "eval", "--no-project", "--settings", "pkl:settings",
                "--cache-dir", str(cache), mod_uri,
            ]
            cap = capture_devnull(cmd, ROOT, env, timeout)
            details["modules"].setdefault(mod_uri, {})[side] = {
                "exit_code": cap["exit_code"],
                "stdout_sha256": digest(cap["stdout"]),
                "stderr_sha256": digest(cap["stderr"]),
            }
    for mod_uri, pair in details["modules"].items():
        j, r = pair["java_pkg"], pair["rust_pkg"]
        ok = j["exit_code"] == 0 and j == r
        pair["match"] = ok
        if not ok:
            all_ok = False
    return all_ok, details


# ---------------------------------------------------------------------------
# Environment, Setup, File Collection, Assertions & Classification
# ---------------------------------------------------------------------------

def build_case_env(
    base_env: dict[str, str],
    case_env: dict[str, str],
    state_dir: Path,
    *,
    prepend_path_dir: Path | None = None,
    java_version: str | None = None,
) -> dict[str, str]:
    home = state_dir / "home"
    tmp = state_dir / "tmp"
    home.mkdir(parents=True, exist_ok=True)
    tmp.mkdir(parents=True, exist_ok=True)
    env = {
        **base_env,
        **case_env,
        "HOME": str(home),
        "TMPDIR": str(tmp),
        "TMP": str(tmp),
        "TEMP": str(tmp),
    }
    if prepend_path_dir is not None:
        env["PATH"] = str(prepend_path_dir) + os.pathsep + env.get("PATH", os.defpath)
    if java_version:
        env["PKL_ORACLE_JAVA_VERSION"] = java_version
    return env


def java_prefix_for_state(prefix: list[str], state_dir: Path) -> list[str]:
    home = state_dir / "home"
    tmp = state_dir / "tmp"
    out = []
    for arg in prefix:
        if arg.startswith("-Duser.home="):
            out.append(f"-Duser.home={home}")
        elif arg.startswith("-Djava.io.tmpdir="):
            out.append(f"-Djava.io.tmpdir={tmp}")
        else:
            out.append(arg)
    return out


def expand_placeholders(text: str, mapping: dict[str, str]) -> str:
    out = text
    for key, val in mapping.items():
        out = out.replace("{" + key + "}", val)
    return out


def apply_case_setup(
    root: Path,
    setup: dict | None,
    work_dir: Path,
    cache_dir: Path,
    home_dir: Path,
    placeholders: dict[str, str],
) -> tuple[set[str], dict[str, bytes]]:
    """Stage case workspace/cache/home inputs; return (declared_symlinks, work_dir_baseline)."""
    declared_symlinks: set[str] = set()
    work_dir.mkdir(parents=True, exist_ok=True)
    cache_dir.mkdir(parents=True, exist_ok=True)
    home_dir.mkdir(parents=True, exist_ok=True)
    if setup:
        for src_rel, dst_rel in setup.get("copy_dir", []):
            src = compat.relative_path(root, src_rel)
            dst = compat.relative_path(work_dir, dst_rel)
            shutil.copytree(src, dst, symlinks=True)
        for rel_dir in setup.get("dirs", []):
            compat.relative_path(work_dir, rel_dir).mkdir(parents=True, exist_ok=True)
        for rel_file, tmpl in setup.get("templates", []):
            dst = compat.relative_path(work_dir, rel_file)
            dst.parent.mkdir(parents=True, exist_ok=True)
            dst.write_bytes(expand_placeholders(tmpl, placeholders).encode("utf-8"))
        for link_rel, target in setup.get("symlinks", []):
            link_path = compat.relative_path(work_dir, link_rel)
            link_path.parent.mkdir(parents=True, exist_ok=True)
            link_path.symlink_to(target)
            declared_symlinks.add(Path(link_rel).as_posix())
        for rel_cache, spec in setup.get("seed_cache_files", []):
            dst = compat.relative_path(cache_dir, rel_cache)
            dst.parent.mkdir(parents=True, exist_ok=True)
            if spec.startswith("literal:"):
                dst.write_bytes(spec[len("literal:"):].encode("utf-8"))
            elif spec.startswith("hex:"):
                dst.write_bytes(bytes.fromhex(spec[len("hex:"):]))
            else:
                dst.write_bytes(compat.relative_path(root, spec).read_bytes())
        for rel_home, spec in setup.get("seed_home_files", []):
            dst = compat.relative_path(home_dir, rel_home)
            dst.parent.mkdir(parents=True, exist_ok=True)
            if spec == "{ca_cert}":
                dst.write_bytes(Path(placeholders["ca_cert"]).read_bytes())
            elif spec.startswith("template:"):
                dst.write_bytes(expand_placeholders(spec[len("template:"):], placeholders).encode("utf-8"))
            else:
                dst.write_bytes(compat.relative_path(root, spec).read_bytes())
    baseline = collect_tree_bytes(work_dir, declared_symlinks, prefix="")
    return declared_symlinks, baseline


def collect_tree_bytes(directory: Path, declared_symlinks: set[str], prefix: str = "generated/") -> dict[str, bytes]:
    result: dict[str, bytes] = {}
    if directory.is_symlink():
        raise ValueError(f"Unexpected root symlink: {directory}")
    if not directory.exists():
        return result
    for current, dirs, files in os.walk(directory, followlinks=False):
        cur = Path(current)
        kept_dirs = []
        for d in sorted(dirs):
            p = cur / d
            rel = p.relative_to(directory).as_posix()
            if p.is_symlink():
                if rel not in declared_symlinks:
                    raise ValueError(f"Unexpected output symlink: {p}")
                continue
            kept_dirs.append(d)
        dirs[:] = kept_dirs
        for name in sorted(files):
            p = cur / name
            rel = p.relative_to(directory).as_posix()
            if p.is_symlink():
                if rel not in declared_symlinks:
                    raise ValueError(f"Unexpected output symlink: {p}")
                continue
            if p.is_file():
                result[f"{prefix}{rel}"] = p.read_bytes()
    return dict(sorted(result.items()))


def pid_alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        return True


def check_runtime_assertions(
    assertions: dict | None,
    server_events: list[dict],
    lifecycle_log: Path,
    cache_dir: Path,
    elapsed_ns: int,
    files: dict[str, bytes],
) -> list[str]:
    if not assertions:
        return []
    failures: list[str] = []
    if assertions.get("proxy_hit"):
        if not any(ev.get("server") == "proxy" for ev in server_events):
            failures.append("assertion:proxy_hit_missing")
    if assertions.get("proxy_bypassed"):
        if any(ev.get("server") == "proxy" for ev in server_events):
            failures.append("assertion:proxy_was_not_bypassed")
    if assertions.get("reader_closed") or assertions.get("reader_single_process"):
        if not lifecycle_log.is_file():
            failures.append("assertion:lifecycle_log_missing")
        else:
            events = [json.loads(line) for line in lifecycle_log.read_text(encoding="utf-8").splitlines() if line.strip()]
            starts = [e for e in events if e.get("event") == "start"]
            closes = [e for e in events if e.get("event") == "close"]
            pids = sorted({int(e["pid"]) for e in starts if "pid" in e})
            if not starts:
                failures.append("assertion:reader_never_started")
            if assertions.get("reader_closed") and not closes:
                failures.append("assertion:reader_missing_close_0x32")
            if assertions.get("reader_single_process") and len(pids) != 1:
                failures.append(f"assertion:reader_expected_1_pid_got_{len(pids)}")
            # Brief poll up to 1s for kernel reaping after parent exit
            for pid in pids:
                deadline = time.monotonic() + 1.0
                while pid_alive(pid) and time.monotonic() < deadline:
                    time.sleep(0.05)
                if pid_alive(pid):
                    failures.append(f"assertion:reader_orphan_pid_{pid}")
    if assertions.get("cache_empty"):
        cache_files = [p for p in cache_dir.rglob("*") if p.is_file()]
        if cache_files:
            failures.append(f"assertion:cache_not_empty({len(cache_files)})")
    for rel_del in assertions.get("deleted_cache_paths", []):
        if (cache_dir / rel_del).exists():
            failures.append(f"assertion:corrupt_cache_not_deleted({rel_del})")
    if assertions.get("http_timeout_fast"):
        if elapsed_ns > 5_000_000_000:
            failures.append(f"assertion:http_timeout_too_slow({elapsed_ns / 1e9:.2f}s)")
    if assertions.get("expect_empty_zip_sha256"):
        try:
            verify_package_bundles(files, expect_empty_zip_sha256=assertions["expect_empty_zip_sha256"])
        except ValueError as err:
            failures.append(f"assertion:empty_zip:{err}")
    return failures


def classify_result(
    case: dict,
    expected: dict,
    actual: dict | None,
    *,
    java_assertion_failures: list[str] | None = None,
    rust_assertion_failures: list[str] | None = None,
    package_compare_meta: dict | None = None,
    cross_interop_ok: bool | None = None,
    idempotency_ok: bool | None = None,
) -> tuple[str, list[str]]:
    """Classify one Java vs Rust differential result strictly and honestly."""
    if expected["timed_out"]:
        return "oracle-error", ["oracle_timed_out"]
    if expected["exit_code"] not in case["expected_exit"]:
        return "oracle-contract-mismatch", [f"oracle_exit_{expected['exit_code']}"]
    if java_assertion_failures:
        return "oracle-assertion-failed", list(java_assertion_failures)
    if actual is None:
        return "oracle-recorded", []
    if actual["timed_out"]:
        return "candidate-timeout", ["candidate_timed_out"]
    if actual["exit_code"] in (2, 3, 4):
        kind = {
            2: "candidate-unsupported",
            3: "candidate-limit-exceeded",
            4: "candidate-internal-error",
        }[actual["exit_code"]]
        return kind, [f"candidate_exit_{actual['exit_code']}"]
    if rust_assertion_failures:
        return "mismatch", list(rust_assertion_failures)

    comp_mode = case.get("comparison_mode", "raw")
    if comp_mode == "package-archive" and expected["exit_code"] == 0 and actual["exit_code"] == 0:
        diffs: list[str] = []
        for ch in ("stdout", "stderr", "exit_code"):
            if expected[ch] != actual[ch]:
                diffs.append(ch)
        for ch in ("workspace_changes", "timed_out"):
            if expected.get(ch) != actual.get(ch):
                diffs.append(ch)
        pkg_diffs, meta = compare_package_archives(expected["files"], actual["files"], case.get("assertions"))
        if package_compare_meta is not None:
            package_compare_meta.update(meta)
        diffs.extend(pkg_diffs)
        if idempotency_ok is False:
            diffs.append("package-not-idempotent")
        if cross_interop_ok is False:
            diffs.append("java-cross-interop-failed")
        return ("mismatch", diffs) if diffs else ("pass", [])

    diffs = compat.differences(expected, actual)
    return ("mismatch", diffs) if diffs else ("pass", [])


# ---------------------------------------------------------------------------
# Execution of Static Lanes (1-3)
# ---------------------------------------------------------------------------

def run_static_case(
    root: Path,
    case: dict,
    output: Path,
    oracle_meta: dict,
    servers: M4FixtureServers,
    candidate_bin: Path | None,
    timeout: float,
) -> dict:
    cdir_name = compat.case_directory(case["id"])
    folder = output / "cases" / cdir_name
    folder.mkdir(parents=True)
    work_root = output / "_work" / cdir_name
    work_root.mkdir(parents=True)

    case_state = work_root / "state"
    sandbox = work_root / "sandbox"
    case_gen = sandbox / "generated"
    case_work = sandbox / "work"
    case_cache = sandbox / "cache"
    lifecycle_log = sandbox / "ext_lifecycle.jsonl"
    empty_dir = work_root / "empty"
    empty_dir.mkdir(parents=True, exist_ok=True)

    placeholders = {
        "http_port": str(servers.http_port),
        "https_port": str(servers.https_port),
        "proxy_port": str(servers.proxy_port),
        "ca_cert": str(servers.ca_path),
        "output_dir": str(case_gen),
        "work_dir": str(case_work),
        "cache_dir": str(case_cache),
        "reader_bin": str((root / READER_BIN).resolve()),
        "lifecycle_log": str(lifecycle_log),
        "root": str(root),
    }

    setup = case.get("setup")
    prepend_path = (root / setup["prepend_path_dir"]).resolve() if setup and setup.get("prepend_path_dir") else None
    collect_mode = case.get("collect_files", "none")
    assertions = case.get("assertions")
    java_version = oracle_meta.get("jdk", {}).get("properties", {}).get("java.version")

    def _run_side(is_java: bool, suffix_bin: Path | None) -> tuple[list[str], dict, list[str], list[dict]]:
        shutil.rmtree(case_state, ignore_errors=True)
        shutil.rmtree(sandbox, ignore_errors=True)
        case_gen.mkdir(parents=True)
        env = build_case_env(
            oracle_meta["environment"],
            case.get("environment", {}),
            case_state,
            prepend_path_dir=prepend_path,
            java_version=None if is_java else java_version,
        )
        declared_symlinks, work_baseline = apply_case_setup(
            root, setup, case_work, case_cache, case_state / "home", placeholders
        )
        args = [expand_placeholders(a, placeholders) for a in case["command"]]
        cmd = (
            [*java_prefix_for_state(oracle_meta["command"], case_state), *args]
            if is_java
            else [str(suffix_bin), *args]
        )
        cwd = Path(expand_placeholders(case.get("cwd", "."), placeholders))
        if not cwd.is_absolute():
            cwd = (root / cwd).resolve()

        log_off = servers.log_offset()
        raw = capture_devnull(cmd, cwd, env, timeout)
        server_events = servers.read_log_since(log_off)

        collected: dict[str, bytes] = {}
        if collect_mode == "output_dir":
            collected = collect_tree_bytes(case_gen, set(), prefix="generated/")
        elif collect_mode == "cache_dir":
            collected = collect_tree_bytes(case_cache, set(), prefix="cache/")
        elif collect_mode == "work_dir_changes":
            after = collect_tree_bytes(case_work, declared_symlinks, prefix="")
            for rel, data in after.items():
                if work_baseline.get(rel) != data:
                    collected[f"work/{rel}"] = data
        raw["files"] = dict(sorted(collected.items()))
        raw["workspace_changes"] = {}

        afails = check_runtime_assertions(
            assertions, server_events, lifecycle_log, case_cache, raw["elapsed_ns"], raw["files"]
        )
        return cmd, raw, afails, server_events

    java_cmd, java_raw, java_afails, java_events = _run_side(True, None)
    java_stored = compat.store_capture(folder / "java", java_raw)

    rust_cmd = None
    rust_raw = None
    rust_stored = None
    rust_afails: list[str] = []
    rust_events: list[dict] = []
    idempotency_ok: bool | None = None
    cross_interop_ok: bool | None = None
    cross_interop_detail: dict | None = None

    if candidate_bin is not None:
        rust_cmd, rust_raw, rust_afails, rust_events = _run_side(False, candidate_bin)
        rust_stored = compat.store_capture(folder / "rust", rust_raw)

        # For package-archive mode on success: verify two-run idempotency + Java cross-interop
        if (
            case.get("comparison_mode") == "package-archive"
            and java_raw["exit_code"] == 0
            and rust_raw["exit_code"] == 0
        ):
            _, rust_raw_2, _, _ = _run_side(False, candidate_bin)
            idempotency_ok = rust_raw_2["exit_code"] == 0 and rust_raw_2["files"] == rust_raw["files"]
            interop_mods = (assertions or {}).get("cross_interop_modules", [])
            if interop_mods:
                cross_interop_ok, cross_interop_detail = verify_java_cross_interop(
                    oracle_meta,
                    java_raw["files"],
                    rust_raw["files"],
                    interop_mods,
                    work_root / "interop",
                    timeout,
                )

    pkg_meta: dict = {}
    status, diffs = classify_result(
        case,
        java_raw,
        rust_raw,
        java_assertion_failures=java_afails,
        rust_assertion_failures=rust_afails,
        package_compare_meta=pkg_meta,
        cross_interop_ok=cross_interop_ok,
        idempotency_ok=idempotency_ok,
    )
    shutil.rmtree(work_root, ignore_errors=True)

    return {
        "id": case["id"],
        "lane": case["lane"],
        "directory": cdir_name,
        "status": status,
        "differences": diffs,
        "expected_exit": case["expected_exit"],
        "comparison_mode": case.get("comparison_mode", "raw"),
        "java_assertion_failures": java_afails,
        "rust_assertion_failures": rust_afails,
        "java_server_events_count": len(java_events),
        "rust_server_events_count": len(rust_events),
        "package_archive_meta": pkg_meta or None,
        "idempotency_ok": idempotency_ok,
        "cross_interop_ok": cross_interop_ok,
        "cross_interop_detail": cross_interop_detail,
        "java": {"command": java_cmd, "result": java_stored},
        "rust": None if rust_stored is None else {"command": rust_cmd, "result": rust_stored},
    }


# ---------------------------------------------------------------------------
# Execution of Pantry Lanes (4: pantry-dev, 5: pantry-packaged)
# ---------------------------------------------------------------------------

def run_pantry_lanes(
    root: Path,
    output: Path,
    oracle_meta: dict,
    candidate_bin: Path | None,
    run_dev: bool,
    run_packaged: bool,
    workers: int,
    timeout: float,
) -> tuple[list[dict], list[dict]]:
    """Run offline Pantry dev-tree and/or packaged-form differential lanes.

    Returns (dynamic_case_definitions, result_rows).
    """
    dev_census = pantry_offline.enumerate_pantry_dev_cases(root)
    pantry_work = output / "_pantry_work"
    pantry_work.mkdir(parents=True)
    remote_cache = pantry_work / "remote_cache"
    pantry_offline.seed_remote_cache(remote_cache, root)

    # Shared read-only staged Pantry source copy for parallel read-only evals/tests
    pantry_tree = pantry_work / "pantry_src/external/pkl-pantry"
    shutil.copytree(
        root / "external/pkl-pantry",
        pantry_tree,
        ignore=shutil.ignore_patterns(".git", "build", ".gradle"),
    )
    pantry_cwd = pantry_work / "pantry_src"
    java_version = oracle_meta.get("jdk", {}).get("properties", {}).get("java.version")

    dynamic_cases: list[dict] = []
    results: list[dict] = []
    generated_dir = pantry_work / "generated_packages"
    generated_dir.mkdir()

    def _run_pair(
        case_def: dict,
        args: list[str],
        cwd: Path,
        cache_dir: Path,
        *,
        out_dir: Path | None = None,
        extra_env: dict[str, str] | None = None,
        collect_out: bool = False,
        custom_java_cmd: list[str] | None = None,
        custom_rust_cmd: list[str] | None = None,
        preserve_java_out_to: Path | None = None,
    ) -> dict:
        cdir_name = compat.case_directory(case_def["id"])
        folder = output / "cases" / cdir_name
        folder.mkdir(parents=True)
        scratch = pantry_work / "case_tmp" / cdir_name
        scratch.mkdir(parents=True)
        state = scratch / "state"
        gen = out_dir if out_dir is not None else (scratch / "generated")
        empty = scratch / "empty"
        empty.mkdir()

        def _exec(is_java: bool) -> tuple[list[str], dict]:
            shutil.rmtree(state, ignore_errors=True)
            shutil.rmtree(gen, ignore_errors=True)
            gen.mkdir(parents=True)
            env = build_case_env(
                oracle_meta["environment"],
                extra_env or {},
                state,
                java_version=None if is_java else java_version,
            )
            if is_java:
                cmd = custom_java_cmd if custom_java_cmd is not None else [
                    *java_prefix_for_state(oracle_meta["command"], state),
                    *[a.replace("{output_dir}", str(gen)).replace("{cache_dir}", str(cache_dir)) for a in args],
                ]
            else:
                cmd = custom_rust_cmd if custom_rust_cmd is not None else [
                    str(candidate_bin),
                    *[a.replace("{output_dir}", str(gen)).replace("{cache_dir}", str(cache_dir)) for a in args],
                ]
            raw = capture_devnull(cmd, cwd, env, timeout)
            raw["files"] = collect_tree_bytes(gen, set(), prefix="generated/") if collect_out else {}
            raw["workspace_changes"] = {}
            if is_java and preserve_java_out_to is not None and gen.exists():
                shutil.copytree(gen, preserve_java_out_to, dirs_exist_ok=True)
            return cmd, raw

        j_cmd, j_raw = _exec(True)
        j_stored = compat.store_capture(folder / "java", j_raw)

        r_cmd = None
        r_raw = None
        r_stored = None
        if candidate_bin is not None:
            r_cmd, r_raw = _exec(False)
            r_stored = compat.store_capture(folder / "rust", r_raw)

        pkg_meta: dict = {}
        st, diffs = classify_result(case_def, j_raw, r_raw, package_compare_meta=pkg_meta)
        shutil.rmtree(scratch, ignore_errors=True)
        return {
            "id": case_def["id"],
            "lane": case_def["lane"],
            "directory": cdir_name,
            "status": st,
            "differences": diffs,
            "expected_exit": case_def["expected_exit"],
            "comparison_mode": case_def.get("comparison_mode", "raw"),
            "java_assertion_failures": [],
            "rust_assertion_failures": [],
            "package_archive_meta": pkg_meta or None,
            "java": {"command": j_cmd, "result": j_stored},
            "rust": None if r_stored is None else {"command": r_cmd, "result": r_stored},
        }

    # --- Build the 29 Pantry packages (required for both pantry-dev and pantry-packaged) ---
    # Serve the 2 pinned `raw.githubusercontent.com` blobs needed by `pkl.experimental.structuredRead`
    # locally over HTTPS (`--http-rewrite` + `--ca-certificates`) so Pantry dev & packaged lanes
    # execute 100% offline with zero external network access.
    raw_gh_root = pantry_work / "raw_gh_fixtures"
    pantry_offline.seed_structured_read_fixtures(raw_gh_root, root)
    pkg_api_cases = dev_census["by_suite"]["package-api-tests"]
    with M4FixtureServers(
        oracle_meta,
        raw_gh_root,
        None,
        pantry_work / "raw_gh_server",
    ) as raw_gh_srv:
        raw_gh_flags = pantry_offline.raw_github_rewrite_args(raw_gh_srv.https_port, raw_gh_srv.ca_path)
        if run_dev:
            def _run_pkg_api(item: tuple[int, dict]) -> dict:
                idx, c = item
                cdef = {
                    "id": f"m4:pantry-dev:package-api-tests:{c['source']['path']}",
                    "lane": "pantry-dev",
                    "expected_exit": [0],
                    "comparison_mode": "package-archive",
                    "corpus_case_id": c["id"],
                }
                args = [
                    *c["command"],
                    "--cache-dir", "{cache_dir}",
                    "--allowed-resources", "file:,pkl:,https:,prop:,package:,projectpackage:",
                    *raw_gh_flags,
                ]
                row = _run_pair(
                    cdef,
                    args,
                    pantry_cwd,
                    remote_cache,
                    collect_out=True,
                    preserve_java_out_to=generated_dir / f"pkg_{idx}",
                )
                return cdef, row

            with ThreadPoolExecutor(max_workers=workers) as ex:
                pkg_pairs = list(ex.map(_run_pkg_api, enumerate(pkg_api_cases)))
            for cdef, row in pkg_pairs:
                dynamic_cases.append(cdef)
                results.append(row)

            # 44 project-test + 1 module-names
            test_cases = dev_census["by_suite"]["project-test"] + dev_census["by_suite"]["module-names"]
            def _run_pantry_test(c: dict) -> tuple[dict, dict]:
                cdef = {
                    "id": f"m4:pantry-dev:{c['suite']}:{c['source']['path']}",
                    "lane": "pantry-dev",
                    "expected_exit": [0],
                    "comparison_mode": "raw",
                    "corpus_case_id": c["id"],
                }
                args = [
                    *c["command"],
                    "--cache-dir", "{cache_dir}",
                    "--allowed-resources", "file:,pkl:,https:,prop:,package:,projectpackage:",
                    *raw_gh_flags,
                ]
                return cdef, _run_pair(cdef, args, pantry_cwd, remote_cache)

            with ThreadPoolExecutor(max_workers=workers) as ex:
                test_pairs = list(ex.map(_run_pantry_test, test_cases))
            for cdef, row in test_pairs:
                dynamic_cases.append(cdef)
                results.append(row)

            # 51 example candidates (expected_exit [0, 1] matching corpus.json; raw bytes must still match!)
            ex_cases = dev_census["by_suite"]["example"]
            def _run_pantry_example(c: dict) -> tuple[dict, dict]:
                cdef = {
                    "id": f"m4:pantry-dev:example:{c['source']['path']}",
                    "lane": "pantry-dev",
                    "expected_exit": c["expected_exit"],
                    "comparison_mode": "raw",
                    "corpus_case_id": c["id"],
                }
                args = [
                    *c["command"],
                    "--cache-dir", "{cache_dir}",
                    "--allowed-resources", "file:,pkl:,https:,prop:,package:,projectpackage:",
                    *raw_gh_flags,
                ]
                return cdef, _run_pair(cdef, args, pantry_cwd, remote_cache)

            with ThreadPoolExecutor(max_workers=workers) as ex:
                ex_pairs = list(ex.map(_run_pantry_example, ex_cases))
            for cdef, row in ex_pairs:
                dynamic_cases.append(cdef)
                results.append(row)

            if not raw_gh_srv.read_log_since(0):
                raise RuntimeError("offline raw-gh localhost mirror received 0 requests during pantry-dev")

            # 1 pkl.tar shell pipeline (bash -e -o pipefail packages/pkl.tar/examples/test_examples.sh)
            tar_cdef = {
                "id": "m4:pantry-dev:shell-suite:packages/pkl.tar/examples/test_examples.sh",
                "lane": "pantry-dev",
                "expected_exit": [0],
                "comparison_mode": "raw",
                "corpus_case_id": "pkl-pantry:shell-suite:packages/pkl.tar/examples/test_examples.sh",
            }
            tar_work = pantry_work / "tar_shell"
            tar_work.mkdir()
            j_state = tar_work / "j_state"
            j_bin_dir = tar_work / "jbin"
            j_bin_dir.mkdir()
            j_prefix = java_prefix_for_state(oracle_meta["command"], j_state)
            j_wrap = j_bin_dir / "pkl"
            j_wrap.write_text(
                f"#!{sys.executable}\nimport os, sys\n"
                f"argv = sys.argv[1:]\n"
                f"injected = argv[:1] + ['--cache-dir', {str(remote_cache)!r}] + argv[1:] if argv else ['--cache-dir', {str(remote_cache)!r}]\n"
                f"os.execv({j_prefix[0]!r}, {j_prefix!r} + injected)\n",
                encoding="utf-8",
            )
            j_wrap.chmod(0o755)
            bash_bin = shutil.which("bash", path=oracle_meta["environment"].get("PATH", os.defpath)) or "/bin/bash"
            script_path = str(pantry_tree / "packages/pkl.tar/examples/test_examples.sh")

            cdir_name = compat.case_directory(tar_cdef["id"])
            folder = output / "cases" / cdir_name
            folder.mkdir(parents=True)
            j_env = build_case_env(
                oracle_meta["environment"],
                {},
                j_state,
                prepend_path_dir=j_bin_dir,
            )
            j_cmd = [bash_bin, "-e", "-o", "pipefail", script_path]
            j_raw = capture_devnull(j_cmd, pantry_tree, j_env, timeout)
            j_stored = compat.store_capture(folder / "java", j_raw)

            r_cmd = None
            r_raw = None
            r_stored = None
            if candidate_bin is not None:
                r_state = tar_work / "r_state"
                r_bin_dir = tar_work / "rbin"
                r_bin_dir.mkdir()
                r_wrap = r_bin_dir / "pkl"
                r_wrap.write_text(
                    f"#!{sys.executable}\nimport os, sys\n"
                    f"argv = sys.argv[1:]\n"
                    f"injected = argv[:1] + ['--cache-dir', {str(remote_cache)!r}] + argv[1:] if argv else ['--cache-dir', {str(remote_cache)!r}]\n"
                    f"os.execv({str(candidate_bin)!r}, [{str(candidate_bin)!r}] + injected)\n",
                    encoding="utf-8",
                )
                r_wrap.chmod(0o755)
                r_env = build_case_env(
                    oracle_meta["environment"],
                    {},
                    r_state,
                    prepend_path_dir=r_bin_dir,
                    java_version=java_version,
                )
                r_cmd = [bash_bin, "-e", "-o", "pipefail", script_path]
                r_raw = capture_devnull(r_cmd, pantry_tree, r_env, timeout)
                r_stored = compat.store_capture(folder / "rust", r_raw)

            st, diffs = classify_result(tar_cdef, j_raw, r_raw)
            dynamic_cases.append(tar_cdef)
            results.append({
                "id": tar_cdef["id"],
                "lane": "pantry-dev",
                "directory": cdir_name,
                "status": st,
                "differences": diffs,
                "expected_exit": [0],
                "comparison_mode": "raw",
                "java_assertion_failures": [],
                "rust_assertion_failures": [],
                "package_archive_meta": None,
                "java": {"command": j_cmd, "result": j_stored},
                "rust": None if r_stored is None else {"command": r_cmd, "result": r_stored},
            })
        elif run_packaged:
            # Generate the 29 Pantry packages via Java into generated_dir so pantry-packaged can run standalone
            def _build_java_pkg(item: tuple[int, dict]) -> None:
                idx, c = item
                out_dir = generated_dir / f"pkg_{idx}"
                out_dir.mkdir(parents=True)
                args = [a.replace("{output_dir}", str(out_dir)) for a in c["command"]]
                args += [
                    "--cache-dir", str(remote_cache),
                    "--allowed-resources", "file:,pkl:,https:,prop:,package:,projectpackage:",
                    *raw_gh_flags,
                ]
                proc = subprocess.run([*oracle_meta["command"], *args], cwd=pantry_cwd, env=oracle_meta["environment"], capture_output=True, timeout=timeout)
                if proc.returncode != 0:
                    raise RuntimeError(f"failed building Pantry package {c['id']}: {proc.stderr.decode(errors='replace')}")
            with ThreadPoolExecutor(max_workers=workers) as ex:
                list(ex.map(_build_java_pkg, enumerate(pkg_api_cases)))

    # --- Lane 5: pantry-packaged ---
    if run_packaged:
        packaged_cache = pantry_work / "packaged_cache"
        pantry_offline.seed_remote_cache(packaged_cache, root)
        generated_meta: dict[str, dict] = {}
        for meta_file in sorted(generated_dir.rglob("*")):
            if not meta_file.is_file() or meta_file.suffix in (".sha256", ".zip"):
                continue
            item = pantry_offline.seed_generated_package_into_cache(meta_file, packaged_cache)
            generated_meta[item["uri"]] = item

        if len(generated_meta) != 29:
            raise RuntimeError(f"expected 29 generated Pantry packages, got {len(generated_meta)}")

        all_modules = sorted({m for pkg in generated_meta.values() for m in pkg["modules"]})

        # 206 packaged module evals (--no-project, warm cache)
        def _run_packaged_mod(uri: str) -> tuple[dict, dict]:
            cdef = {
                "id": f"m4:pantry-packaged:module:{uri}",
                "lane": "pantry-packaged",
                "expected_exit": [0, 1],
                "comparison_mode": "raw",
                "module_uri": uri,
            }
            args = [
                "eval", "--no-project", "--settings", "pkl:settings",
                "--cache-dir", "{cache_dir}",
                "--property", "pkl-pantry.testMode=1",
                uri,
            ]
            return cdef, _run_pair(cdef, args, pantry_work, packaged_cache)

        with ThreadPoolExecutor(max_workers=workers) as ex:
            mod_pairs = list(ex.map(_run_packaged_mod, all_modules))
        multi_file_uris: list[str] = []
        for cdef, row in mod_pairs:
            dynamic_cases.append(cdef)
            results.append(row)
            j_stderr = (output / "cases" / row["directory"] / "java/stderr.bin").read_bytes()
            if b"multiple-file output" in j_stderr or b"`-m` flag" in j_stderr:
                multi_file_uris.append(cdef["module_uri"])

        # 4 multi-file (-m) retries
        def _run_multi_file(uri: str) -> tuple[dict, dict]:
            cdef = {
                "id": f"m4:pantry-packaged:multi-file:{uri}",
                "lane": "pantry-packaged",
                "expected_exit": [0, 1],
                "comparison_mode": "raw",
                "module_uri": uri,
            }
            args = [
                "eval", "--no-project", "--settings", "pkl:settings",
                "--cache-dir", "{cache_dir}",
                "--property", "pkl-pantry.testMode=1",
                "--multiple-file-output-path", "{output_dir}",
                uri,
            ]
            return cdef, _run_pair(cdef, args, pantry_work, packaged_cache, collect_out=True)

        with ThreadPoolExecutor(max_workers=workers) as ex:
            mf_pairs = list(ex.map(_run_multi_file, sorted(multi_file_uris)))
        for cdef, row in mf_pairs:
            dynamic_cases.append(cdef)
            results.append(row)

        # 2 configured generator evaluations (-m with pinned Pantry fixture input)
        for pkg_name, fixture_rel in pantry_offline.CONFIGURED_GENERATORS:
            pkg_uri = next(
                (u for u in generated_meta if u.rsplit("/", 1)[1].split("@", 1)[0] == pkg_name),
                None,
            )
            if pkg_uri is None:
                raise RuntimeError(f"missing generated Pantry package for generator: {pkg_name}")
            mod_uri = f"{pkg_uri}#/generate.pkl"
            fixture_path = (pantry_tree / fixture_rel).resolve()
            cdef = {
                "id": f"m4:pantry-packaged:configured-generator:{pkg_name}",
                "lane": "pantry-packaged",
                "expected_exit": [0],
                "comparison_mode": "raw",
                "module_uri": mod_uri,
            }
            args = [
                "eval", "--no-project", "--settings", "pkl:settings",
                "--cache-dir", "{cache_dir}",
                "--property", "pkl-pantry.testMode=1",
                "--property", f"source={fixture_path.as_uri()}",
                "--multiple-file-output-path", "{output_dir}",
                mod_uri,
            ]
            dynamic_cases.append(cdef)
            results.append(_run_pair(cdef, args, pantry_work, packaged_cache, collect_out=True))

        # 5 cold-cache offline HTTPS mirror targets via --http-rewrite + --ca-certificates + empty fresh cache!
        mirror_root = pantry_work / "pantry_mirror"
        pantry_offline.seed_mirror_root(mirror_root, generated_dir, root)
        with M4FixtureServers(
            oracle_meta,
            root / "tests/fixtures/m4/host_io",
            mirror_root,
            pantry_work / "mirror_server",
        ) as mirror_srv:
            rewrite_flags = pantry_offline.http_rewrite_args(mirror_srv.https_port)
            for pkg_name, mod_rel in pantry_offline.COLD_CACHE_MIRROR_TARGETS:
                pkg_uri = next(
                    (u for u in generated_meta if u.rsplit("/", 1)[1].split("@", 1)[0] == pkg_name),
                    None,
                )
                if pkg_uri is None:
                    raise RuntimeError(f"missing generated Pantry package for mirror target: {pkg_name}")
                mod_uri = f"{pkg_uri}#/{mod_rel}"
                cdef = {
                    "id": f"m4:pantry-packaged:cold-https-mirror:{pkg_name}:{mod_rel}",
                    "lane": "pantry-packaged",
                    "expected_exit": [0],
                    "comparison_mode": "raw",
                    "module_uri": mod_uri,
                }
                fresh_cache = pantry_work / f"cold_cache_{pkg_name}"
                args = [
                    "eval", "--no-project", "--settings", "pkl:settings",
                    "--cache-dir", "{cache_dir}",
                    "--ca-certificates", str(mirror_srv.ca_path),
                    *rewrite_flags,
                    "--property", "pkl-pantry.testMode=1",
                    mod_uri,
                ]
                cdir_name = compat.case_directory(cdef["id"])
                folder = output / "cases" / cdir_name
                folder.mkdir(parents=True)

                def _exec_cold(is_java: bool) -> tuple[list[str], dict, list[dict]]:
                    shutil.rmtree(fresh_cache, ignore_errors=True)
                    fresh_cache.mkdir(parents=True)
                    st_dir = pantry_work / f"cold_state_{pkg_name}_{'j' if is_java else 'r'}"
                    shutil.rmtree(st_dir, ignore_errors=True)
                    env = build_case_env(
                        oracle_meta["environment"], {}, st_dir,
                        java_version=None if is_java else java_version,
                    )
                    expanded = [a.replace("{cache_dir}", str(fresh_cache)) for a in args]
                    cmd = [*java_prefix_for_state(oracle_meta["command"], st_dir), *expanded] if is_java else [str(candidate_bin), *expanded]
                    log_off = mirror_srv.log_offset()
                    raw = capture_devnull(cmd, pantry_work, env, timeout)
                    events = mirror_srv.read_log_since(log_off)
                    return cmd, raw, events

                j_cmd, j_raw, j_events = _exec_cold(True)
                j_stored = compat.store_capture(folder / "java", j_raw)
                j_afails = [] if len(j_events) > 0 else ["assertion:cold_mirror_no_server_hit"]
                r_cmd = r_raw = r_stored = None
                r_events: list[dict] = []
                r_afails: list[str] = []
                if candidate_bin is not None:
                    r_cmd, r_raw, r_events = _exec_cold(False)
                    r_stored = compat.store_capture(folder / "rust", r_raw)
                    r_afails = [] if len(r_events) > 0 else ["assertion:cold_mirror_no_server_hit"]
                st, diffs = classify_result(
                    cdef,
                    j_raw,
                    r_raw,
                    java_assertion_failures=j_afails,
                    rust_assertion_failures=r_afails,
                )
                dynamic_cases.append(cdef)
                results.append({
                    "id": cdef["id"],
                    "lane": "pantry-packaged",
                    "directory": cdir_name,
                    "status": st,
                    "differences": diffs,
                    "expected_exit": cdef["expected_exit"],
                    "comparison_mode": "raw",
                    "java_assertion_failures": j_afails,
                    "rust_assertion_failures": r_afails,
                    "java_server_events_count": len(j_events),
                    "rust_server_events_count": len(r_events),
                    "package_archive_meta": None,
                    "java": {"command": j_cmd, "result": j_stored},
                    "rust": None if r_stored is None else {"command": r_cmd, "result": r_stored},
                })

    shutil.rmtree(pantry_work, ignore_errors=True)
    return dynamic_cases, results


# ---------------------------------------------------------------------------
# Candidate Build, Source Snapshot, Report Generation & Tamper-Proof Verifier
# ---------------------------------------------------------------------------

DEFAULT_CANDIDATE_PACKAGE = "pkl-host-native"
DEFAULT_CANDIDATE_BIN = "pkl_m4_cli"


def snapshot_rust_sources(root: Path) -> dict[str, str]:
    """Hash all Rust runtime & native host sources + Cargo manifests/lockfile."""
    files: dict[str, str] = {}
    fixed = (
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "crates/pkl-runtime/Cargo.toml",
        "crates/pkl-host-native/Cargo.toml",
        "vendor/java_regex/Cargo.toml",
    )
    for rel in fixed:
        p = root / rel
        if p.is_file():
            files[rel] = oracle.sha256(p)
    roots = (
        "crates/pkl-runtime/src",
        "crates/pkl-host-native/src",
        "crates/pkl-host-native/assets",
        "vendor/java_regex/src",
    )
    for rel_dir in roots:
        base = root / rel_dir
        if not base.is_dir():
            continue
        for current, dirs, names in os.walk(base, followlinks=False):
            dirs.sort()
            for name in sorted(names):
                p = Path(current) / name
                if p.is_symlink() or not p.is_file():
                    continue
                files[p.relative_to(root).as_posix()] = oracle.sha256(p)
    return dict(sorted(files.items()))


def build_bin(
    root: Path,
    package: str,
    bin_name: str,
    target_dir: Path,
    output: Path,
) -> tuple[Path, dict]:
    cmd = [
        "cargo", "build", "--release", "-p", package,
        "--bin", bin_name, "--locked", "--target-dir", str(target_dir),
    ]
    proc = subprocess.run(cmd, cwd=root, capture_output=True, timeout=600)
    (output / f"build-{bin_name}.stdout.bin").write_bytes(proc.stdout)
    (output / f"build-{bin_name}.stderr.bin").write_bytes(proc.stderr)
    record = {
        "command": cmd,
        "package": package,
        "bin": bin_name,
        "exit_code": proc.returncode,
        "stdout_sha256": digest(proc.stdout),
        "stderr_sha256": digest(proc.stderr),
    }
    if proc.returncode != 0:
        stderr_preview = proc.stderr.decode("utf-8", errors="replace")[-1200:]
        raise ValueError(f"failed building {package}::{bin_name} (rc={proc.returncode}): {stderr_preview}")
    built = target_dir / "release" / bin_name
    if not built.is_file():
        raise ValueError(f"built binary missing: {built}")
    return built, record


def build_example(root: Path, example_name: str, target_dir: Path, output: Path) -> tuple[Path, dict]:
    cmd = [
        "cargo", "build", "-p", "pkl-runtime", "--locked", "--release",
        "--example", example_name, "--target-dir", str(target_dir),
    ]
    proc = subprocess.run(cmd, cwd=root, capture_output=True, timeout=600)
    (output / f"build-{example_name}.stdout.bin").write_bytes(proc.stdout)
    (output / f"build-{example_name}.stderr.bin").write_bytes(proc.stderr)
    record = {
        "command": cmd,
        "package": "pkl-runtime",
        "example": example_name,
        "exit_code": proc.returncode,
        "stdout_sha256": digest(proc.stdout),
        "stderr_sha256": digest(proc.stderr),
    }
    if proc.returncode != 0:
        raise ValueError(f"failed building {example_name} (rc={proc.returncode})")
    built = target_dir / "release/examples" / example_name
    if not built.is_file():
        raise ValueError(f"built example missing: {built}")
    return built, record


def freeze_artifact(artifacts_dir: Path, name: str, path: Path, source_binding: str, build_rec: dict | None) -> tuple[Path, dict]:
    frozen = artifacts_dir / name
    frozen.write_bytes(path.read_bytes())
    frozen.chmod(0o755)
    return frozen, {
        "name": name,
        "original_path": str(path),
        "sha256": oracle.sha256(frozen),
        "source_binding": source_binding,
        "build": build_rec,
    }


def validate_static_cases(root: Path, cases_doc: dict) -> dict[str, dict]:
    if cases_doc.get("schema_version") != 1 or not isinstance(cases_doc.get("cases"), list):
        raise ValueError("invalid tests/fixtures/m4/cases.json schema")
    by_id: dict[str, dict] = {}
    for case in cases_doc["cases"]:
        cid = case["id"]
        if cid in by_id:
            raise ValueError(f"duplicate M4 case id: {cid}")
        if case.get("lane") not in STATIC_LANES:
            raise ValueError(f"unknown static M4 lane for {cid}: {case.get('lane')}")
        for inp in case.get("inputs", []):
            rel = inp["path"]
            p = Path(rel)
            if p.is_absolute() or ".." in p.parts:
                raise ValueError(f"invalid M4 fixture input path: {rel}")
            full = root / p
            if not full.is_file():
                raise ValueError(f"missing M4 fixture input: {rel}")
            if digest(full.read_bytes()) != inp["sha256"]:
                raise ValueError(f"M4 fixture input sha256 mismatch: {rel}")
        by_id[cid] = case
    return by_id


def execute(
    root: Path,
    output: Path,
    lanes: list[str],
    workers: int,
    timeout: float,
    java_only: bool,
    candidate: Path | None,
    candidate_bin: str | None,
    candidate_package: str,
    candidate_example: str | None,
    cargo_target_dir: Path | None,
) -> dict:
    cases_doc = json.loads((root / CASES_PATH).read_bytes())
    validate_static_cases(root, cases_doc)
    selected_static = [c for c in cases_doc["cases"] if c["lane"] in lanes]
    run_pantry_dev = "pantry-dev" in lanes
    run_pantry_packaged = "pantry-packaged" in lanes

    if output.exists() or output.is_symlink():
        raise ValueError(f"output directory must be fresh (does not exist): {output}")

    oracle_meta = oracle.ensure_oracle(root, fetch=False)
    if not oracle_meta.get("exact_jdk_lock_match", False):
        raise ValueError("pinned oracle JDK lock mismatch (exact_jdk_lock_match is false)")

    sources_before = snapshot_rust_sources(root)

    output.mkdir(parents=True, exist_ok=False)
    artifacts = output / "artifacts"
    artifacts.mkdir()
    compat.write_json(output / "oracle.json", oracle_meta)
    (output / "cases.snapshot.json").write_bytes((root / CASES_PATH).read_bytes())

    frozen_candidate: Path | None = None
    candidate_binding: dict | None = None
    if not java_only:
        tdir = cargo_target_dir.resolve() if cargo_target_dir else (root / "target")
        if candidate is not None:
            frozen_candidate, candidate_binding = freeze_artifact(
                artifacts, candidate.name, candidate.resolve(), "unbound-caller-supplied", None
            )
        elif candidate_example is not None:
            built, rec = build_example(root, candidate_example, tdir, output)
            frozen_candidate, candidate_binding = freeze_artifact(
                artifacts, candidate_example, built, "recorded-build", rec
            )
        else:
            bin_name = candidate_bin or DEFAULT_CANDIDATE_BIN
            built, rec = build_bin(root, candidate_package, bin_name, tdir, output)
            frozen_candidate, candidate_binding = freeze_artifact(
                artifacts, bin_name, built, "recorded-build", rec
            )

    results: list[dict] = []
    server_self_test: dict | None = None

    if selected_static:
        with M4FixtureServers(
            oracle_meta,
            root / "tests/fixtures/m4/host_io",
            root / "tests/fixtures/m4/projects/registry",
            output / "_server",
        ) as servers:
            server_self_test = servers.self_test()
            if not server_self_test["passed"]:
                raise RuntimeError("M4HttpFixtureServer self-test failed: " + json.dumps(server_self_test))
            # Run static cases sequentially so per-case proxy request log slices are 100% deterministic.
            for case in selected_static:
                row = run_static_case(root, case, output, oracle_meta, servers, frozen_candidate, timeout)
                results.append(row)
        shutil.rmtree(output / "_server", ignore_errors=True)

    dynamic_cases: list[dict] = []
    if run_pantry_dev or run_pantry_packaged:
        dynamic_cases, pantry_results = run_pantry_lanes(
            root, output, oracle_meta, frozen_candidate, run_pantry_dev, run_pantry_packaged, workers, timeout
        )
        results.extend(pantry_results)

    sources_after = snapshot_rust_sources(root)
    source_stability = sources_before == sources_after
    compat.write_json(output / "sources.snapshot.json", {"before": sources_before, "after": sources_after})
    compat.write_json(output / "dynamic-cases.snapshot.json", {"schema_version": 1, "cases": dynamic_cases})

    by_status = dict(sorted(Counter(r["status"] for r in results).items()))
    by_lane_status = {
        lane: dict(sorted(Counter(r["status"] for r in results if r["lane"] == lane).items()))
        for lane in lanes
    }
    report = {
        "schema_version": 1,
        "baseline": cases_doc["baseline"],
        "mode": "java-only-oracle-recording" if frozen_candidate is None else "differential",
        "cases_manifest_sha256": oracle.sha256(root / CASES_PATH),
        "dynamic_cases_sha256": oracle.sha256(output / "dynamic-cases.snapshot.json"),
        "sources_snapshot_sha256": oracle.sha256(output / "sources.snapshot.json"),
        "sources_file_count": len(sources_before),
        "source_stability": source_stability,
        "lanes": lanes,
        "workers": workers,
        "timeout_seconds": timeout,
        "oracle": oracle_meta,
        "candidate": candidate_binding,
        "server_self_test": server_self_test,
        "selected_count": len(results),
        "status_counts": by_status,
        "lane_status_counts": by_lane_status,
        "results": results,
    }
    report["evidence_sha256"] = {
        str(p.relative_to(output).as_posix()): oracle.sha256(p) for p in sorted(evidence_paths(output))
    }
    compat.write_json(output / "report.json", report)
    verify_report(output, strict=False)
    return report


def verify_report(directory: Path, *, strict: bool = False) -> dict:
    report = json.loads((directory / "report.json").read_bytes())
    if report.get("schema_version") != 1:
        raise ValueError("unsupported M4 differential report schema_version")
    if json.loads((directory / "oracle.json").read_bytes()) != report["oracle"]:
        raise ValueError("oracle.json differs from report.json")

    recorded_evidence = report.get("evidence_sha256", {})
    actual_files = {str(p.relative_to(directory).as_posix()) for p in evidence_paths(directory)}
    if not recorded_evidence or set(recorded_evidence) != actual_files:
        raise ValueError("evidence file set differs from recorded evidence_sha256")
    for rel, expected_sha in recorded_evidence.items():
        if oracle.sha256(directory / rel) != expected_sha:
            raise ValueError(f"evidence file modified: {rel}")

    if oracle.sha256(directory / "cases.snapshot.json") != report.get("cases_manifest_sha256"):
        raise ValueError("cases.snapshot.json sha256 disagrees with cases_manifest_sha256")
    if oracle.sha256(directory / "dynamic-cases.snapshot.json") != report.get("dynamic_cases_sha256"):
        raise ValueError("dynamic-cases.snapshot.json sha256 disagrees with dynamic_cases_sha256")

    cand_info = report.get("candidate")
    if cand_info is not None:
        if not isinstance(cand_info, dict):
            raise ValueError("candidate metadata must be an object or null")
        cand_name = cand_info.get("name")
        if (
            not isinstance(cand_name, str)
            or not cand_name
            or "/" in cand_name
            or "\\" in cand_name
            or cand_name in (".", "..")
        ):
            raise ValueError(f"invalid candidate artifact name: {cand_name!r}")
        artifact_rel = f"artifacts/{cand_name}"
        if artifact_rel not in recorded_evidence:
            raise ValueError(f"candidate artifact missing from evidence_sha256: {artifact_rel}")
        if cand_info.get("sha256") != recorded_evidence[artifact_rel]:
            raise ValueError(f"candidate sha256 disagrees with evidence_sha256[{artifact_rel!r}]")
        build_rec = cand_info.get("build")
        if isinstance(build_rec, dict):
            build_target = build_rec.get("bin") or build_rec.get("example")
            if isinstance(build_target, str) and build_target:
                for stream, key in (("stdout", "stdout_sha256"), ("stderr", "stderr_sha256")):
                    rel = f"build-{build_target}.{stream}.bin"
                    if rel not in recorded_evidence or build_rec.get(key) != recorded_evidence[rel]:
                        raise ValueError(f"candidate build {stream} sha256 disagrees with {rel}")

    static_doc = json.loads((directory / "cases.snapshot.json").read_bytes())
    dynamic_doc = json.loads((directory / "dynamic-cases.snapshot.json").read_bytes())
    cases_by_id = {c["id"]: c for c in static_doc["cases"] + dynamic_doc["cases"]}
    expected_ids = [c["id"] for c in static_doc["cases"] if c["lane"] in report["lanes"]] + [
        c["id"] for c in dynamic_doc["cases"] if c["lane"] in report["lanes"]
    ]
    result_ids = [r["id"] for r in report["results"]]
    if result_ids != expected_ids or len(result_ids) != report["selected_count"]:
        raise ValueError("result list order or membership differs from selected lanes")

    for row in report["results"]:
        case = cases_by_id[row["id"]]
        expected_dir = compat.case_directory(row["id"])
        if row["directory"] != expected_dir:
            raise ValueError(f"unexpected capture directory for {row['id']}")
        folder = directory / "cases" / expected_dir
        java_cap = compat.load_capture(folder / "java")
        rust_cap = compat.load_capture(folder / "rust") if row["rust"] is not None else None

        if (
            case.get("comparison_mode") == "package-archive"
            and java_cap["exit_code"] == 0
            and rust_cap is not None
            and rust_cap["exit_code"] == 0
        ):
            interop_mods = (case.get("assertions") or {}).get("cross_interop_modules", [])
            detail = row.get("cross_interop_detail")
            if interop_mods or detail is not None:
                if not isinstance(detail, dict) or not isinstance(detail.get("modules"), dict):
                    raise ValueError(f"missing or invalid cross_interop_detail for {row['id']}")
                modules = detail["modules"]
                for mod_uri in interop_mods:
                    if mod_uri not in modules:
                        raise ValueError(f"cross_interop_detail missing module {mod_uri} for {row['id']}")
                for mod_uri, pair in modules.items():
                    if not isinstance(pair, dict):
                        raise ValueError(f"invalid cross_interop_detail entry for {row['id']} ({mod_uri})")
                    j_res = pair.get("java_pkg")
                    r_res = pair.get("rust_pkg")
                    pair_ok = (
                        isinstance(j_res, dict)
                        and isinstance(r_res, dict)
                        and j_res.get("exit_code") == 0
                        and j_res == r_res
                    )
                    if pair.get("match") is not pair_ok:
                        raise ValueError(f"cross_interop_detail match flag disagrees for {row['id']} ({mod_uri})")
                checked_mods = interop_mods if interop_mods else list(modules)
                expected_interop_ok = all(modules[m]["match"] is True for m in checked_mods)
                if row.get("cross_interop_ok") is not expected_interop_ok:
                    raise ValueError(
                        f"cross_interop_ok disagrees with cross_interop_detail for {row['id']}"
                    )

        if row["id"].startswith("m4:pantry-packaged:cold-https-mirror:"):
            if "java_server_events_count" in row:
                if row["java_server_events_count"] == 0 and "assertion:cold_mirror_no_server_hit" not in (
                    row.get("java_assertion_failures") or []
                ):
                    raise ValueError(f"cold-https-mirror java server hit assertion missing for {row['id']}")
            if row["rust"] is not None and "rust_server_events_count" in row:
                if row["rust_server_events_count"] == 0 and "assertion:cold_mirror_no_server_hit" not in (
                    row.get("rust_assertion_failures") or []
                ):
                    raise ValueError(f"cold-https-mirror rust server hit assertion missing for {row['id']}")

        status, diffs = classify_result(
            case,
            java_cap,
            rust_cap,
            java_assertion_failures=row.get("java_assertion_failures"),
            rust_assertion_failures=row.get("rust_assertion_failures"),
            cross_interop_ok=row.get("cross_interop_ok"),
            idempotency_ok=row.get("idempotency_ok"),
        )
        if row["status"] != status or row["differences"] != diffs:
            raise ValueError(f"recomputed status/differences disagree for {row['id']}: {row['status']} vs {status}")

    recomputed = dict(sorted(Counter(r["status"] for r in report["results"]).items()))
    if recomputed != report["status_counts"]:
        raise ValueError("recomputed status_counts disagree with report.json")
    recomputed_lanes = {
        lane: dict(sorted(Counter(r["status"] for r in report["results"] if r["lane"] == lane).items()))
        for lane in report["lanes"]
    }
    if recomputed_lanes != report["lane_status_counts"]:
        raise ValueError("recomputed lane_status_counts disagree with report.json")

    if "sources_snapshot_sha256" in report:
        src_snap = directory / "sources.snapshot.json"
        if not src_snap.is_file() or oracle.sha256(src_snap) != report["sources_snapshot_sha256"]:
            raise ValueError("sources.snapshot.json missing or sha256 disagrees with report.json")
        snap_doc = json.loads(src_snap.read_bytes())
        if (snap_doc.get("before") == snap_doc.get("after")) != report.get("source_stability"):
            raise ValueError("recomputed source_stability disagrees with report.json")

    if strict:
        if not report["oracle"].get("exact_jdk_lock_match", False):
            raise ValueError("strict gate requires exact_jdk_lock_match == true")
        cand = report.get("candidate")
        if cand is None or cand.get("source_binding") != "recorded-build":
            raise ValueError("strict gate requires a recorded-build Rust candidate (java-only recordings cannot pass)")
        if "sources_snapshot_sha256" not in report or report.get("source_stability") is not True:
            raise ValueError("strict gate requires sources_snapshot_sha256 and source_stability == true")
        for row in report["results"]:
            if row["id"].startswith("m4:pantry-packaged:cold-https-mirror:"):
                if not isinstance(row.get("java_server_events_count"), int) or row["java_server_events_count"] <= 0:
                    raise ValueError(f"strict gate requires cold-https-mirror java server hits > 0 for {row['id']}")
                if not isinstance(row.get("rust_server_events_count"), int) or row["rust_server_events_count"] <= 0:
                    raise ValueError(f"strict gate requires cold-https-mirror rust server hits > 0 for {row['id']}")
        non_pass = [r for r in report["results"] if r["status"] != "pass"]
        if non_pass:
            summary = ", ".join(f"{r['id']}={r['status']}" for r in non_pass[:10])
            raise ValueError(f"strict gate failed: {len(non_pass)} non-pass case(s): {summary}")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--output", type=Path, help="fresh evidence directory for live run")
    action.add_argument("--verify-report", type=Path, help="verify integrity only (preserves recorded failures)")
    action.add_argument("--verify-strict", type=Path, help="verify integrity AND require 100%% pass + recorded-build + exact JDK lock")
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--lane", action="append", choices=ALL_LANES, help="select specific lane(s); default runs all 5")
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT)
    parser.add_argument("--java-only", action="store_true", help="record pinned Java 0.32.0 oracle baseline only (never counts as pass)")
    parser.add_argument("--record-only", action="store_true", help="exit 0 after integrity verification even with mismatches")
    parser.add_argument("--candidate", type=Path, help="caller-supplied Rust CLI candidate binary (unbound)")
    parser.add_argument("--candidate-bin", type=str, default=None, help=f"Cargo bin name to build as recorded-build candidate (default: {DEFAULT_CANDIDATE_BIN})")
    parser.add_argument("--candidate-package", type=str, default=DEFAULT_CANDIDATE_PACKAGE, help=f"Cargo package for --candidate-bin (default: {DEFAULT_CANDIDATE_PACKAGE})")
    parser.add_argument("--candidate-example", type=str, help="Cargo example name in pkl-runtime to build as recorded-build candidate")
    parser.add_argument("--cargo-target-dir", type=Path, help="Cargo target directory for recorded builds")
    args = parser.parse_args()

    try:
        if args.output:
            if not 1 <= args.workers <= 16:
                raise ValueError("workers must be between 1 and 16")
            lanes = list(dict.fromkeys(args.lane)) if args.lane else list(ALL_LANES)
            report = execute(
                args.root.resolve(),
                args.output.resolve(),
                lanes,
                args.workers,
                args.timeout,
                args.java_only,
                args.candidate,
                args.candidate_bin,
                args.candidate_package,
                args.candidate_example,
                args.cargo_target_dir,
            )
            print(json.dumps({
                "mode": report["mode"],
                "selected_count": report["selected_count"],
                "status_counts": report["status_counts"],
                "lane_status_counts": report["lane_status_counts"],
            }, sort_keys=True))
            if args.record_only or args.java_only:
                return 0
            verify_report(args.output.resolve(), strict=True)
            return 0
        target = (args.verify_report or args.verify_strict).resolve()
        report = verify_report(target, strict=bool(args.verify_strict))
        print(json.dumps({
            "mode": report["mode"],
            "selected_count": report["selected_count"],
            "status_counts": report["status_counts"],
            "lane_status_counts": report["lane_status_counts"],
        }, sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"M4 differential error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
