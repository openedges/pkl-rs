#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""5-lane offline raw-byte Java 0.32.0 differential runner for Milestone M6 (Wasm & N-API).

Lanes:
  1. wasm-browser          : Real Chromium via pinned Playwright 1.64.1 loading clean-installed
                             packed `@pkl-rs/wasm` (language, stdlib, all 9 renderers, output.bytes,
                             output.files, formatter, schema, REPL, analyze, test, VFS + rootDir,
                             async custom loaders, localhost HTTP fetch, pure-Wasm package archive,
                             lossless BigInt/Uint8Array/UTF-16, 64-bit clock, cancel/timeout/close,
                             conversion telemetry, plus explicit capability-blocked host-gap probes)
  2. wasm-node             : Node.js importing clean-installed packed `@pkl-rs/wasm`
  3. napi-node             : Node.js ESM import & CJS require of clean-installed packed `@pkl-rs/napi`
                             (full shared corpus + real Node FS/env/prop/modulepath dir+JAR,
                             localhost HTTP/HTTPS/CA/headers/rewrites/proxy/noProxy, PklProject &
                             package:// + package-2 disk cache, external readers 0x26-0x32 with
                             0x32 close & zero orphan PIDs, custom JS async readers/logger/frame/
                             security, non-blocking event-loop async + cancel/timeout/close,
                             lossless BigInt/Uint8Array/UTF-16, conversion telemetry)
  4. cross-target-parity   : 4-way / 5-endpoint raw-byte check on shared portable corpus:
                             Java 0.32.0 == Native Rust `pkl` == Node N-API == Wasm Node == Wasm Browser
                             (reporting per-target match counts separately without masking gaps)
  5. packaging-and-install : Reproducible build, pack, clean isolated temp install & live execution
                             for native `pkl`/`pkldoc` + standalone downstream Rust crate linking
                             `crates/pkl`, packed `@pkl-rs/wasm` (Node + Chromium), packed
                             `@pkl-rs/napi` (ESM + CJS), and platform support matrix metadata.

Reuses `tools/oracle.py` (pinned Eclipse Adoptium Temurin 21.0.13.1+2-LTS,
`exact_jdk_lock_match == true`, zero `PKL_ALLOW_LOCAL_JDK`), `tools/compat.py`,
`tools/m4_differential.py::M4FixtureServers`, and pinned Playwright 1.64.1.

Honesty invariants:
  - Raw-byte comparison only: NEVER trims, sorts, reserializes, or normalizes stdout/stderr/files.
  - Explicit capability-unavailable host-gap cases are classified strictly as `capability-blocked`
    and are NEVER counted in `compatibility_pass_count` or `status_counts["pass"]`.
  - Required executable cases (`capability_boundary: false`) MUST pass (`status == "pass"`).
  - `--oracle-only` recordings NEVER pass `--verify-strict`.
"""
from __future__ import annotations

import argparse
import base64
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tarfile
import time
import zipfile

import compat
import m4_differential
import oracle

ROOT = Path(__file__).resolve().parents[1]
CASES_PATH = Path("tests/fixtures/m6/cases.json")
ORACLE_PROBE_JAVA = Path("tests/fixtures/m6/oracle/M6OracleProbe.java")
M4_READER_BIN = Path("tests/fixtures/m4/readers/m4_ext_reader.py")
RUNNERS_DIR = Path("tests/fixtures/m6/runners")
PLAYWRIGHT_PACKAGE_JSON = Path("tools/platform/package.json")

ALL_LANES = (
    "wasm-browser",
    "wasm-node",
    "napi-node",
    "cross-target-parity",
    "packaging-and-install",
)
CROSS_TARGET_ENDPOINTS = ("native", "napi_node", "wasm_node", "wasm_browser")
DEFAULT_TIMEOUT = 60.0

DEFAULT_PKL_PACKAGE = "pkl-cli"
DEFAULT_PKL_BIN = "pkl"
DEFAULT_PKLDOC_PACKAGE = "pkl-doc"
DEFAULT_PKLDOC_BIN = "pkldoc"

VALID_EXEC_MODES = {
    "js_api",
    "cross_target_parity",
    "packaging_native",
    "packaging_rust_crate",
    "packaging_wasm",
    "packaging_napi",
    "packaging_matrix",
}
VALID_ORACLE_MODES = {"probe", "cli"}
NATIVE_VERSION_RE = re.compile(rb"^(pkl-rs|pkldoc-rs) 0\.32\.0 \(([^,]+), native\)\n$")


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


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


def capture_process(
    command: list[str],
    cwd: Path,
    env: dict[str, str],
    timeout: float,
    *,
    stdin_bytes: bytes | None = None,
) -> dict:
    """Capture process stdout/stderr/exit with POSIX process-group cleanup."""
    if os.name != "posix":
        raise RuntimeError("M6 differential runner requires POSIX process groups")
    before = time.perf_counter_ns()
    proc = subprocess.Popen(
        command,
        cwd=cwd,
        env=env,
        stdin=subprocess.PIPE if stdin_bytes is not None else subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    timed_out = False
    try:
        try:
            stdout, stderr = proc.communicate(input=stdin_bytes, timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass
            try:
                stdout, stderr = proc.communicate(timeout=5)
            except subprocess.TimeoutExpired as err:
                stdout, stderr = err.output or b"", err.stderr or b""
                proc.wait(timeout=5)
    finally:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
    return {
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": proc.returncode,
        "files": {},
        "timed_out": timed_out,
        "workspace_changes": {},
        "elapsed_ns": time.perf_counter_ns() - before,
    }


def build_case_env(
    base_env: dict[str, str],
    case_env: dict[str, str],
    state_dir: Path,
) -> dict[str, str]:
    home = state_dir / "home"
    tmp = state_dir / "tmp"
    home.mkdir(parents=True, exist_ok=True)
    tmp.mkdir(parents=True, exist_ok=True)
    return {
        **base_env,
        **case_env,
        "HOME": str(home),
        "TMPDIR": str(tmp),
        "TMP": str(tmp),
        "TEMP": str(tmp),
        "PYTHONDONTWRITEBYTECODE": "1",
    }


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


def compile_oracle_probe_java(oracle_meta: dict, work_dir: Path, root: Path = ROOT) -> Path:
    """Compile tests/fixtures/m6/oracle/M6OracleProbe.java against pinned pkl-tools-0.32.0.jar."""
    classes = work_dir / "m6_probe_classes"
    classes.mkdir(parents=True, exist_ok=True)
    java_home = Path(oracle_meta["jdk"]["properties"]["java.home"])
    javac = java_home / "bin/javac"
    env = dict(oracle_meta["environment"])
    tmp = work_dir / "javac_tmp"
    tmp.mkdir(exist_ok=True)
    env.update(TMPDIR=str(tmp), TMP=str(tmp), TEMP=str(tmp))
    proc = subprocess.run(
        [
            str(javac),
            "-encoding", "UTF-8",
            "-cp", oracle_meta["artifact"],
            "-d", str(classes),
            str(root / ORACLE_PROBE_JAVA),
        ],
        cwd=work_dir,
        env=env,
        capture_output=True,
        timeout=60,
    )
    if proc.returncode != 0:
        raise RuntimeError(
            f"failed compiling {ORACLE_PROBE_JAVA} (rc={proc.returncode}): "
            f"{proc.stderr.decode('utf-8', errors='replace')}"
        )
    return classes


def java_probe_prefix(oracle_meta: dict, classes_dir: Path, state_dir: Path) -> list[str]:
    base = java_prefix_for_state(oracle_meta["command"], state_dir)
    out = []
    skip_next = False
    for i, arg in enumerate(base[:-1]):
        if skip_next:
            skip_next = False
            continue
        if arg == "-cp" and i + 1 < len(base) - 1:
            out.extend(["-cp", f"{classes_dir}{os.pathsep}{base[i + 1]}"])
            skip_next = True
        else:
            out.append(arg)
    out.append("M6OracleProbe")
    return out


def expand_placeholders(obj: object, mapping: dict[str, str]):
    if isinstance(obj, str):
        out = obj
        for k, v in mapping.items():
            out = out.replace("{" + k + "}", v)
        return out
    if isinstance(obj, list):
        return [expand_placeholders(x, mapping) for x in obj]
    if isinstance(obj, dict):
        return {expand_placeholders(k, mapping): expand_placeholders(v, mapping) for k, v in obj.items()}
    return obj


def apply_case_setup(
    root: Path,
    setup: dict | None,
    work_dir: Path,
    output_dir: Path,
    cache_dir: Path,
    home_dir: Path,
    placeholders: dict[str, str],
) -> set[str]:
    declared_symlinks: set[str] = set()
    work_dir.mkdir(parents=True, exist_ok=True)
    output_dir.mkdir(parents=True, exist_ok=True)
    cache_dir.mkdir(parents=True, exist_ok=True)
    home_dir.mkdir(parents=True, exist_ok=True)
    if not setup:
        return declared_symlinks
    for src_rel, dst_rel in setup.get("copy_dir", []):
        src = compat.relative_path(root, src_rel)
        dst = compat.relative_path(work_dir, dst_rel)
        shutil.copytree(src, dst, symlinks=True)
    for src_rel, dst_rel in setup.get("copy_file", []):
        src = compat.relative_path(root, src_rel)
        dst = compat.relative_path(work_dir, dst_rel)
        dst.parent.mkdir(parents=True, exist_ok=True)
        dst.write_bytes(src.read_bytes())
    for rel_dir in setup.get("dirs", []):
        compat.relative_path(work_dir, rel_dir).mkdir(parents=True, exist_ok=True)
    for rel_file, tmpl in setup.get("templates", []):
        dst = compat.relative_path(work_dir, rel_file)
        dst.parent.mkdir(parents=True, exist_ok=True)
        dst.write_bytes(expand_placeholders(tmpl, placeholders).encode("utf-8"))
    for jar_rel, entries in setup.get("jars", []):
        jar_path = compat.relative_path(work_dir, jar_rel)
        jar_path.parent.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(jar_path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
            for entry_name, entry_text in sorted(entries.items()):
                info = zipfile.ZipInfo(entry_name, date_time=(1980, 2, 1, 0, 0, 0))
                zf.writestr(info, expand_placeholders(entry_text, placeholders).encode("utf-8"))
    for link_rel, target in setup.get("symlinks", []):
        link_path = compat.relative_path(work_dir, link_rel)
        link_path.parent.mkdir(parents=True, exist_ok=True)
        link_path.symlink_to(target)
        declared_symlinks.add(Path(link_rel).as_posix())
    return declared_symlinks


def check_reader_lifecycle(lifecycle_log: Path) -> list[str]:
    failures: list[str] = []
    if not lifecycle_log.is_file():
        return ["assertion:lifecycle_log_missing"]
    evs = [json.loads(ln) for ln in lifecycle_log.read_text(encoding="utf-8").splitlines() if ln.strip()]
    starts = [e for e in evs if e.get("event") == "start"]
    closes = [e for e in evs if e.get("event") == "close"]
    pids = sorted({int(e["pid"]) for e in starts if "pid" in e})
    if not starts:
        failures.append("assertion:reader_never_started")
    if not closes:
        failures.append("assertion:reader_missing_close_0x32")
    for pid in pids:
        deadline = time.monotonic() + 1.0
        while m4_differential.pid_alive(pid) and time.monotonic() < deadline:
            time.sleep(0.05)
        if m4_differential.pid_alive(pid):
            failures.append(f"assertion:reader_orphan_pid_{pid}")
    return failures


def check_case_assertions(
    case: dict,
    raw: dict,
    lifecycle_log: Path,
    server_events: list[dict],
    *,
    is_java: bool,
    extra_assertions: list[str] | None = None,
    telemetry: dict | None = None,
) -> list[str]:
    assertions = case.get("assertions") or {}
    failures: list[str] = []
    if extra_assertions:
        failures.extend(f"target:{a}" for a in extra_assertions)
    if assertions.get("reader_closed"):
        failures.extend(check_reader_lifecycle(lifecycle_log))
    if assertions.get("http_server_hit"):
        if not server_events:
            failures.append("assertion:http_server_hit_missing")
    if assertions.get("proxy_hit"):
        if not any(ev.get("server") == "proxy" for ev in server_events):
            failures.append("assertion:proxy_hit_missing")
    if assertions.get("min_generated_files") is not None:
        n_gen = sum(1 for k in raw.get("files", {}) if k.startswith("generated/"))
        if n_gen < int(assertions["min_generated_files"]):
            failures.append(f"assertion:min_generated_files({n_gen}<{assertions['min_generated_files']})")
    if not is_java and assertions.get("telemetry_required"):
        if not isinstance(telemetry, dict) or telemetry.get("iterations", 0) < 1 or "median_ns" not in telemetry:
            failures.append("assertion:conversion_telemetry_missing")
    return failures


def classify_result(
    case: dict,
    expected: dict,
    actual: dict | None,
    *,
    java_assertion_failures: list[str] | None = None,
    candidate_assertion_failures: list[str] | None = None,
    capability_gap: dict | None = None,
    cross_target_captures: dict[str, dict] | None = None,
) -> tuple[str, list[str]]:
    """Classify a single M6 case strictly without normalization or capability inflation.

    Honesty rules:
      - `capability_boundary: true` cases NEVER return `pass` — they return `capability-blocked`
        when the target accurately declares `capabilities()[cap] === false` and rejects with
        `PklError(code="capability-unavailable", capability=cap)`, or `mismatch` otherwise.
      - `capability_boundary: false` (required executable) cases NEVER return `capability-blocked`;
        a missing capability on a required executable case is a `mismatch`!
      - Raw-byte differences in stdout/stderr/exit_code/files always return `mismatch`.
    """
    if expected["timed_out"]:
        return "oracle-error", ["oracle_timed_out"]
    if expected["exit_code"] not in case["expected_exit"]:
        return "oracle-contract-mismatch", [f"oracle_exit_{expected['exit_code']}"]
    if java_assertion_failures:
        return "oracle-assertion-failed", list(java_assertion_failures)

    # Oracle-only baseline mode
    if case["exec_mode"] == "cross_target_parity":
        if cross_target_captures is None:
            return "oracle-recorded", []
        diffs: list[str] = []
        if candidate_assertion_failures:
            diffs.extend(candidate_assertion_failures)
        for ep in CROSS_TARGET_ENDPOINTS:
            cap = cross_target_captures.get(ep)
            if cap is None:
                diffs.append(f"{ep}:missing")
                continue
            if cap.get("timed_out"):
                return "candidate-timeout", [f"{ep}:timed_out"]
            ep_diffs = compat.differences(expected, cap)
            diffs.extend(f"{ep}:{d}" for d in ep_diffs)
        return ("mismatch", diffs) if diffs else ("pass", [])

    if actual is None:
        return "oracle-recorded", []
    if actual["timed_out"]:
        return "candidate-timeout", ["candidate_timed_out"]

    is_cap_boundary = bool(case.get("capability_boundary", False))
    if is_cap_boundary:
        expected_cap = case.get("expected_capability")
        if (
            actual["exit_code"] != 0
            and len(actual["stderr"]) > 0
            and isinstance(capability_gap, dict)
            and capability_gap.get("capability_contract_verified") is True
            and capability_gap.get("expected_capability") == expected_cap
            and capability_gap.get("capability_reported_false") is True
            and capability_gap.get("error_code") == "capability-unavailable"
            and capability_gap.get("error_capability") == expected_cap
            and not candidate_assertion_failures
        ):
            return "capability-blocked", []
        reasons = list(candidate_assertion_failures or [])
        if actual["exit_code"] == 0:
            reasons.append("capability_boundary_unexpected_exit_0")
        if len(actual["stderr"]) == 0:
            reasons.append("capability_boundary_empty_stderr")
        reasons.append(f"capability_contract_unmet:{expected_cap}")
        return "mismatch", reasons

    # Normal required executable case: capability-unavailable is a mismatch (NEVER capability-blocked!)
    if isinstance(capability_gap, dict) and capability_gap.get("error_code") == "capability-unavailable":
        return "mismatch", [f"unexpected_capability_unavailable:{capability_gap.get('error_capability')}"]

    if candidate_assertion_failures:
        return "mismatch", list(candidate_assertion_failures)

    diffs = compat.differences(expected, actual)
    return ("mismatch", diffs) if diffs else ("pass", [])


def decode_js_capture(row: dict) -> dict:
    files = {k: base64.b64decode(v) for k, v in (row.get("files") or {}).items()}
    return {
        "stdout": base64.b64decode(row.get("stdout_b64", "")),
        "stderr": base64.b64decode(row.get("stderr_b64", "")),
        "exit_code": int(row.get("exit_code", 1)),
        "files": dict(sorted(files.items())),
        "timed_out": False,
        "workspace_changes": {},
        "elapsed_ns": int(row.get("elapsed_ns", 0)),
    }


def materialize_target_spec(root: Path, case: dict, placeholders: dict[str, str]) -> dict:
    raw_spec = expand_placeholders(case.get("target_spec") or {}, placeholders)
    spec = dict(raw_spec)
    spec["id"] = case["id"]

    # Load inline text/format/repl/vfs file contents from disk cleanly
    src_file = spec.pop("source_file", None)
    src_kind = spec.pop("source_kind", None)
    src_uri = spec.pop("source_uri", None)
    src_path = spec.pop("source_path", None)
    if src_kind == "text":
        text = Path(src_file).read_text(encoding="utf-8") if src_file else spec.pop("source_text", "")
        spec["source"] = {"kind": "text", "text": text, **({"uri": src_uri} if src_uri else {})}
    elif src_kind == "path":
        spec["source"] = {"kind": "path", "path": src_path or src_file}
    elif src_kind == "uri":
        spec["source"] = {"kind": "uri", "uri": src_uri}
    elif src_kind == "modulePath":
        spec["source"] = {"kind": "modulePath", "path": src_path}

    if "format_input_file" in spec:
        spec["formatInput"] = Path(spec.pop("format_input_file")).read_text(encoding="utf-8")
    if "repl_script_file" in spec:
        spec["replCommands"] = Path(spec.pop("repl_script_file")).read_text(encoding="utf-8").splitlines()
    if "fast_source_file" in spec:
        spec["fastSource"] = {
            "kind": "text",
            "text": Path(spec.pop("fast_source_file")).read_text(encoding="utf-8"),
        }
    if "slow_source_file" in spec:
        spec["slowSource"] = {
            "kind": "text",
            "text": Path(spec.pop("slow_source_file")).read_text(encoding="utf-8"),
        }

    # Resolve VFS files specified as `{vfs_path: disk_file_path}`
    opts = dict(spec.get("target_options") or {})
    if "vfs_file_sources" in opts:
        vfs_files = {}
        for vpath, disk_path in opts.pop("vfs_file_sources").items():
            vfs_files[vpath] = Path(disk_path).read_text(encoding="utf-8")
        opts["vfsFiles"] = vfs_files
    if "http" in opts and isinstance(opts["http"], dict):
        http_cfg = dict(opts["http"])
        if "caCertificateFiles" in http_cfg:
            http_cfg["caCertificates"] = [
                Path(p).read_text(encoding="utf-8") for p in http_cfg.pop("caCertificateFiles")
            ]
        opts["http"] = http_cfg
    if opts:
        spec["target_options"] = opts
    if "http_ca_cert_files" in spec:
        http_cfg = dict(spec.get("http") or {})
        http_cfg["caCertificates"] = [Path(p).read_text(encoding="utf-8") for p in spec.pop("http_ca_cert_files")]
        spec["http"] = http_cfg
    return spec


# ---------------------------------------------------------------------------
# Source Snapshot & Candidate Build / Pack / Clean Temp Install
# ---------------------------------------------------------------------------

def snapshot_sources(root: Path) -> dict[str, str]:
    """Hash authored Rust, JS/TS package, runner, and fixture sources (never build outputs)."""
    files: dict[str, str] = {}
    for rel in (
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "tools/platform/package.json",
        "tools/platform/package-lock.json",
        ORACLE_PROBE_JAVA.as_posix(),
    ):
        p = root / rel
        if p.is_file():
            files[rel] = oracle.sha256(p)

    for top in ("crates", "vendor"):
        base = root / top
        if not base.is_dir():
            continue
        for crate_dir in sorted(base.iterdir()):
            if not crate_dir.is_dir() or crate_dir.is_symlink():
                continue
            toml = crate_dir / "Cargo.toml"
            if toml.is_file():
                files[toml.relative_to(root).as_posix()] = oracle.sha256(toml)
            for sub in ("src", "assets", "examples"):
                sub_dir = crate_dir / sub
                if not sub_dir.is_dir():
                    continue
                for current, dirs, names in os.walk(sub_dir, followlinks=False):
                    dirs.sort()
                    for name in sorted(names):
                        p = Path(current) / name
                        if p.is_symlink() or not p.is_file():
                            continue
                        files[p.relative_to(root).as_posix()] = oracle.sha256(p)

    # Authored JS/TS package sources (excluding build output dirs dist/, native/, node_modules/)
    pkg_root = root / "packages"
    if pkg_root.is_dir():
        for pkg_dir in sorted(pkg_root.iterdir()):
            if not pkg_dir.is_dir() or pkg_dir.is_symlink():
                continue
            for rel_name in ("package.json", "README.md", "tsconfig.json", "index.d.ts"):
                p = pkg_dir / rel_name
                if p.is_file():
                    files[p.relative_to(root).as_posix()] = oracle.sha256(p)
            for sub in ("src", "scripts", "types"):
                sub_dir = pkg_dir / sub
                if not sub_dir.is_dir():
                    continue
                for current, dirs, names in os.walk(sub_dir, followlinks=False):
                    dirs.sort()
                    for name in sorted(names):
                        p = Path(current) / name
                        if p.is_symlink() or not p.is_file():
                            continue
                        files[p.relative_to(root).as_posix()] = oracle.sha256(p)

    runners = root / RUNNERS_DIR
    if runners.is_dir():
        for p in sorted(runners.rglob("*")):
            if p.is_file() and not p.is_symlink():
                files[p.relative_to(root).as_posix()] = oracle.sha256(p)

    return dict(sorted(files.items()))


def build_rust_bin(root: Path, package: str, bin_name: str, target_dir: Path, output: Path) -> tuple[Path, dict]:
    cmd = [
        "cargo", "build", "--release", "-p", package,
        "--bin", bin_name, "--locked", "--target-dir", str(target_dir),
    ]
    proc = subprocess.run(cmd, cwd=root, capture_output=True, timeout=600)
    (output / f"build-{bin_name}.stdout.bin").write_bytes(proc.stdout)
    (output / f"build-{bin_name}.stderr.bin").write_bytes(proc.stderr)
    rec = {
        "command": cmd,
        "package": package,
        "bin": bin_name,
        "exit_code": proc.returncode,
        "stdout_sha256": digest(proc.stdout),
        "stderr_sha256": digest(proc.stderr),
    }
    if proc.returncode != 0:
        raise ValueError(
            f"failed building {package}::{bin_name} (rc={proc.returncode}): "
            f"{proc.stderr.decode('utf-8', errors='replace')[-1200:]}"
        )
    built = target_dir / "release" / bin_name
    if not built.is_file():
        raise ValueError(f"built binary missing: {built}")
    return built, rec


def build_and_pack_npm_package(
    root: Path,
    pkg_rel: str,
    role_name: str,
    output: Path,
    install_base: Path,
) -> tuple[Path, Path, dict]:
    """Build an npm package via its scripts/build.mjs, `npm pack` it, seal the .tgz, and clean-install."""
    pkg_dir = (root / pkg_rel).resolve()
    if not (pkg_dir / "package.json").is_file():
        raise ValueError(f"missing package.json in {pkg_rel}")
    build_script = pkg_dir / "scripts/build.mjs"
    if not build_script.is_file():
        raise ValueError(f"missing build script: {pkg_rel}/scripts/build.mjs")

    build_cmd = ["node", str(build_script)]
    bproc = subprocess.run(build_cmd, cwd=pkg_dir, capture_output=True, timeout=600)
    (output / f"build-{role_name}.stdout.bin").write_bytes(bproc.stdout)
    (output / f"build-{role_name}.stderr.bin").write_bytes(bproc.stderr)
    if bproc.returncode != 0:
        raise ValueError(
            f"failed building {pkg_rel} (rc={bproc.returncode}): "
            f"{bproc.stderr.decode('utf-8', errors='replace')[-1200:]}"
        )

    pack_dir = output / "_pack" / role_name
    pack_dir.mkdir(parents=True, exist_ok=True)
    npm_cache = output / "_npm_cache"
    npm_cache.mkdir(parents=True, exist_ok=True)
    pack_env = {**os.environ, "npm_config_cache": str(npm_cache), "NPM_CONFIG_OFFLINE": "true"}
    pack_cmd = ["npm", "pack", "--pack-destination", str(pack_dir)]
    pproc = subprocess.run(pack_cmd, cwd=pkg_dir, env=pack_env, capture_output=True, timeout=180)
    (output / f"pack-{role_name}.stdout.bin").write_bytes(pproc.stdout)
    (output / f"pack-{role_name}.stderr.bin").write_bytes(pproc.stderr)
    if pproc.returncode != 0:
        raise ValueError(
            f"npm pack failed for {pkg_rel} (rc={pproc.returncode}): "
            f"{pproc.stderr.decode('utf-8', errors='replace')[-1200:]}"
        )
    tarballs = sorted(pack_dir.glob("*.tgz"))
    if len(tarballs) != 1:
        raise ValueError(f"expected 1 tarball from npm pack for {pkg_rel}, got {len(tarballs)}")
    tgz = tarballs[0]

    # Seal tarball into artifacts/
    artifacts = output / "artifacts"
    artifacts.mkdir(exist_ok=True)
    sealed_tgz = artifacts / f"{role_name}.tgz"
    sealed_tgz.write_bytes(tgz.read_bytes())

    # Clean-directory extract into isolated node_modules/<pkg-name>
    pkg_meta = json.loads((pkg_dir / "package.json").read_bytes())
    pkg_name = pkg_meta["name"]
    installed_root = install_base / role_name
    installed_pkg = installed_root / "node_modules" / pkg_name
    installed_pkg.mkdir(parents=True, exist_ok=True)
    tarball_members: list[str] = []
    with tarfile.open(sealed_tgz, "r:gz") as tf:
        for member in tf.getmembers():
            mpath = Path(member.name)
            if mpath.is_absolute() or ".." in mpath.parts:
                raise ValueError(f"unsafe tarball path in {role_name}: {member.name}")
            if member.issym() or member.islnk():
                raise ValueError(f"unexpected symlink in npm tarball {role_name}: {member.name}")
            if member.isfile():
                rel = mpath.relative_to("package") if mpath.parts[:1] == ("package",) else mpath
                tarball_members.append(rel.as_posix())
                dest = installed_pkg / rel
                dest.parent.mkdir(parents=True, exist_ok=True)
                extracted = tf.extractfile(member)
                assert extracted is not None
                dest.write_bytes(extracted.read())
    (installed_root / "package.json").write_text(
        json.dumps({"name": f"m6-clean-{role_name}", "private": True, "type": "module"}) + "\n",
        encoding="utf-8",
    )

    rec = {
        "name": sealed_tgz.name,
        "package_name": pkg_name,
        "package_version": pkg_meta.get("version"),
        "sha256": oracle.sha256(sealed_tgz),
        "source_binding": "recorded-build",
        "tarball_files": sorted(tarball_members),
        "build": {
            "role": role_name,
            "command": build_cmd,
            "exit_code": bproc.returncode,
            "stdout_sha256": digest(bproc.stdout),
            "stderr_sha256": digest(bproc.stderr),
        },
        "pack": {
            "command": pack_cmd,
            "exit_code": pproc.returncode,
            "stdout_sha256": digest(pproc.stdout),
            "stderr_sha256": digest(pproc.stderr),
        },
    }
    return sealed_tgz, installed_pkg, rec


def freeze_binary(artifacts_dir: Path, name: str, path: Path, source_binding: str, build_rec: dict | None) -> tuple[Path, dict]:
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


def validate_cases(root: Path, cases_doc: dict) -> dict[str, dict]:
    if cases_doc.get("schema_version") != 1 or not isinstance(cases_doc.get("cases"), list):
        raise ValueError("invalid tests/fixtures/m6/cases.json schema")
    by_id: dict[str, dict] = {}
    for case in cases_doc["cases"]:
        cid = case["id"]
        if cid in by_id:
            raise ValueError(f"duplicate M6 case id: {cid}")
        if case.get("lane") not in ALL_LANES:
            raise ValueError(f"unknown M6 lane for {cid}: {case.get('lane')}")
        if case.get("exec_mode") not in VALID_EXEC_MODES:
            raise ValueError(f"unknown M6 exec_mode for {cid}: {case.get('exec_mode')}")
        if case.get("oracle_mode") not in VALID_ORACLE_MODES:
            raise ValueError(f"unknown M6 oracle_mode for {cid}: {case.get('oracle_mode')}")
        is_cap = bool(case.get("capability_boundary", False))
        if is_cap and not case.get("expected_capability"):
            raise ValueError(f"capability_boundary case {cid} missing expected_capability")
        if not is_cap and case.get("expected_capability") is not None:
            raise ValueError(f"non-boundary case {cid} must not set expected_capability")
        for inp in case.get("inputs", []):
            rel = inp["path"]
            p = Path(rel)
            if p.is_absolute() or ".." in p.parts:
                raise ValueError(f"invalid M6 fixture input path: {rel}")
            full = root / p
            if not full.is_file():
                raise ValueError(f"missing M6 fixture input: {rel}")
            if digest(full.read_bytes()) != inp["sha256"]:
                raise ValueError(f"M6 fixture input sha256 mismatch: {rel}")
        by_id[cid] = case
    return by_id


def required_candidate_roles(lanes: list[str]) -> list[str]:
    roles: set[str] = set()
    for lane in lanes:
        if lane in ("wasm-browser", "wasm-node"):
            roles.add("wasm_pkg")
        elif lane == "napi-node":
            roles.add("napi_pkg")
        elif lane == "cross-target-parity":
            roles.update(("pkl", "wasm_pkg", "napi_pkg"))
        elif lane == "packaging-and-install":
            roles.update(("pkl", "pkldoc", "wasm_pkg", "napi_pkg"))
    return sorted(roles)


def run_js_driver(
    driver_script: Path,
    plan: dict,
    work_dir: Path,
    timeout: float,
) -> dict:
    plan_path = work_dir / f"{driver_script.stem}.plan.json"
    result_path = work_dir / f"{driver_script.stem}.result.json"
    compat.write_json(plan_path, plan)
    proc = subprocess.run(
        ["node", str(driver_script), str(plan_path), str(result_path)],
        cwd=ROOT,
        capture_output=True,
        timeout=timeout,
    )
    if proc.returncode != 0 or not result_path.is_file():
        raise RuntimeError(
            f"{driver_script.name} failed (rc={proc.returncode}): "
            f"{proc.stderr.decode('utf-8', errors='replace')[-1500:]}"
        )
    return json.loads(result_path.read_bytes())


def run_downstream_rust_crate_check(
    root: Path,
    work_dir: Path,
    sample_pkl: Path,
    target_dir: Path,
    timeout: float,
) -> tuple[dict, list[str]]:
    """Build & execute a clean standalone Rust consumer crate depending on `crates/pkl`."""
    crate_dir = work_dir / "downstream_pkl_consumer"
    (crate_dir / "src").mkdir(parents=True, exist_ok=True)
    pkl_crate_path = (root / "crates/pkl").resolve()
    java_regex_path = (root / "vendor/java_regex").resolve()
    cargo_toml = (
        '[package]\nname = "m6-downstream-consumer"\nversion = "0.1.0"\nedition = "2024"\n\n'
        '[workspace]\n\n'
        f'[dependencies]\npkl = {{ path = "{pkl_crate_path}" }}\n\n'
        f'[patch.crates-io]\njava_regex = {{ path = "{java_regex_path}" }}\n'
    )
    main_rs = (
        'use pkl::{EvaluatorBuilder, ModuleSource};\n'
        'fn main() {\n'
        '    let path = std::env::args().nth(1).expect("sample pkl path");\n'
        '    let mut ev = EvaluatorBuilder::preconfigured()\n'
        '        .with_module_cache_dir(None)\n'
        '        .with_external_properties(Vec::new())\n'
        '        .with_environment_variables(Vec::new())\n'
        '        .build()\n'
        '        .expect("build evaluator");\n'
        '    let out = ev.evaluate_output_text(&ModuleSource::path(&path)).expect("eval");\n'
        '    print!("{out}");\n'
        '    ev.close();\n'
        '}\n'
    )
    (crate_dir / "Cargo.toml").write_text(cargo_toml, encoding="utf-8")
    (crate_dir / "src/main.rs").write_text(main_rs, encoding="utf-8")
    shutil.copyfile(root / "Cargo.lock", crate_dir / "Cargo.lock")

    env = dict(os.environ)
    bproc = subprocess.run(
        ["cargo", "build", "--release", "--offline", "--target-dir", str(target_dir / "m6-downstream-target")],
        cwd=crate_dir,
        env=env,
        capture_output=True,
        timeout=300,
    )
    if bproc.returncode != 0:
        return {
            "stdout": bproc.stdout,
            "stderr": bproc.stderr,
            "exit_code": bproc.returncode,
            "files": {},
            "timed_out": False,
            "workspace_changes": {},
            "elapsed_ns": 0,
        }, ["downstream_rust_crate_build_failed"]

    bin_path = target_dir / "m6-downstream-target/release/m6-downstream-consumer"
    raw = capture_process([str(bin_path), str(sample_pkl.resolve())], crate_dir, env, timeout)
    return raw, []


def execute(
    root: Path,
    output: Path,
    lanes: list[str],
    timeout: float,
    oracle_only: bool,
    cargo_target_dir: Path | None,
) -> dict:
    cases_doc = json.loads((root / CASES_PATH).read_bytes())
    validate_cases(root, cases_doc)
    selected = [c for c in cases_doc["cases"] if c["lane"] in lanes]

    if output.exists() or output.is_symlink():
        raise ValueError(f"output directory must be fresh (does not exist): {output}")

    oracle_meta = oracle.ensure_oracle(root, fetch=False)
    if not oracle_meta.get("exact_jdk_lock_match", False):
        raise ValueError("pinned oracle JDK lock mismatch (exact_jdk_lock_match is false)")

    sources_before = snapshot_sources(root)

    output.mkdir(parents=True, exist_ok=False)
    artifacts = output / "artifacts"
    artifacts.mkdir()
    compat.write_json(output / "oracle.json", oracle_meta)
    (output / "cases.snapshot.json").write_bytes((root / CASES_PATH).read_bytes())

    needed_roles = required_candidate_roles(lanes)
    tdir = cargo_target_dir.resolve() if cargo_target_dir else (root / "target")
    work_root = output / "_work"
    work_root.mkdir()
    install_base = work_root / "clean_install"
    install_base.mkdir()

    candidate_bindings: dict[str, dict] | None = None
    pkl_bin: Path | None = None
    pkldoc_bin: Path | None = None
    installed_wasm_pkg: Path | None = None
    installed_napi_pkg: Path | None = None

    if not oracle_only:
        candidate_bindings = {}
        if "pkl" in needed_roles:
            built, rec = build_rust_bin(root, DEFAULT_PKL_PACKAGE, DEFAULT_PKL_BIN, tdir, output)
            pkl_bin, candidate_bindings["pkl"] = freeze_binary(artifacts, DEFAULT_PKL_BIN, built, "recorded-build", rec)
        if "pkldoc" in needed_roles:
            built, rec = build_rust_bin(root, DEFAULT_PKLDOC_PACKAGE, DEFAULT_PKLDOC_BIN, tdir, output)
            pkldoc_bin, candidate_bindings["pkldoc"] = freeze_binary(artifacts, DEFAULT_PKLDOC_BIN, built, "recorded-build", rec)
        if "wasm_pkg" in needed_roles:
            _, installed_wasm_pkg, candidate_bindings["wasm_pkg"] = build_and_pack_npm_package(
                root, "packages/pkl-wasm", "wasm_pkg", output, install_base
            )
        if "napi_pkg" in needed_roles:
            _, installed_napi_pkg, candidate_bindings["napi_pkg"] = build_and_pack_npm_package(
                root, "packages/pkl-napi", "napi_pkg", output, install_base
            )

    probe_classes = compile_oracle_probe_java(oracle_meta, work_root / "probe_compile", root)

    need_http = any(c.get("requires_http_server") for c in selected)
    server_self_test: dict | None = None
    browser_provenance: dict | None = None
    conversion_telemetry: dict[str, dict] = {}
    results: list[dict] = []

    def _run_all(servers: m4_differential.M4FixtureServers | None) -> None:
        nonlocal browser_provenance

        # Pre-prepare sandboxes and run pinned Java 0.32.0 oracle for every selected case
        # plus collect JS batch plans for wasm-browser / wasm-node / napi-node!
        prepared: dict[str, dict] = {}
        wasm_browser_plan_cases: list[dict] = []
        wasm_node_plan_cases: list[dict] = []
        napi_node_plan_cases: list[dict] = []

        for case in selected:
            cid = case["id"]
            cdir_name = compat.case_directory(cid)
            folder = output / "cases" / cdir_name
            folder.mkdir(parents=True)
            cwork_root = work_root / "cases" / cdir_name
            case_state = cwork_root / "state"
            sandbox = cwork_root / "sandbox"
            case_gen = sandbox / "generated"
            case_work = sandbox / "work"
            case_cache = sandbox / "cache"
            lifecycle_log = sandbox / "ext_lifecycle.jsonl"

            placeholders = {
                "output_dir": str(case_gen),
                "work_dir": str(case_work),
                "cache_dir": str(case_cache),
                "reader_bin": str((root / M4_READER_BIN).resolve()),
                "lifecycle_log": str(lifecycle_log),
                "root": str(root),
            }
            if servers is not None:
                placeholders.update({
                    "http_port": str(servers.http_port),
                    "https_port": str(servers.https_port),
                    "proxy_port": str(servers.proxy_port),
                    "ca_cert": str(servers.ca_path),
                })

            apply_case_setup(root, case.get("setup"), case_work, case_gen, case_cache, case_state / "home", placeholders)
            env = build_case_env(oracle_meta["environment"], case.get("environment", {}), case_state)
            cwd_raw = expand_placeholders(case.get("cwd", "."), placeholders)
            cwd = Path(cwd_raw) if Path(cwd_raw).is_absolute() else (root / cwd_raw).resolve()

            log_off = servers.log_offset() if servers is not None else 0
            oargs = expand_placeholders(case["oracle_command"], placeholders)
            if case["oracle_mode"] == "probe":
                java_cmd = [*java_probe_prefix(oracle_meta, probe_classes, case_state), *oargs]
            else:
                java_cmd = [*java_prefix_for_state(oracle_meta["command"], case_state), *oargs]

            stdin_bytes = None
            if "oracle_stdin_file" in case:
                stdin_bytes = compat.relative_path(root, case["oracle_stdin_file"]).read_bytes()

            java_raw = capture_process(java_cmd, cwd, env, timeout, stdin_bytes=stdin_bytes)
            if case.get("collect_files") == "output_dir":
                java_raw["files"] = m4_differential.collect_tree_bytes(case_gen, set(), prefix="generated/")
            java_events = servers.read_log_since(log_off) if servers is not None else []
            java_afails = check_case_assertions(case, java_raw, lifecycle_log, java_events, is_java=True)
            java_stored = compat.store_capture(folder / "java", java_raw)

            # Re-wipe sandbox cleanly before candidate execution so zero state leaks from Java!
            shutil.rmtree(case_state, ignore_errors=True)
            shutil.rmtree(sandbox, ignore_errors=True)
            apply_case_setup(root, case.get("setup"), case_work, case_gen, case_cache, case_state / "home", placeholders)

            prepared[cid] = {
                "case": case,
                "folder": folder,
                "cdir_name": cdir_name,
                "case_state": case_state,
                "sandbox": sandbox,
                "case_gen": case_gen,
                "case_work": case_work,
                "case_cache": case_cache,
                "lifecycle_log": lifecycle_log,
                "placeholders": placeholders,
                "cwd": cwd,
                "env": build_case_env(oracle_meta["environment"], case.get("environment", {}), case_state),
                "java_cmd": java_cmd,
                "java_raw": java_raw,
                "java_stored": java_stored,
                "java_afails": java_afails,
                "java_events": java_events,
                "stdin_bytes": stdin_bytes,
            }

            if not oracle_only:
                emode = case["exec_mode"]
                if emode == "js_api":
                    tspec = materialize_target_spec(root, case, placeholders)
                    if case["target"] == "wasm-browser":
                        wasm_browser_plan_cases.append(tspec)
                    elif case["target"] == "wasm-node":
                        wasm_node_plan_cases.append(tspec)
                    elif case["target"] == "napi-node":
                        napi_node_plan_cases.append(tspec)
                elif emode == "cross_target_parity":
                    tspec = materialize_target_spec(root, case, placeholders)
                    wasm_browser_plan_cases.append(tspec)
                    wasm_node_plan_cases.append(tspec)
                    napi_node_plan_cases.append(tspec)

        # Execute batched JS/Browser candidate runners (in full differential mode),
        # slicing M4FixtureServers logs strictly per candidate batch (never including Java/self_test!)
        browser_rows: dict[str, dict] = {}
        wasm_node_rows: dict[str, dict] = {}
        napi_node_rows: dict[str, dict] = {}
        browser_server_events: list[dict] = []
        wasm_node_server_events: list[dict] = []
        napi_node_server_events: list[dict] = []

        if not oracle_only:
            if wasm_browser_plan_cases:
                assert installed_wasm_pkg is not None
                off = servers.log_offset() if servers else 0
                b_out = run_js_driver(
                    root / RUNNERS_DIR / "run_wasm_browser.mjs",
                    {
                        "repo_root": str(root),
                        "installed_wasm_pkg_dir": str(installed_wasm_pkg),
                        "m4_http_port": servers.http_port if servers else 0,
                        "cases": wasm_browser_plan_cases,
                    },
                    work_root,
                    max(timeout * 2, 180.0),
                )
                browser_server_events = servers.read_log_since(off) if servers else []
                browser_provenance = {k: v for k, v in b_out.items() if k != "results"}
                for row in b_out["results"]:
                    browser_rows[row["id"]] = row

            if wasm_node_plan_cases:
                assert installed_wasm_pkg is not None
                off = servers.log_offset() if servers else 0
                wn_out = run_js_driver(
                    root / RUNNERS_DIR / "run_wasm_node.mjs",
                    {
                        "repo_root": str(root),
                        "installed_wasm_pkg_dir": str(installed_wasm_pkg),
                        "m4_http_port": servers.http_port if servers else 0,
                        "cases": wasm_node_plan_cases,
                    },
                    work_root,
                    max(timeout * 2, 180.0),
                )
                wasm_node_server_events = servers.read_log_since(off) if servers else []
                for row in wn_out["results"]:
                    wasm_node_rows[row["id"]] = row

            if napi_node_plan_cases:
                assert installed_napi_pkg is not None
                off = servers.log_offset() if servers else 0
                nn_out = run_js_driver(
                    root / RUNNERS_DIR / "run_napi_node.mjs",
                    {
                        "repo_root": str(root),
                        "installed_napi_pkg_dir": str(installed_napi_pkg),
                        "cases": napi_node_plan_cases,
                    },
                    work_root,
                    max(timeout * 2, 180.0),
                )
                napi_node_server_events = servers.read_log_since(off) if servers else []
                for row in nn_out["results"]:
                    napi_node_rows[row["id"]] = row

        # Finalize captures & classifications per case
        for case in selected:
            cid = case["id"]
            p = prepared[cid]
            folder = p["folder"]
            emode = case["exec_mode"]
            cand_raw: dict | None = None
            cand_stored: dict | None = None
            cand_afails: list[str] = []
            cap_gap: dict | None = None
            telemetry: dict | None = None
            cross_caps: dict[str, dict] | None = None
            cross_stored: dict[str, dict] | None = None
            per_target_matches: dict[str, bool] | None = None
            shared_engine_match: bool | None = None

            # Always seal Java assertion metadata inside evidence directory
            compat.write_json(
                folder / "java/assertions.json",
                {"java_assertion_failures": p["java_afails"]},
            )

            if not oracle_only:
                if emode == "js_api":
                    target_map = {
                        "wasm-browser": browser_rows,
                        "wasm-node": wasm_node_rows,
                        "napi-node": napi_node_rows,
                    }[case["target"]]
                    target_events = {
                        "wasm-browser": browser_server_events,
                        "wasm-node": wasm_node_server_events,
                        "napi-node": napi_node_server_events,
                    }[case["target"]]
                    jrow = target_map[cid]
                    cand_raw = decode_js_capture(jrow)
                    cap_gap = jrow.get("capability_gap")
                    telemetry = jrow.get("telemetry")
                    if telemetry is not None:
                        conversion_telemetry[cid] = telemetry
                    cand_afails = check_case_assertions(
                        case,
                        cand_raw,
                        p["lifecycle_log"],
                        target_events,
                        is_java=False,
                        extra_assertions=jrow.get("assertions") or [],
                        telemetry=telemetry,
                    )
                    cand_stored = compat.store_capture(folder / "candidate", cand_raw)
                    compat.write_json(
                        folder / "candidate/runner_meta.json",
                        {
                            "candidate_assertion_failures": cand_afails,
                            "capability_gap": cap_gap,
                            "telemetry": telemetry,
                        },
                    )

                elif emode == "cross_target_parity":
                    assert pkl_bin is not None
                    n_args = expand_placeholders(case["native_command"], p["placeholders"])
                    native_raw = capture_process(
                        [str(pkl_bin), *n_args],
                        p["cwd"],
                        p["env"],
                        timeout,
                        stdin_bytes=p["stdin_bytes"],
                    )
                    if case.get("collect_files") == "output_dir":
                        native_raw["files"] = m4_differential.collect_tree_bytes(p["case_gen"], set(), prefix="generated/")
                    cross_caps = {
                        "native": native_raw,
                        "napi_node": decode_js_capture(napi_node_rows[cid]),
                        "wasm_node": decode_js_capture(wasm_node_rows[cid]),
                        "wasm_browser": decode_js_capture(browser_rows[cid]),
                    }
                    cross_stored = {
                        ep: compat.store_capture(folder / ep, cap) for ep, cap in cross_caps.items()
                    }
                    for ep, src_row in (
                        ("napi_node", napi_node_rows[cid]),
                        ("wasm_node", wasm_node_rows[cid]),
                        ("wasm_browser", browser_rows[cid]),
                    ):
                        for a in src_row.get("assertions") or []:
                            cand_afails.append(f"{ep}:{a}")
                    compat.write_json(
                        folder / "cross_target_meta.json",
                        {"candidate_assertion_failures": cand_afails},
                    )
                    per_target_matches = {
                        ep: len(compat.differences(p["java_raw"], cap)) == 0
                        for ep, cap in cross_caps.items()
                    }
                    shared_engine_match = all(
                        len(compat.differences(cross_caps["native"], cross_caps[ep])) == 0
                        for ep in ("napi_node", "wasm_node", "wasm_browser")
                    )

                elif emode == "packaging_native":
                    assert pkl_bin is not None and pkldoc_bin is not None
                    ver_pkl = capture_process([str(pkl_bin), "--version"], p["cwd"], p["env"], timeout)
                    ver_doc = capture_process([str(pkldoc_bin), "--version"], p["cwd"], p["env"], timeout)
                    extra_a = []
                    if ver_pkl["exit_code"] != 0 or not NATIVE_VERSION_RE.match(ver_pkl["stdout"]):
                        extra_a.append("pkl_native_version_invalid")
                    if ver_doc["exit_code"] != 0 or not NATIVE_VERSION_RE.match(ver_doc["stdout"]):
                        extra_a.append("pkldoc_native_version_invalid")
                    sample = compat.relative_path(root, case["sample_pkl"])
                    cand_raw = capture_process(
                        [str(pkl_bin), "eval", "--no-project", "--color=never", str(sample.resolve())],
                        p["cwd"],
                        p["env"],
                        timeout,
                    )
                    cand_afails = check_case_assertions(case, cand_raw, p["lifecycle_log"], [], is_java=False, extra_assertions=extra_a)
                    cand_stored = compat.store_capture(folder / "candidate", cand_raw)
                    compat.write_json(
                        folder / "candidate/runner_meta.json",
                        {"candidate_assertion_failures": cand_afails, "capability_gap": None, "telemetry": None},
                    )

                elif emode == "packaging_rust_crate":
                    sample = compat.relative_path(root, case["sample_pkl"])
                    cand_raw, extra_a = run_downstream_rust_crate_check(root, p["case_work"], sample, tdir, timeout)
                    cand_afails = check_case_assertions(case, cand_raw, p["lifecycle_log"], [], is_java=False, extra_assertions=extra_a)
                    cand_stored = compat.store_capture(folder / "candidate", cand_raw)
                    compat.write_json(
                        folder / "candidate/runner_meta.json",
                        {"candidate_assertion_failures": cand_afails, "capability_gap": None, "telemetry": None},
                    )

                elif emode in ("packaging_wasm", "packaging_napi", "packaging_matrix"):
                    sample = compat.relative_path(root, case["sample_pkl"])
                    mode_arg = {
                        "packaging_wasm": "wasm-node",
                        "packaging_napi": "napi-esm-cjs",
                        "packaging_matrix": "matrix",
                    }[emode]
                    target_dir_arg = {
                        "packaging_wasm": str(install_base / "wasm_pkg/node_modules/@pkl-rs/wasm"),
                        "packaging_napi": str(install_base / "napi_pkg/node_modules/@pkl-rs/napi"),
                        "packaging_matrix": str(root),
                    }[emode]
                    res_json = p["case_work"] / f"pack_{emode}.json"
                    if emode in ("packaging_wasm", "packaging_napi"):
                        role_dir = install_base / ("wasm_pkg" if emode == "packaging_wasm" else "napi_pkg")
                        staged_script = role_dir / "run_packaging_check.mjs"
                        staged_script.write_bytes((root / RUNNERS_DIR / "run_packaging_check.mjs").read_bytes())
                        script_to_run = staged_script
                        run_cwd = role_dir
                    else:
                        script_to_run = root / RUNNERS_DIR / "run_packaging_check.mjs"
                        run_cwd = root
                    proc = subprocess.run(
                        [
                            "node",
                            str(script_to_run),
                            mode_arg,
                            target_dir_arg,
                            str(sample.resolve()),
                            str(res_json),
                        ],
                        cwd=run_cwd,
                        capture_output=True,
                        timeout=timeout,
                    )
                    if proc.returncode != 0 or not res_json.is_file():
                        cand_raw = {
                            "stdout": proc.stdout,
                            "stderr": proc.stderr,
                            "exit_code": proc.returncode or 1,
                            "files": {},
                            "timed_out": False,
                            "workspace_changes": {},
                            "elapsed_ns": 0,
                        }
                        extra_a = [f"packaging_check_failed_rc_{proc.returncode}"]
                    else:
                        parsed = json.loads(res_json.read_bytes())
                        cand_raw = {
                            "stdout": base64.b64decode(parsed["stdout_b64"]),
                            "stderr": b"",
                            "exit_code": 0,
                            "files": {},
                            "timed_out": False,
                            "workspace_changes": {},
                            "elapsed_ns": 0,
                        }
                        extra_a = list(parsed.get("assertions") or [])
                    # For packaging_wasm, ALSO verify real Chromium execution of the clean-installed tarball
                    # and preserve Chromium provenance even when packaging-and-install runs standalone!
                    if emode == "packaging_wasm" and not extra_a:
                        assert installed_wasm_pkg is not None
                        wb = run_js_driver(
                            root / RUNNERS_DIR / "run_wasm_browser.mjs",
                            {
                                "repo_root": str(root),
                                "installed_wasm_pkg_dir": str(installed_wasm_pkg),
                                "m4_http_port": 0,
                                "cases": [{
                                    "id": cid,
                                    "action": "eval_text",
                                    "source": {"kind": "text", "text": sample.read_text(encoding="utf-8")},
                                }],
                            },
                            p["case_work"],
                            timeout,
                        )
                        if browser_provenance is None:
                            browser_provenance = {k: v for k, v in wb.items() if k != "results"}
                        wb_cap = decode_js_capture(wb["results"][0])
                        if compat.differences(cand_raw, wb_cap):
                            extra_a.append("packed_wasm_browser_differs_from_node")
                    cand_afails = check_case_assertions(case, cand_raw, p["lifecycle_log"], [], is_java=False, extra_assertions=extra_a)
                    cand_stored = compat.store_capture(folder / "candidate", cand_raw)
                    compat.write_json(
                        folder / "candidate/runner_meta.json",
                        {"candidate_assertion_failures": cand_afails, "capability_gap": None, "telemetry": None},
                    )

            status, diffs = classify_result(
                case,
                p["java_raw"],
                cand_raw,
                java_assertion_failures=p["java_afails"],
                candidate_assertion_failures=cand_afails,
                capability_gap=cap_gap,
                cross_target_captures=cross_caps,
            )

            results.append({
                "id": cid,
                "lane": case["lane"],
                "target": case["target"],
                "exec_mode": emode,
                "directory": p["cdir_name"],
                "status": status,
                "differences": diffs,
                "expected_exit": case["expected_exit"],
                "capability_boundary": bool(case.get("capability_boundary", False)),
                "expected_capability": case.get("expected_capability"),
                "capability_gap": cap_gap,
                "java_assertion_failures": p["java_afails"],
                "candidate_assertion_failures": cand_afails,
                "telemetry": telemetry,
                "per_target_oracle_matches": per_target_matches,
                "shared_engine_match": shared_engine_match,
                "java": {"command": p["java_cmd"], "result": p["java_stored"]},
                "candidate": None if cand_stored is None else {"result": cand_stored},
                "cross_target": cross_stored,
            })

    if need_http:
        with m4_differential.M4FixtureServers(
            oracle_meta,
            root / "tests/fixtures/m4/host_io",
            root / "tests/fixtures/m4/projects/registry",
            output / "_server",
        ) as servers:
            server_self_test = servers.self_test()
            if not server_self_test["passed"]:
                raise RuntimeError("M4HttpFixtureServer self-test failed: " + json.dumps(server_self_test))
            _run_all(servers)
        shutil.rmtree(output / "_server", ignore_errors=True)
    else:
        _run_all(None)

    shutil.rmtree(work_root, ignore_errors=True)
    shutil.rmtree(output / "_pack", ignore_errors=True)
    shutil.rmtree(output / "_npm_cache", ignore_errors=True)

    sources_after = snapshot_sources(root)
    source_stability = sources_before == sources_after
    compat.write_json(output / "sources.snapshot.json", {"before": sources_before, "after": sources_after})
    if browser_provenance is not None:
        compat.write_json(output / "browser_provenance.json", browser_provenance)

    by_status = dict(sorted(Counter(r["status"] for r in results).items()))
    by_lane_status = {
        lane: dict(sorted(Counter(r["status"] for r in results if r["lane"] == lane).items()))
        for lane in lanes
    }
    exec_cases = [r for r in results if not r["capability_boundary"]]
    cap_cases = [r for r in results if r["capability_boundary"]]

    cross_rows = [r for r in results if r["exec_mode"] == "cross_target_parity"]
    cross_summary = None
    if cross_rows:
        cross_summary = {
            "total": len(cross_rows),
            "per_target_passed": (
                None
                if oracle_only
                else {
                    ep: sum(1 for r in cross_rows if (r.get("per_target_oracle_matches") or {}).get(ep) is True)
                    for ep in CROSS_TARGET_ENDPOINTS
                }
            ),
            "shared_engine_passed": (
                None if oracle_only else sum(1 for r in cross_rows if r.get("shared_engine_match") is True)
            ),
            "all_targets_passed": (
                None if oracle_only else sum(1 for r in cross_rows if r["status"] == "pass")
            ),
        }

    report = {
        "schema_version": 1,
        "baseline": cases_doc["baseline"],
        "mode": "java-only-oracle-recording" if oracle_only else "differential",
        "cases_manifest_sha256": oracle.sha256(root / CASES_PATH),
        "oracle_probe_java_sha256": oracle.sha256(root / ORACLE_PROBE_JAVA),
        "sources_snapshot_sha256": oracle.sha256(output / "sources.snapshot.json"),
        "sources_file_count": len(sources_before),
        "source_stability": source_stability,
        "lanes": lanes,
        "required_candidate_roles": needed_roles,
        "timeout_seconds": timeout,
        "oracle": oracle_meta,
        "candidates": candidate_bindings,
        "browser_provenance": browser_provenance,
        "server_self_test": server_self_test,
        "selected_count": len(results),
        "executable_case_count": len(exec_cases),
        "capability_boundary_case_count": len(cap_cases),
        "compatibility_pass_count": sum(1 for r in results if r["status"] == "pass"),
        "capability_blocked_count": sum(1 for r in results if r["status"] == "capability-blocked"),
        "status_counts": by_status,
        "lane_status_counts": by_lane_status,
        "cross_target_summary": cross_summary,
        "conversion_telemetry": conversion_telemetry,
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
        raise ValueError("unsupported M6 differential report schema_version")
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

    cands = report.get("candidates")
    if cands is not None:
        if not isinstance(cands, dict):
            raise ValueError("candidates metadata must be an object or null")
        for role, info in cands.items():
            if not isinstance(info, dict):
                raise ValueError(f"candidate entry for {role} must be an object")
            cname = info.get("name")
            if not isinstance(cname, str) or not cname or "/" in cname or "\\" in cname or cname in (".", ".."):
                raise ValueError(f"invalid candidate artifact name for {role}: {cname!r}")
            rel = f"artifacts/{cname}"
            if rel not in recorded_evidence or info.get("sha256") != recorded_evidence[rel]:
                raise ValueError(f"candidate artifact sha256 mismatch for {role}: {rel}")
            build_rec = info.get("build")
            if isinstance(build_rec, dict):
                bname = build_rec.get("bin") or build_rec.get("role")
                if isinstance(bname, str) and bname:
                    for stream, key in (("stdout", "stdout_sha256"), ("stderr", "stderr_sha256")):
                        brel = f"build-{bname}.{stream}.bin"
                        if brel not in recorded_evidence or build_rec.get(key) != recorded_evidence[brel]:
                            raise ValueError(f"candidate build {stream} sha256 mismatch for {role}: {brel}")
            pack_rec = info.get("pack")
            if isinstance(pack_rec, dict):
                for stream, key in (("stdout", "stdout_sha256"), ("stderr", "stderr_sha256")):
                    prel = f"pack-{role}.{stream}.bin"
                    if prel not in recorded_evidence or pack_rec.get(key) != recorded_evidence[prel]:
                        raise ValueError(f"candidate pack {stream} sha256 mismatch for {role}: {prel}")

    bp_report = report.get("browser_provenance")
    bp_file = directory / "browser_provenance.json"
    if bp_report is not None:
        if not bp_file.is_file() or json.loads(bp_file.read_bytes()) != bp_report:
            raise ValueError("browser_provenance.json missing or differs from report.json")
    elif bp_file.is_file():
        raise ValueError("unexpected browser_provenance.json when report.browser_provenance is null")

    cases_doc = json.loads((directory / "cases.snapshot.json").read_bytes())
    cases_by_id = {c["id"]: c for c in cases_doc["cases"]}
    expected_ids = [c["id"] for c in cases_doc["cases"] if c["lane"] in report["lanes"]]
    result_ids = [r["id"] for r in report["results"]]
    if result_ids != expected_ids or len(result_ids) != report["selected_count"]:
        raise ValueError("result list order or membership differs from selected lanes")

    recomputed_telemetry: dict[str, dict] = {}
    for row in report["results"]:
        case = cases_by_id[row["id"]]
        expected_dir = compat.case_directory(row["id"])
        if row["directory"] != expected_dir:
            raise ValueError(f"unexpected capture directory for {row['id']}")
        if bool(row.get("capability_boundary", False)) != bool(case.get("capability_boundary", False)):
            raise ValueError(f"capability_boundary flag tampered for {row['id']}")
        if row.get("expected_capability") != case.get("expected_capability"):
            raise ValueError(f"expected_capability tampered for {row['id']}")

        folder = directory / "cases" / expected_dir
        java_cap = compat.load_capture(folder / "java")

        # Cross-check sealed Java assertions file
        java_assert_path = folder / "java/assertions.json"
        if not java_assert_path.is_file():
            raise ValueError(f"missing sealed java/assertions.json for {row['id']}")
        sealed_java_asserts = json.loads(java_assert_path.read_bytes())
        if sealed_java_asserts.get("java_assertion_failures") != row.get("java_assertion_failures"):
            raise ValueError(f"java_assertion_failures tampered for {row['id']}")

        cand_cap = compat.load_capture(folder / "candidate") if row.get("candidate") is not None else None
        if cand_cap is not None:
            meta_path = folder / "candidate/runner_meta.json"
            if not meta_path.is_file():
                raise ValueError(f"missing sealed candidate/runner_meta.json for {row['id']}")
            sealed_meta = json.loads(meta_path.read_bytes())
            if sealed_meta.get("candidate_assertion_failures") != row.get("candidate_assertion_failures"):
                raise ValueError(f"candidate_assertion_failures tampered for {row['id']}")
            if sealed_meta.get("capability_gap") != row.get("capability_gap"):
                raise ValueError(f"capability_gap tampered for {row['id']}")
            if sealed_meta.get("telemetry") != row.get("telemetry"):
                raise ValueError(f"telemetry tampered for {row['id']}")
            if sealed_meta.get("telemetry") is not None:
                recomputed_telemetry[row["id"]] = sealed_meta["telemetry"]
        else:
            if row.get("capability_gap") is not None or row.get("telemetry") is not None:
                raise ValueError(f"unexpected capability_gap/telemetry without candidate capture for {row['id']}")

        cross_caps = None
        if row.get("cross_target") is not None:
            ct_meta_path = folder / "cross_target_meta.json"
            if not ct_meta_path.is_file():
                raise ValueError(f"missing sealed cross_target_meta.json for {row['id']}")
            sealed_ct = json.loads(ct_meta_path.read_bytes())
            if sealed_ct.get("candidate_assertion_failures") != row.get("candidate_assertion_failures"):
                raise ValueError(f"cross-target candidate_assertion_failures tampered for {row['id']}")
            cross_caps = {ep: compat.load_capture(folder / ep) for ep in CROSS_TARGET_ENDPOINTS}
            recomp_per_target = {
                ep: len(compat.differences(java_cap, cap)) == 0 for ep, cap in cross_caps.items()
            }
            if row.get("per_target_oracle_matches") != recomp_per_target:
                raise ValueError(f"recomputed per_target_oracle_matches disagrees for {row['id']}")
            recomp_shared = all(
                len(compat.differences(cross_caps["native"], cross_caps[ep])) == 0
                for ep in ("napi_node", "wasm_node", "wasm_browser")
            )
            if row.get("shared_engine_match") != recomp_shared:
                raise ValueError(f"recomputed shared_engine_match disagrees for {row['id']}")
        elif cand_cap is None and row.get("candidate_assertion_failures"):
            raise ValueError(f"unexpected candidate_assertion_failures without candidate capture for {row['id']}")

        status, diffs = classify_result(
            case,
            java_cap,
            cand_cap,
            java_assertion_failures=sealed_java_asserts.get("java_assertion_failures"),
            candidate_assertion_failures=row.get("candidate_assertion_failures"),
            capability_gap=row.get("capability_gap"),
            cross_target_captures=cross_caps,
        )
        if row["status"] != status or row["differences"] != diffs:
            raise ValueError(f"recomputed status/differences disagree for {row['id']}: {row['status']} vs {status}")
        # Explicitly verify capability-boundary cases are NEVER labeled `pass`!
        if case.get("capability_boundary") and row["status"] == "pass":
            raise ValueError(f"capability-boundary case {row['id']} must never be counted as pass")

    if recomputed_telemetry != (report.get("conversion_telemetry") or {}):
        raise ValueError("recomputed conversion_telemetry disagrees with report.json")

    cross_rows = [r for r in report["results"] if r["exec_mode"] == "cross_target_parity"]
    expected_cross_summary = None
    if cross_rows:
        is_oracle_mode = report.get("mode") == "java-only-oracle-recording"
        expected_cross_summary = {
            "total": len(cross_rows),
            "per_target_passed": (
                None
                if is_oracle_mode
                else {
                    ep: sum(1 for r in cross_rows if (r.get("per_target_oracle_matches") or {}).get(ep) is True)
                    for ep in CROSS_TARGET_ENDPOINTS
                }
            ),
            "shared_engine_passed": (
                None if is_oracle_mode else sum(1 for r in cross_rows if r.get("shared_engine_match") is True)
            ),
            "all_targets_passed": (
                None if is_oracle_mode else sum(1 for r in cross_rows if r["status"] == "pass")
            ),
        }
    if report.get("cross_target_summary") != expected_cross_summary:
        raise ValueError("recomputed cross_target_summary disagrees with report.json")

    recomputed_status = dict(sorted(Counter(r["status"] for r in report["results"]).items()))
    if recomputed_status != report["status_counts"]:
        raise ValueError("recomputed status_counts disagree with report.json")
    recomputed_lanes = {
        lane: dict(sorted(Counter(r["status"] for r in report["results"] if r["lane"] == lane).items()))
        for lane in report["lanes"]
    }
    if recomputed_lanes != report["lane_status_counts"]:
        raise ValueError("recomputed lane_status_counts disagree with report.json")
    if report.get("compatibility_pass_count") != recomputed_status.get("pass", 0):
        raise ValueError("compatibility_pass_count disagrees with strict pass count")
    if report.get("capability_blocked_count") != recomputed_status.get("capability-blocked", 0):
        raise ValueError("capability_blocked_count disagrees with capability-blocked count")
    if report.get("executable_case_count") != sum(1 for r in report["results"] if not r["capability_boundary"]):
        raise ValueError("executable_case_count disagrees with results")
    if report.get("capability_boundary_case_count") != sum(1 for r in report["results"] if r["capability_boundary"]):
        raise ValueError("capability_boundary_case_count disagrees with results")

    src_snap = directory / "sources.snapshot.json"
    if not src_snap.is_file() or oracle.sha256(src_snap) != report.get("sources_snapshot_sha256"):
        raise ValueError("sources.snapshot.json missing or sha256 disagrees with report.json")
    snap_doc = json.loads(src_snap.read_bytes())
    if (snap_doc.get("before") == snap_doc.get("after")) != report.get("source_stability"):
        raise ValueError("recomputed source_stability disagrees with report.json")

    if strict:
        if report.get("mode") != "differential":
            raise ValueError("strict gate requires differential mode (oracle-only recordings cannot pass)")
        if not report["oracle"].get("exact_jdk_lock_match", False):
            raise ValueError("strict gate requires exact_jdk_lock_match == true")
        if cands is None:
            raise ValueError("strict gate requires recorded-build candidates (oracle-only recordings cannot pass)")
        for role in report.get("required_candidate_roles", []):
            if role not in cands or cands[role].get("source_binding") != "recorded-build":
                raise ValueError(f"strict gate requires recorded-build candidate for role: {role}")
        if report.get("source_stability") is not True:
            raise ValueError("strict gate requires source_stability == true")
        if report.get("server_self_test") is not None and not report["server_self_test"].get("passed"):
            raise ValueError("strict gate requires server_self_test.passed == true")
        if any(lane in report["lanes"] for lane in ("wasm-browser", "cross-target-parity", "packaging-and-install")):
            bp = report.get("browser_provenance")
            pinned_pw = json.loads((ROOT / PLAYWRIGHT_PACKAGE_JSON).read_bytes())["devDependencies"]["playwright"]
            if (
                not isinstance(bp, dict)
                or bp.get("browserErrors") != []
                or bp.get("playwrightPin") != pinned_pw
                or bp.get("playwright") != pinned_pw
                or bp.get("playwrightCore") != pinned_pw
                or not isinstance(bp.get("browser"), str)
                or not bp["browser"]
                or not isinstance(bp.get("browserExecutableSha256"), str)
                or not re.fullmatch(r"[0-9a-f]{64}", bp["browserExecutableSha256"])
            ):
                raise ValueError(
                    f"strict gate requires verified browser_provenance with pinned Playwright {pinned_pw}, "
                    "non-empty browser version/sha256, and empty browserErrors"
                )
        bad_exec = [r for r in report["results"] if not r["capability_boundary"] and r["status"] != "pass"]
        if bad_exec:
            summary = ", ".join(f"{r['id']}={r['status']}" for r in bad_exec[:10])
            raise ValueError(f"strict gate failed: {len(bad_exec)} non-pass executable case(s): {summary}")
        bad_cap = [
            r for r in report["results"]
            if r["capability_boundary"] and r["status"] != "capability-blocked"
        ]
        if bad_cap:
            summary = ", ".join(f"{r['id']}={r['status']}" for r in bad_cap[:10])
            raise ValueError(f"strict gate failed: {len(bad_cap)} unverified capability-boundary case(s): {summary}")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--output", type=Path, help="fresh evidence directory for live run")
    action.add_argument("--verify-report", type=Path, help="verify integrity only (preserves recorded failures)")
    action.add_argument("--verify-strict", type=Path, help="verify integrity AND enforce strict M6 completion gate")
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--lane", action="append", choices=ALL_LANES, help="select specific lane(s); default runs all 5")
    parser.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT)
    parser.add_argument("--oracle-only", "--java-only", dest="oracle_only", action="store_true",
                        help="record pinned Java 0.32.0 oracle baseline only (never passes strict gate)")
    parser.add_argument("--record-only", action="store_true", help="exit 0 after integrity verification even with mismatches")
    parser.add_argument("--cargo-target-dir", type=Path, help="Cargo target directory for recorded builds")
    args = parser.parse_args()

    try:
        if args.output:
            lanes = list(dict.fromkeys(args.lane)) if args.lane else list(ALL_LANES)
            report = execute(
                args.root.resolve(),
                args.output.resolve(),
                lanes,
                args.timeout,
                args.oracle_only,
                args.cargo_target_dir,
            )
            print(json.dumps({
                "mode": report["mode"],
                "selected_count": report["selected_count"],
                "executable_case_count": report["executable_case_count"],
                "capability_boundary_case_count": report["capability_boundary_case_count"],
                "compatibility_pass_count": report["compatibility_pass_count"],
                "capability_blocked_count": report["capability_blocked_count"],
                "status_counts": report["status_counts"],
                "lane_status_counts": report["lane_status_counts"],
            }, sort_keys=True))
            if args.record_only or args.oracle_only:
                return 0
            verify_report(args.output.resolve(), strict=True)
            return 0
        target = (args.verify_report or args.verify_strict).resolve()
        report = verify_report(target, strict=bool(args.verify_strict))
        print(json.dumps({
            "mode": report["mode"],
            "selected_count": report["selected_count"],
            "executable_case_count": report["executable_case_count"],
            "capability_boundary_case_count": report["capability_boundary_case_count"],
            "compatibility_pass_count": report["compatibility_pass_count"],
            "capability_blocked_count": report["capability_blocked_count"],
            "status_counts": report["status_counts"],
            "lane_status_counts": report["lane_status_counts"],
        }, sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"M6 differential error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
