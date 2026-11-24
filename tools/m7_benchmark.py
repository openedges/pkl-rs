#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Milestone M7 Performance & Peak RSS Benchmark Harness (Rust vs pinned Pkl 0.32.0 Native Image & JVM).

Measures execution time AND real per-process peak RSS (`os.wait4` `ru_maxrss`,
macOS bytes / Linux KiB) across all required M7 workload families:
  - Core 6 (`scalar`, `stdlib-collections`, `recursive-compute`, `large-output`,
    `failure-diagnostics`, `import-graph`) in `cold-cli`, `persistent`, and
    `fresh-evaluator` modes
  - `reflection-stdlib` (`import "pkl:reflect"` in `cold-cli` and `persistent`)
  - `renderers-files-binary` (`output.files` 7 text renderers + `pkl-binary`)
  - `packages-http-cache` (warm `package-2` cache + cold HTTPS download via `M4FixtureServers`)
  - `repl` (startup + deterministic piped session)
  - `server` (MessagePack `0x20-0x25` startup + steady request throughput)
  - `formatter` (`pkl format` on substantial unformatted module)
  - `pkldoc` (`SinglePackageTest` documentation site generation)
  - `napi` (`@pkl-rs/napi` Node startup, warm eval & `BigInt`/`Uint8Array` conversion + Node RSS)
  - `wasm-node` (`@pkl-rs/wasm` Node init/compile, warm eval & conversion + Node RSS)
  - `wasm-browser` (`@pkl-rs/wasm` Playwright Chromium 1.64.1 init, warm eval & browser-tree RSS)

Strict honesty invariants (`INTENT.md`, `TESTING.md`, `AGENTS.md`):
  1. 100% raw-byte output/error/file verification against pinned Pkl 0.32.0 BEFORE timing.
  2. Never mixes JVM heap with OS RSS; measures real `os.wait4` `ru_maxrss` (plus full
     Chromium process-tree RSS via `/bin/ps` for `wasm-browser`).
  3. Reports full empirical distributions (`count`, `min`, `median`, `p90`, `p95`, `max`,
     `mean`, `sample_stdev`) and explicit per-workload ratios & verdicts (`pass`, `fail`,
     `inconclusive`, `unverified`). Never conceals individual regressions behind averages.
  4. **Primary CLI/Server/`pkldoc` baseline (2026-11-23 user decision)**: All 15 CLI, server,
     and `pkldoc` workloads (`cold-cli` and `steady-server`) are strictly gated at `<= 1.00x`
     execution time AND `<= 1.00x` `os.wait4` peak RSS against **pinned Official Apple Pkl
     0.32.0 `native-image`** (`pkl` and `pkldoc`, `exact_native_image_lock_match: true`),
     while retaining live pinned Eclipse Adoptium Temurin `21.0.13.1+2-LTS`
     (`exact_jdk_lock_match: true`, rejecting `PKL_ALLOW_LOCAL_JDK=1`) and archived M0
     GraalVM CE `25.0.1` (`reports/m0/benchmarks.json`) separately for complete provenance.
     All 19 embedded API (`persistent` / `fresh-evaluator`) and JS/Wasm (`napi`, `wasm-node`,
     `wasm-browser`) workloads remain strictly gated against pinned JVM (plus the `2026-11-22`
     browser Wasm contract), because official `native-image` ships CLI executables only.
"""
from __future__ import annotations

import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import queue
import re
import shutil
import signal
import statistics
import subprocess
import sys
import threading
import time
from typing import Any

import benchmark as m0_benchmark
import m4_differential
import m5_differential
import m5_pkldoc_tree
import m5_server_client
import oracle

ROOT = Path(__file__).resolve().parents[1]
WORKLOADS_PATH = Path("benchmarks/m7_workloads.json")
M0_GRAALVM_REF_PATH = Path("reports/m0/benchmarks.json")
BENCHMARK_JAVA = Path("tools/java/Benchmark.java")
JAVA_TYPED_BENCH = Path("benchmarks/m7_fixtures/JavaTypedBench.java")
M4_REGISTRY_DIR = Path("tests/fixtures/m4/projects/registry")
M4_HOST_IO_DIR = Path("tests/fixtures/m4/host_io")

DEFAULT_CACHED_TEMURIN = Path(
    ".cache/oracle/temurin-21.0.13.1+2-LTS/jdk-21.0.13.1+2/Contents/Home/bin/java"
)

ALL_FAMILIES = (
    "core-6",
    "reflection-stdlib",
    "renderers-files-binary",
    "packages-http-cache",
    "repl",
    "server",
    "formatter",
    "pkldoc",
    "napi",
    "wasm-node",
    "wasm-browser",
)

NATIVE_IMAGE_ELIGIBLE_MODES = ("cold-cli", "steady-server")
NATIVE_IMAGE_ELIGIBLE_FAMILIES = frozenset(
    {
        "core-6",
        "reflection-stdlib",
        "renderers-files-binary",
        "packages-http-cache",
        "repl",
        "server",
        "formatter",
        "pkldoc",
    }
)
VALID_BASELINES = ("both", "native-image", "jvm")
NATIVE_IMAGE_API_NA_REASON = (
    "Official Apple Pkl 0.32.0 native-image only ships CLI executables (pkl, pkldoc) "
    "and has no embeddable Java/JS/Wasm API."
)
REPL_JLINE_STDERR_NOTE = (
    'Official Apple Pkl 0.32.0 native-image pkl repl reads System.getProperty("user.home")/.pkl/repl-history '
    "via JLine and may emit benign JLine history log lines to stderr; Rust pkl repl remains strictly "
    'verified for stderr == b"" and 100% raw-byte stdout parity.'
)
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")

SOURCE_SNAPSHOT_GLOBS = (
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "oracle.lock.json",
    "tools/oracle.py",
    "crates/*/src/**/*.rs",
    "crates/pkl/examples/m7_bench_probe.rs",
    "packages/pkl-wasm/src/**/*",
    "packages/pkl-wasm/dist/**/*",
    "packages/pkl-napi/src/**/*",
    "packages/pkl-napi/dist/**/*",
    "packages/pkl-napi/native/*.node",
    "benchmarks/workloads.json",
    "benchmarks/m7_workloads.json",
    "benchmarks/fixtures/**/*",
    "benchmarks/m7_fixtures/**/*",
    "tools/m7_benchmark.py",
    "tools/java/Benchmark.java",
)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def file_digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def distribution(values: list[float | int]) -> dict[str, Any]:
    if not values or any(not isinstance(v, (int, float)) or not math.isfinite(v) or v <= 0 for v in values):
        raise ValueError(f"missing or non-positive raw samples: {values!r}")
    ordered = sorted(float(v) for v in values)
    pct = lambda p: ordered[max(0, math.ceil(len(ordered) * p) - 1)]
    return {
        "count": len(ordered),
        "min": ordered[0],
        "median": float(statistics.median(ordered)),
        "p90": pct(0.90),
        "p95": pct(0.95),
        "max": ordered[-1],
        "mean": float(statistics.mean(ordered)),
        "sample_stdev": float(statistics.stdev(ordered)) if len(ordered) > 1 else 0.0,
        "uncertainty": "Empirical sample distribution only; per-workload ratios evaluated directly without cross-workload averaging",
    }


def per_process_rss_bytes(usage: Any, system: str | None = None) -> int:
    return m0_benchmark.per_process_rss_bytes(usage, system=system)


def expected_workload_sets(contract: dict[str, Any]) -> tuple[set[str], set[str], set[str]]:
    """Return `(all_workload_ids, core_workload_ids, native_image_eligible_ids)`."""
    all_ids: set[str] = set()
    core_ids: set[str] = set()
    native_ids: set[str] = set()
    for w in contract.get("core_workloads", []):
        for mode in w.get("modes", []):
            wid = f"{w['id']}:{mode}"
            all_ids.add(wid)
            core_ids.add(wid)
            if mode in NATIVE_IMAGE_ELIGIBLE_MODES:
                native_ids.add(wid)
    for w in contract.get("product_workloads", []):
        kind = w["kind"]
        if kind == "eval_cold_and_persistent":
            modes: tuple[str, ...] = ("cold-cli", "persistent")
        elif kind == "server_msgpack":
            modes = ("steady-server",)
        elif kind == "js_binding":
            modes = (w["target"],)
        else:
            modes = ("cold-cli",)
        for mode in modes:
            wid = f"{w['id']}:{mode}"
            all_ids.add(wid)
            if mode in NATIVE_IMAGE_ELIGIBLE_MODES:
                native_ids.add(wid)
    return all_ids, core_ids, native_ids


def validate_native_image_lock(root: Path = ROOT) -> dict[str, Any]:
    lock_path = root / "oracle.lock.json"
    if not lock_path.is_file():
        raise ValueError(f"missing oracle.lock.json at {lock_path}")
    lock = json.loads(lock_path.read_bytes())
    if lock.get("schema") != 1 or lock.get("version") != "0.32.0":
        raise ValueError("oracle.lock.json schema/version mismatch")
    native_cfg = lock.get("native_image")
    if not isinstance(native_cfg, dict):
        raise ValueError("oracle.lock.json missing required native_image section")
    dists = native_cfg.get("distributions")
    if not isinstance(dists, dict) or not dists:
        raise ValueError("oracle.lock.json native_image.distributions must be non-empty")
    for target, dist in dists.items():
        if not isinstance(dist, dict):
            raise ValueError(f"invalid native_image distribution for {target}")
        subdir = dist.get("cache_subdir")
        if not isinstance(subdir, str) or not subdir.strip() or Path(subdir).is_absolute() or ".." in Path(subdir).parts:
            raise ValueError(f"invalid native_image cache_subdir for {target}: {subdir!r}")
        for label in ("pkl", "pkldoc"):
            spec = dist.get(label)
            if not isinstance(spec, dict):
                raise ValueError(f"oracle.lock.json native_image.{target} missing {label} pin")
            if not isinstance(spec.get("filename"), str) or not spec["filename"].strip():
                raise ValueError(f"invalid native_image {target}.{label}.filename")
            if not isinstance(spec.get("url"), str) or not spec["url"].startswith("https://"):
                raise ValueError(f"invalid native_image {target}.{label}.url")
            nbytes = spec.get("bytes")
            if isinstance(nbytes, bool) or not isinstance(nbytes, int) or nbytes <= 0:
                raise ValueError(f"invalid native_image {target}.{label}.bytes")
            sha_val = spec.get("sha256")
            if not isinstance(sha_val, str) or not SHA256_RE.match(sha_val):
                raise ValueError(f"invalid native_image {target}.{label}.sha256")
            if not isinstance(spec.get("version_prefix"), str) or not spec["version_prefix"]:
                raise ValueError(f"invalid native_image {target}.{label}.version_prefix")
            if not isinstance(spec.get("version_suffix"), str) or not spec["version_suffix"]:
                raise ValueError(f"invalid native_image {target}.{label}.version_suffix")
    return native_cfg


def is_benign_jline_repl_stderr(stderr_bytes: bytes) -> bool:
    """Return True iff `stderr_bytes` is empty or contains solely benign JLine history log lines.

    Official Apple Pkl 0.32.0 native-image `pkl repl` resolves `System.getProperty("user.home")`
    from the OS user record rather than `$HOME`, so JLine may log benign history messages
    (`INFO: History file ...` or `WARNING: Failed to load/save history` with `repl-history`
    `DefaultHistory` stack traces). Any Pkl diagnostic or non-JLine error is rejected.
    """
    if not stderr_bytes:
        return True
    try:
        text = stderr_bytes.decode("utf-8")
    except UnicodeDecodeError:
        return False
    for forbidden in ("–– Pkl Error ––", "PklException", "PklBugException"):
        if forbidden in text:
            return False
    lines = text.splitlines()
    idx = 0
    saw_jline = False
    while idx < len(lines):
        line = lines[idx]
        if not line.strip():
            idx += 1
            continue
        if not re.search(r"\borg\.jline\.utils\.Log\s+log\s*$", line):
            return False
        saw_jline = True
        idx += 1
        if idx >= len(lines):
            return False
        msg = lines[idx]
        if re.match(r"^INFO:\s+History file\b", msg):
            idx += 1
            continue
        if re.match(r"^WARNING:\s+Failed to (load|save) history\s*$", msg):
            idx += 1
            if idx < len(lines) and re.match(r"^java\.(nio\.file|io)\.[A-Za-z0-9_$.]+Exception:.*repl-history", lines[idx]):
                idx += 1
                saw_history_frame = False
                while idx < len(lines) and lines[idx].startswith("\tat "):
                    if "org.jline.reader.impl.history." in lines[idx]:
                        saw_history_frame = True
                    idx += 1
                if not saw_history_frame:
                    return False
            continue
        return False
    return saw_jline


def load_contract(root: Path = ROOT) -> dict[str, Any]:
    path = root / WORKLOADS_PATH
    contract = json.loads(path.read_bytes())
    if contract.get("schema_version") != 1 or contract.get("baseline") != "0.32.0":
        raise ValueError("unknown M7 benchmark contract schema_version or baseline")
    targets = contract.get("target_ratios", {})
    t_max = targets.get("execution_time_max")
    r_max = targets.get("peak_rss_max")
    if isinstance(t_max, bool) or isinstance(r_max, bool) or t_max != 1.0 or r_max != 1.0:
        raise ValueError("M7 target_ratios must enforce execution_time_max == 1.0 and peak_rss_max == 1.0")
    ntargets = contract.get("native_image_targets")
    if not isinstance(ntargets, dict):
        raise ValueError("M7 benchmark contract missing native_image_targets")
    nt_max = ntargets.get("execution_time_max")
    nr_max = ntargets.get("peak_rss_max")
    if isinstance(nt_max, bool) or isinstance(nr_max, bool) or nt_max != 1.0 or nr_max != 1.0:
        raise ValueError("native_image_targets must enforce execution_time_max == 1.0 and peak_rss_max == 1.0")
    if ntargets.get("required_for_cli_server_pkldoc") is not True:
        raise ValueError("native_image_targets.required_for_cli_server_pkldoc must be true")
    if ntargets.get("eligible_modes") != list(NATIVE_IMAGE_ELIGIBLE_MODES):
        raise ValueError(f"native_image_targets.eligible_modes must be {list(NATIVE_IMAGE_ELIGIBLE_MODES)}")
    if not str(ntargets.get("repl_jline_stderr_note", "")).strip():
        raise ValueError("native_image_targets.repl_jline_stderr_note must be documented")
    validate_native_image_lock(root)
    btargets = contract.get("browser_wasm_targets")
    if not isinstance(btargets, dict):
        raise ValueError("M7 benchmark contract missing browser_wasm_targets")
    if isinstance(btargets.get("execution_time_max"), bool) or btargets.get("execution_time_max") != 1.0:
        raise ValueError("browser_wasm_targets.execution_time_max must remain 1.0")
    lin_max = btargets.get("wasm_linear_memory_ratio_max")
    if isinstance(lin_max, bool) or not isinstance(lin_max, (int, float)) or not (0.0 < float(lin_max) <= 0.15):
        raise ValueError("browser_wasm_targets.wasm_linear_memory_ratio_max must be in (0, 0.15]")
    js_heap_max = btargets.get("js_heap_used_max_bytes")
    if isinstance(js_heap_max, bool) or not isinstance(js_heap_max, int) or not (0 < js_heap_max <= 64 * 1024 * 1024):
        raise ValueError("browser_wasm_targets.js_heap_used_max_bytes must be in (0, 64 MiB]")
    gzip_max = btargets.get("wasm_binary_gzip_max_bytes")
    if isinstance(gzip_max, bool) or not isinstance(gzip_max, int) or not (0 < gzip_max <= 2_621_440):
        raise ValueError("browser_wasm_targets.wasm_binary_gzip_max_bytes must be in (0, 2.50 MiB]")
    exceptions = btargets.get("engine_time_exceptions", {})
    if not isinstance(exceptions, dict) or not set(exceptions.keys()).issubset({"firefox"}):
        raise ValueError("browser_wasm_targets.engine_time_exceptions may only contain explicit 'firefox' entries")
    for eng, wmap in exceptions.items():
        if not isinstance(wmap, dict):
            raise ValueError(f"invalid engine_time_exceptions entry for {eng}")
        for wkey, rule in wmap.items():
            if not isinstance(rule, dict) or not isinstance(rule.get("execution_time_max"), (int, float)):
                raise ValueError(f"invalid engine_time_exceptions rule for {eng}:{wkey}")
            if float(rule["execution_time_max"]) <= 1.0 or not str(rule.get("reason", "")).strip():
                raise ValueError(f"engine_time_exceptions rule {eng}:{wkey} must document reason and > 1.0 bound")
    for profile_name in ("full", "quick"):
        if profile_name not in contract.get("profiles", {}):
            raise ValueError(f"missing profile {profile_name} in M7 benchmark contract")
        if profile_name not in contract.get("js_sampling", {}):
            raise ValueError(f"missing js_sampling {profile_name} in M7 benchmark contract")
    full_prof = contract["profiles"]["full"]
    if (
        not full_prof.get("strict_eligible")
        or full_prof.get("cold_samples", 0) < 5
        or full_prof.get("samples", 0) < 5
        or full_prof.get("minimum_warmup_seconds", 0) < 10.0
        or full_prof.get("independent_api_forks", 0) < 2
    ):
        raise ValueError("full profile does not meet strict sampling contract")
    if contract["profiles"]["quick"].get("strict_eligible"):
        raise ValueError("quick profile must never be strict_eligible")

    m0_ref = (root / contract["m0_graalvm_reference"]).resolve()
    if not m0_ref.is_relative_to(root.resolve()) or not m0_ref.is_file():
        raise ValueError(f"missing M0 GraalVM reference: {contract['m0_graalvm_reference']}")

    seen_ids: set[str] = set()
    for w in contract.get("core_workloads", []):
        wid = w["id"]
        if wid in seen_ids:
            raise ValueError(f"duplicate workload id: {wid}")
        seen_ids.add(wid)
        mod = (root / w["module"]).resolve()
        if not mod.is_relative_to(root.resolve()) or not mod.is_file():
            raise ValueError(f"missing core workload module: {w['module']}")
        for res in w.get("resources", []):
            rp = (root / res).resolve()
            if not rp.is_relative_to(root.resolve()) or not rp.is_file():
                raise ValueError(f"missing core workload resource: {res}")

    for w in contract.get("product_workloads", []):
        wid = w["id"]
        if wid in seen_ids:
            raise ValueError(f"duplicate workload id: {wid}")
        seen_ids.add(wid)
        if w.get("family") not in ALL_FAMILIES:
            raise ValueError(f"unknown workload family for {wid}: {w.get('family')}")
        for key in ("module", "template", "session_file", "input_file"):
            if key in w:
                p = (root / w[key]).resolve()
                if not p.is_relative_to(root.resolve()) or not p.is_file():
                    raise ValueError(f"missing fixture file for {wid}: {w[key]}")
        if "input_dir" in w:
            p = (root / w["input_dir"]).resolve()
            if not p.is_relative_to(root.resolve()) or not p.is_dir():
                raise ValueError(f"missing fixture dir for {wid}: {w['input_dir']}")

    _, _, native_ids = expected_workload_sets(contract)
    if ntargets.get("expected_eligible_workload_count") != len(native_ids):
        raise ValueError(
            f"native_image_targets.expected_eligible_workload_count={ntargets.get('expected_eligible_workload_count')} "
            f"!= actual eligible CLI/server/pkldoc count {len(native_ids)}"
        )

    for drv in ("bench_napi.mjs", "bench_wasm_node.mjs", "bench_wasm_browser.mjs"):
        p = root / "benchmarks/m7_fixtures" / drv
        if not p.is_file():
            raise ValueError(f"missing JS benchmark driver: {p}")

    return contract


def source_snapshot(root: Path) -> tuple[dict[str, str], str]:
    files: dict[str, str] = {}
    for pattern in SOURCE_SNAPSHOT_GLOBS:
        for p in sorted(root.glob(pattern)):
            if p.is_file() and not p.is_symlink():
                rel = p.relative_to(root).as_posix()
                files[rel] = file_digest(p)
    encoded = json.dumps(files, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return files, digest(encoded)


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
                raise ValueError(f"evidence directory must contain only regular files: {p}")
            if p != directory / "report.json":
                yield p


def seal_evidence_files(directory: Path) -> list[dict[str, Any]]:
    rows = []
    for p in evidence_paths(directory):
        data = p.read_bytes()
        rows.append(
            {
                "path": p.relative_to(directory).as_posix(),
                "sha256": digest(data),
                "bytes": len(data),
            }
        )
    return rows


def ensure_pinned_oracle(root: Path) -> dict[str, Any]:
    if os.environ.get("PKL_ALLOW_LOCAL_JDK") == "1":
        raise oracle.OracleError(
            "M7 benchmarks forbid PKL_ALLOW_LOCAL_JDK=1; exact pinned Temurin 21.0.13.1+2-LTS is required"
        )
    prev_java = os.environ.get("PKL_ORACLE_JAVA")
    injected = False
    try:
        if not prev_java:
            cached = root / DEFAULT_CACHED_TEMURIN
            if cached.is_file():
                os.environ["PKL_ORACLE_JAVA"] = str(cached)
                injected = True
        meta = oracle.ensure_oracle(root, fetch=False, profile="reference")
    finally:
        if injected:
            os.environ.pop("PKL_ORACLE_JAVA", None)
    if not meta.get("exact_jdk_lock_match", False):
        raise oracle.OracleError("M7 benchmarks require exact_jdk_lock_match == true")
    return meta


def ensure_pinned_native_image_oracle(root: Path) -> dict[str, Any]:
    meta = oracle.ensure_native_image_oracle(root, fetch=False)
    if not meta.get("exact_native_image_lock_match", False):
        raise oracle.OracleError("M7 benchmarks require exact_native_image_lock_match == true")
    return meta


def build_isolated_env(
    base_env: dict[str, str],
    state_dir: Path,
    extra: dict[str, str] | None = None,
) -> dict[str, str]:
    home = state_dir / "home"
    tmp = state_dir / "tmp"
    home.mkdir(parents=True, exist_ok=True)
    tmp.mkdir(parents=True, exist_ok=True)
    env = dict(base_env)
    env.update(
        {
            "HOME": str(home),
            "TMPDIR": str(tmp),
            "TMP": str(tmp),
            "TEMP": str(tmp),
            "LC_ALL": "C",
            "LANG": "C",
            "TZ": "UTC",
            "TERM": "dumb",
            "NO_COLOR": "1",
            "COLUMNS": "80",
            "PYTHONDONTWRITEBYTECODE": "1",
        }
    )
    if extra:
        env.update(extra)
    return env


def java_prefix_for_state(prefix: list[str], state_dir: Path, *, main_class: str | None = None) -> list[str]:
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
    if main_class is not None:
        out[-1] = main_class
    return out


def capture_wait4(
    command: list[str],
    cwd: Path,
    environment: dict[str, str],
    destination: Path,
    *,
    timeout: float = 180.0,
    stdin_file: Path | None = None,
) -> dict[str, Any]:
    """Execute command and reap exact PID via `os.wait4` for per-process peak RSS."""
    if not hasattr(os, "wait4"):
        raise ValueError("per-process os.wait4 peak RSS unavailable on this platform")
    destination.mkdir(parents=True, exist_ok=False)
    stdout_path = destination / "stdout.bin"
    stderr_path = destination / "stderr.bin"
    timed_out = threading.Event()
    stdin_fh = stdin_file.open("rb") if stdin_file is not None else None
    try:
        with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
            started = time.perf_counter_ns()
            proc = subprocess.Popen(
                command,
                cwd=cwd,
                env=environment,
                stdin=stdin_fh if stdin_fh is not None else subprocess.DEVNULL,
                stdout=stdout,
                stderr=stderr,
                start_new_session=True,
            )

            def expire() -> None:
                try:
                    os.killpg(proc.pid, signal.SIGKILL)
                    timed_out.set()
                except (ProcessLookupError, PermissionError):
                    pass

            timer = threading.Timer(timeout, expire)
            timer.daemon = True
            timer.start()
            try:
                _, status, usage = os.wait4(proc.pid, 0)
                elapsed = time.perf_counter_ns() - started
                proc.returncode = os.waitstatus_to_exitcode(status)
            finally:
                timer.cancel()
                try:
                    os.killpg(proc.pid, signal.SIGKILL)
                except (ProcessLookupError, PermissionError):
                    pass
    finally:
        if stdin_fh is not None:
            stdin_fh.close()

    stdout_bytes = stdout_path.read_bytes()
    stderr_bytes = stderr_path.read_bytes()
    result = {
        "command": [str(a) for a in command],
        "cwd": str(cwd),
        "status": "timeout" if timed_out.is_set() else "executed",
        "exit_code": proc.returncode,
        "elapsed_ns": elapsed,
        "peak_rss_bytes": per_process_rss_bytes(usage),
        "user_cpu_seconds": usage.ru_utime,
        "system_cpu_seconds": usage.ru_stime,
        "stdout": {
            "path": str(stdout_path),
            "sha256": digest(stdout_bytes),
            "bytes": len(stdout_bytes),
        },
        "stderr": {
            "path": str(stderr_path),
            "sha256": digest(stderr_bytes),
            "bytes": len(stderr_bytes),
        },
    }
    write_json(destination / "result.json", result)
    return result


def raw_channel(capture: dict[str, Any], channel: str) -> bytes:
    path = Path(capture[channel]["path"])
    data = path.read_bytes()
    if digest(data) != capture[channel]["sha256"] or len(data) != capture[channel]["bytes"]:
        raise ValueError(f"corrupt captured {channel}: {path}")
    return data


def verify_capture_raw(
    capture: dict[str, Any],
    *,
    expected_exit: int = 0,
    expected_stdout: bytes | None = None,
    expected_stderr: bytes | None = None,
    helper: bool = False,
) -> tuple[bytes, bytes]:
    if capture.get("status") != "executed" or capture.get("peak_rss_bytes", 0) <= 0:
        raise ValueError(f"unexecuted/timeout or missing RSS sample: {capture.get('status')}")
    if not isinstance(capture.get("elapsed_ns"), (int, float)) or capture["elapsed_ns"] <= 0:
        raise ValueError("missing or non-positive elapsed_ns")
    if capture["exit_code"] != expected_exit:
        err_preview = raw_channel(capture, "stderr")[:500].decode("utf-8", errors="replace")
        raise ValueError(
            f"unexpected exit code {capture['exit_code']} (expected {expected_exit}): {err_preview}"
        )
    stdout = raw_channel(capture, "stdout")
    stderr = raw_channel(capture, "stderr")
    if helper and stderr:
        raise ValueError(f"unexpected helper stderr: {stderr[:400].decode('utf-8', errors='replace')}")
    if expected_stdout is not None and stdout != expected_stdout:
        raise ValueError(
            f"raw stdout bytes differ from verified Java preflight ({len(stdout)}B vs {len(expected_stdout)}B)"
        )
    if expected_stderr is not None and stderr != expected_stderr:
        raise ValueError(
            f"raw stderr bytes differ from verified Java preflight ({len(stderr)}B vs {len(expected_stderr)}B)"
        )
    return stdout, stderr


def parse_probe_series(
    capture: dict[str, Any],
    sampling: dict[str, Any],
    mode: str,
    memory_boundary: str,
) -> dict[str, Any]:
    if mode not in {"persistent", "fresh-evaluator"}:
        raise ValueError(f"unknown probe mode: {mode}")
    stdout, _ = verify_capture_raw(capture, expected_exit=0, helper=True)
    rows = [json.loads(line) for line in stdout.splitlines() if line.strip()]
    phase = lambda name: [r for r in rows if r.get("phase") == name]
    if len(phase("initialize")) != 1 or len(phase("first-request")) != 1 or len(phase("warmup-result")) != 1:
        raise ValueError("missing or duplicate probe timing phase")
    warmup = phase("warmup")
    measurements = phase("measurement")
    summary = phase("warmup-result")[0]
    phases = [r.get("phase") for r in rows]
    expected_phases = (
        ["initialize", "first-request"]
        + ["warmup"] * len(warmup)
        + ["warmup-result"]
        + ["measurement"] * len(measurements)
    )
    if phases != expected_phases:
        raise ValueError("out-of-order probe timing phases")
    if (
        len(measurements) != sampling["samples"]
        or len(warmup) < sampling["convergence_windows"]
        or summary["windows"] != len(warmup)
        or summary["elapsed_ns"] < sampling["minimum_warmup_seconds"] * 1e9 * 0.999
    ):
        raise ValueError("insufficient warmup or measurement windows")
    for grp in (warmup, measurements):
        if [r.get("index") for r in grp] != list(range(len(grp))):
            raise ValueError("non-sequential timing window index")
        for r in grp:
            if (
                r.get("iterations", 0) <= 0
                or r.get("elapsed_ns", 0) < sampling["sample_seconds"] * 1e9 * 0.999
                or not math.isclose(r["ns_per_op"], r["elapsed_ns"] / r["iterations"], rel_tol=1e-6)
            ):
                raise ValueError(f"invalid timing window row: {r}")
    min_proc = (
        phase("initialize")[0]["elapsed_ns"]
        + phase("first-request")[0]["elapsed_ns"]
        + summary["elapsed_ns"]
        + sum(r["elapsed_ns"] for r in measurements)
    )
    # Account for small rounding differences when comparing ns sums
    if capture["elapsed_ns"] < min_proc * 0.99:
        raise ValueError("probe phase sum exceeds total process elapsed_ns")
    recent = sorted(r["ns_per_op"] for r in warmup[-sampling["convergence_windows"]:])
    spread = (recent[-1] - recent[0]) / statistics.median(recent)
    if summary["converged"] and spread > sampling["maximum_relative_spread"] + 1e-9:
        raise ValueError(f"false warmup convergence claimed (spread={spread})")
    return {
        "rows": rows,
        "warmup": "converged" if summary["converged"] else "inconclusive",
        "recent_relative_spread": spread,
        "operation_ns": distribution([r["ns_per_op"] for r in measurements]),
        "initialize_ns": phase("initialize")[0]["elapsed_ns"],
        "first_request_ns": phase("first-request")[0]["elapsed_ns"],
        "warmup_elapsed_ns": summary["elapsed_ns"],
        "memory_boundary": memory_boundary,
        "peak_rss_bytes": capture["peak_rss_bytes"],
    }


def evaluate_ratio_verdict(
    rust_time_ns: float,
    java_time_ns: float,
    rust_rss_bytes: float,
    java_rss_bytes: float,
    *,
    correctness_verified: bool,
    converged: bool = True,
    target_time_max: float = 1.0,
    target_rss_max: float = 1.0,
    baseline_key_prefix: str = "java",
) -> dict[str, Any]:
    if java_time_ns <= 0 or java_rss_bytes <= 0 or rust_time_ns <= 0 or rust_rss_bytes <= 0:
        raise ValueError("ratio inputs must be strictly positive")
    time_ratio = rust_time_ns / java_time_ns
    rss_ratio = rust_rss_bytes / java_rss_bytes
    time_ok = time_ratio <= target_time_max
    rss_ok = rss_ratio <= target_rss_max
    if not correctness_verified:
        verdict = "fail"
        reason = "raw-byte correctness mismatch"
    elif not time_ok or not rss_ok:
        verdict = "fail"
        parts = []
        if not time_ok:
            parts.append(f"time_ratio={time_ratio:.4f}>{target_time_max:.2f}")
        if not rss_ok:
            parts.append(f"rss_ratio={rss_ratio:.4f}>{target_rss_max:.2f}")
        reason = "; ".join(parts)
    elif not converged:
        verdict = "inconclusive"
        reason = "warmup did not converge within maximum_warmup_seconds"
    else:
        verdict = "pass"
        reason = "correctness verified and both time & peak RSS ratios <= 1.00"
    return {
        "rust_time_ns": rust_time_ns,
        f"{baseline_key_prefix}_time_ns": java_time_ns,
        "time_ratio": time_ratio,
        "time_meets_target": time_ok,
        "rust_peak_rss_bytes": rust_rss_bytes,
        f"{baseline_key_prefix}_peak_rss_bytes": java_rss_bytes,
        "rss_ratio": rss_ratio,
        "rss_meets_target": rss_ok,
        "correctness_verified": correctness_verified,
        "converged": converged,
        "verdict": verdict,
        "reason": reason,
    }


def evaluate_browser_wasm_verdict(
    rust_time_ns: float,
    java_time_ns: float,
    browser_tree_rss_bytes: float,
    java_rss_bytes: float,
    *,
    wasm_linear_memory_bytes: float,
    js_heap_used_bytes: float | None,
    wasm_binary_gzip_bytes: int,
    correctness_verified: bool,
    converged: bool,
    browser_targets: dict[str, Any],
    browser_engine: str = "chromium",
    workload_key: str | None = None,
) -> dict[str, Any]:
    """Evaluate browser `@pkl-rs/wasm` under the 2026-11-22 browser Wasm contract.

    Execution time must meet `<= 1.00x` Java for Chromium and WebKit (with only the
    explicitly documented Firefox `typed-conversion` exception). Memory is gated on
    Wasm linear memory (`<= 0.15x` Java peak RSS), JS heap used (`<= 64 MiB` where
    exposed), and compressed `.wasm` binary size (`<= 2.50 MiB` gzip-9), while full
    multi-process browser-tree RSS (`rust_peak_rss_bytes`, `rss_ratio`) is preserved
    verbatim for transparency.
    """
    if (
        java_time_ns <= 0
        or java_rss_bytes <= 0
        or rust_time_ns <= 0
        or browser_tree_rss_bytes <= 0
        or wasm_linear_memory_bytes <= 0
        or wasm_binary_gzip_bytes <= 0
    ):
        raise ValueError("browser Wasm verdict inputs must be strictly positive")

    target_time_max = float(browser_targets["execution_time_max"])
    exception_applied = False
    exceptions = browser_targets.get("engine_time_exceptions") or {}
    if browser_engine in exceptions and workload_key and workload_key in exceptions[browser_engine]:
        target_time_max = float(exceptions[browser_engine][workload_key]["execution_time_max"])
        exception_applied = True

    lin_max = float(browser_targets["wasm_linear_memory_ratio_max"])
    js_heap_max = int(browser_targets["js_heap_used_max_bytes"])
    gzip_max = int(browser_targets["wasm_binary_gzip_max_bytes"])

    time_ratio = rust_time_ns / java_time_ns
    tree_rss_ratio = browser_tree_rss_bytes / java_rss_bytes
    wasm_linear_ratio = wasm_linear_memory_bytes / java_rss_bytes

    time_ok = time_ratio <= target_time_max
    wasm_linear_ok = wasm_linear_ratio <= lin_max
    js_heap_ok = js_heap_used_bytes is None or float(js_heap_used_bytes) <= float(js_heap_max)
    gzip_ok = int(wasm_binary_gzip_bytes) <= gzip_max
    mem_ok = wasm_linear_ok and js_heap_ok and gzip_ok

    if not correctness_verified:
        verdict = "fail"
        reason = "raw-byte correctness mismatch"
    elif not time_ok or not mem_ok:
        verdict = "fail"
        parts = []
        if not time_ok:
            parts.append(f"time_ratio={time_ratio:.4f}>{target_time_max:.2f}")
        if not wasm_linear_ok:
            parts.append(f"wasm_linear_memory_ratio={wasm_linear_ratio:.4f}>{lin_max:.2f}")
        if not js_heap_ok:
            parts.append(f"js_heap_used_bytes={int(js_heap_used_bytes)}>{js_heap_max}")
        if not gzip_ok:
            parts.append(f"wasm_binary_gzip_bytes={int(wasm_binary_gzip_bytes)}>{gzip_max}")
        reason = "; ".join(parts)
    elif not converged:
        verdict = "inconclusive"
        reason = "warmup did not converge within maximum_warmup_seconds"
    else:
        verdict = "pass"
        suffix = f" ({browser_engine} {workload_key} time exception <= {target_time_max:.2f}x)" if exception_applied else ""
        reason = (
            f"correctness verified; time <= {target_time_max:.2f}x{suffix}, "
            f"wasm_linear_mem <= {lin_max:.2f}x, js_heap <= {js_heap_max // (1024 * 1024)} MiB, "
            f"wasm_gzip <= {gzip_max / (1024 * 1024):.2f} MiB (raw browser-tree RSS {tree_rss_ratio:.3f}x recorded)"
        )

    return {
        "rust_time_ns": rust_time_ns,
        "java_time_ns": java_time_ns,
        "time_ratio": time_ratio,
        "target_time_max": target_time_max,
        "time_meets_target": time_ok,
        "firefox_time_exception_applied": exception_applied,
        "rust_peak_rss_bytes": browser_tree_rss_bytes,
        "java_peak_rss_bytes": java_rss_bytes,
        "rss_ratio": tree_rss_ratio,
        "rss_meets_target": mem_ok,
        "memory_metric": "browser-wasm-linear-mem+js-heap+gzip",
        "wasm_linear_memory_bytes": wasm_linear_memory_bytes,
        "wasm_linear_memory_ratio": wasm_linear_ratio,
        "wasm_linear_memory_meets_target": wasm_linear_ok,
        "js_heap_used_bytes": js_heap_used_bytes,
        "js_heap_meets_target": js_heap_ok,
        "wasm_binary_gzip_bytes": int(wasm_binary_gzip_bytes),
        "wasm_binary_gzip_meets_target": gzip_ok,
        "correctness_verified": correctness_verified,
        "converged": converged,
        "verdict": verdict,
        "reason": reason,
    }


def java_benchmark_cmd(
    oracle_meta: dict[str, Any],
    classes_dir: Path,
    state_dir: Path,
    mode: str,
    module: Path,
    expected: Path,
    expected_error: bool,
    sampling: dict[str, Any],
    first_out: Path,
    write_expected: bool = False,
) -> list[str]:
    prefix = java_prefix_for_state(oracle_meta["command"], state_dir)
    cp = prefix.index("-cp")
    return [
        *prefix[:cp],
        "-cp",
        os.pathsep.join((str(classes_dir), oracle_meta["artifact"])),
        "Benchmark",
        mode,
        str(module),
        str(expected),
        str(expected_error).lower(),
        str(sampling["minimum_warmup_seconds"]),
        str(sampling["maximum_warmup_seconds"]),
        str(sampling["samples"]),
        str(sampling["sample_seconds"]),
        str(sampling["convergence_windows"]),
        str(sampling["maximum_relative_spread"]),
        str(first_out),
        str(write_expected).lower(),
    ]


def rust_probe_cmd(
    probe_bin: Path,
    mode: str,
    module: Path,
    expected: Path,
    expected_error: bool,
    sampling: dict[str, Any],
    first_out: Path,
    write_expected: bool = False,
) -> list[str]:
    return [
        str(probe_bin),
        mode,
        str(module),
        str(expected),
        str(expected_error).lower(),
        str(sampling["minimum_warmup_seconds"]),
        str(sampling["maximum_warmup_seconds"]),
        str(sampling["samples"]),
        str(sampling["sample_seconds"]),
        str(sampling["convergence_windows"]),
        str(sampling["maximum_relative_spread"]),
        str(first_out),
        str(write_expected).lower(),
    ]


def build_rust_artifacts(
    root: Path,
    destination: Path,
    *,
    skip_build: bool,
    selected_families: set[str] | None = None,
    baseline: str = "both",
) -> dict[str, Any]:
    pkl_bin = root / "target/release/pkl"
    pkldoc_bin = root / "target/release/pkldoc"
    probe_bin = root / "target/release/examples/m7_bench_probe"
    napi_node = root / "packages/pkl-napi/native"
    wasm_bg = root / "packages/pkl-wasm/dist/wasm/pkl_wasm_bg.wasm"

    fams = selected_families or set(ALL_FAMILIES)
    use_jvm = baseline in ("both", "jvm")
    full_catalog = fams == set(ALL_FAMILIES) and baseline == "both"
    need_pkldoc = full_catalog or "pkldoc" in fams
    need_probe = full_catalog or (use_jvm and bool(fams & {"core-6", "reflection-stdlib"}))
    need_napi = use_jvm and "napi" in fams
    need_wasm = use_jvm and bool(fams & {"wasm-node", "wasm-browser"})
    node_bin = shutil.which("node")

    build_record: dict[str, Any] = {
        "source_binding": "preexisting-binary" if skip_build else "recorded-build",
        "commands": [],
    }
    pkl_binding = "preexisting-binary"
    pkldoc_binding = "preexisting-binary"
    probe_binding = "preexisting-binary"
    napi_binding = "preexisting-binary"
    wasm_binding = "preexisting-binary"

    if not skip_build:
        cmd = [
            "cargo",
            "build",
            "--release",
            "--locked",
            "-p",
            "pkl-cli",
            "--bin",
            "pkl",
        ]
        if need_pkldoc:
            cmd.extend(["-p", "pkl-doc", "--bin", "pkldoc"])
        if need_probe:
            cmd.extend(["-p", "pkl-lang", "--example", "m7_bench_probe"])
        build_cap = capture_wait4(cmd, root, dict(os.environ), destination / "build/rust-release", timeout=600)
        if build_cap["status"] != "executed" or build_cap["exit_code"] != 0:
            err = raw_channel(build_cap, "stderr").decode("utf-8", errors="replace")
            raise RuntimeError(f"Rust release build failed (rc={build_cap['exit_code']}): {err[-1000:]}")
        build_record["commands"].append({"cmd": cmd, "capture": "build/rust-release/result.json"})
        pkl_binding = "recorded-build"
        if need_pkldoc:
            pkldoc_binding = "recorded-build"
        if need_probe:
            probe_binding = "recorded-build"

        if need_napi:
            if not node_bin:
                raise RuntimeError("node executable required to rebuild @pkl-rs/napi")
            napi_cmd = [node_bin, str(root / "packages/pkl-napi/scripts/build.mjs")]
            napi_cap = capture_wait4(napi_cmd, root, dict(os.environ), destination / "build/napi-build", timeout=600)
            if napi_cap["status"] != "executed" or napi_cap["exit_code"] != 0:
                err = raw_channel(napi_cap, "stderr").decode("utf-8", errors="replace")
                raise RuntimeError(f"@pkl-rs/napi build failed (rc={napi_cap['exit_code']}): {err[-1000:]}")
            build_record["commands"].append({"cmd": napi_cmd, "capture": "build/napi-build/result.json"})
            napi_binding = "recorded-build"

        if need_wasm:
            if not node_bin:
                raise RuntimeError("node executable required to rebuild @pkl-rs/wasm")
            wasm_env = dict(os.environ)
            m1_tools_bin = root / ".cache/m1-tools/bin"
            if m1_tools_bin.is_dir():
                wasm_env["PATH"] = f"{m1_tools_bin}{os.pathsep}{wasm_env.get('PATH', '')}"
            wasm_cmd = [node_bin, str(root / "packages/pkl-wasm/scripts/build.mjs")]
            wasm_cap = capture_wait4(wasm_cmd, root, wasm_env, destination / "build/wasm-build", timeout=600)
            if wasm_cap["status"] != "executed" or wasm_cap["exit_code"] != 0:
                err = raw_channel(wasm_cap, "stderr").decode("utf-8", errors="replace")
                raise RuntimeError(f"@pkl-rs/wasm build failed (rc={wasm_cap['exit_code']}): {err[-1000:]}")
            build_record["commands"].append({"cmd": wasm_cmd, "capture": "build/wasm-build/result.json"})
            wasm_binding = "recorded-build"

    required_paths = [
        ("pkl", pkl_bin, True, pkl_binding),
        ("pkldoc", pkldoc_bin, need_pkldoc, pkldoc_binding),
        ("m7_bench_probe", probe_bin, need_probe, probe_binding),
        ("wasm_bg", wasm_bg, need_wasm or full_catalog, wasm_binding),
    ]
    artifacts: dict[str, Any] = {}
    for label, path, is_required, binding in required_paths:
        if not path.is_file():
            if is_required:
                raise ValueError(f"missing required candidate artifact ({label}): {path}")
            continue
        artifacts[label] = {
            "path": str(path),
            "rel_path": path.relative_to(root).as_posix(),
            "sha256": file_digest(path),
            "bytes": path.stat().st_size,
            "source_binding": binding,
        }

    napi_addons = sorted(napi_node.glob("*.node")) if napi_node.is_dir() else []
    if not napi_addons:
        if need_napi or full_catalog:
            raise ValueError(f"missing compiled @pkl-rs/napi .node addon under {napi_node}")
    else:
        artifacts["napi_addon"] = {
            "path": str(napi_addons[0]),
            "rel_path": napi_addons[0].relative_to(root).as_posix(),
            "sha256": file_digest(napi_addons[0]),
            "bytes": napi_addons[0].stat().st_size,
            "source_binding": napi_binding,
        }

    build_record["artifacts"] = artifacts
    return build_record


def compile_benchmark_java(oracle_meta: dict[str, Any], destination: Path, root: Path) -> tuple[Path, dict[str, Any]]:
    classes = destination / "classes"
    classes.mkdir(parents=True, exist_ok=True)
    java_home = Path(oracle_meta["jdk"]["properties"]["java.home"])
    javac = java_home / "bin/javac"
    env = build_isolated_env(oracle_meta["environment"], destination / "compile-state")
    sources = [str(root / BENCHMARK_JAVA), str(root / JAVA_TYPED_BENCH)]
    cap = capture_wait4(
        [str(javac), "-encoding", "UTF-8", "-cp", oracle_meta["artifact"], "-d", str(classes), *sources],
        root,
        env,
        destination / "compile-java",
        timeout=120,
    )
    stdout, _ = verify_capture_raw(cap, expected_exit=0, helper=True)
    if stdout:
        raise ValueError("unexpected javac stdout when compiling Java benchmark harnesses")
    return classes, {
        "javac": str(javac),
        "javac_sha256": file_digest(javac),
        "benchmark_java_sha256": file_digest(root / BENCHMARK_JAVA),
        "java_typed_bench_sha256": file_digest(root / JAVA_TYPED_BENCH),
        "capture": cap,
    }


def _run_java_typed_persistent_ref(
    module: Path,
    folder: Path,
    oracle_meta: dict[str, Any],
    classes_dir: Path,
    root: Path,
    sampling: dict[str, Any],
) -> dict[str, Any]:
    """Run apples-to-apples Java `Evaluator.evaluate(source)` (`PModule`) reference for typed conversion."""
    pf_state = folder / "state/preflight-text"
    j_txt = capture_wait4(
        [
            *java_prefix_for_state(oracle_meta["command"], pf_state),
            "eval",
            "--no-cache",
            "--no-project",
            "--settings",
            "pkl:settings",
            "--color=never",
            str(module),
        ],
        root,
        build_isolated_env(oracle_meta["environment"], pf_state),
        folder / "preflight-text",
    )
    exp_text, _ = verify_capture_raw(j_txt, expected_exit=0)
    expected_text_path = folder / "expected-text.bin"
    expected_text_path.write_bytes(exp_text)

    timeout = max(float(sampling["maximum_warmup_seconds"]) + 180.0, 240.0)
    forks: list[dict[str, Any]] = []
    for idx in range(sampling["independent_api_forks"]):
        js = folder / f"state/java-typed-{idx}"
        first_out = folder / f"first-java-typed-{idx}.bin"
        prefix = java_prefix_for_state(oracle_meta["command"], js)
        cp = prefix.index("-cp")
        cmd = [
            *prefix[:cp],
            "-cp",
            os.pathsep.join((str(classes_dir), oracle_meta["artifact"])),
            "JavaTypedBench",
            str(module),
            str(expected_text_path),
            str(sampling["minimum_warmup_seconds"]),
            str(sampling["maximum_warmup_seconds"]),
            str(sampling["samples"]),
            str(sampling["sample_seconds"]),
            str(sampling["convergence_windows"]),
            str(sampling["maximum_relative_spread"]),
            str(first_out),
        ]
        cap = capture_wait4(
            cmd,
            root,
            build_isolated_env(oracle_meta["environment"], js),
            folder / f"java-typed/{idx}",
            timeout=timeout,
        )
        if first_out.read_bytes() != exp_text:
            raise ValueError(f"JavaTypedBench fork {idx} first output.text mismatch")
        parsed = parse_probe_series(
            cap,
            sampling,
            "persistent",
            "Whole Java JVM fork calling Evaluator.evaluate(ModuleSource) (PModule materialization) via os.wait4",
        )
        forks.append({"capture": cap, **parsed})

    return {
        "warmup": "converged" if all(f["warmup"] == "converged" for f in forks) else "inconclusive",
        "fork_median_operation_ns": distribution([f["operation_ns"]["median"] for f in forks]),
        "fork_peak_rss_bytes": distribution([f["peak_rss_bytes"] for f in forks]),
        "forks": forks,
    }


def _run_cold_cli_pair(
    name: str,
    family: str,
    folder: Path,
    java_cmd_fn,
    rust_cmd_fn,
    oracle_meta: dict[str, Any] | None,
    root: Path,
    cold_samples: int,
    *,
    native_oracle_meta: dict[str, Any] | None = None,
    native_cmd_fn=None,
    allow_native_jline_stderr: bool = False,
    expected_error: bool = False,
    expected_exit_override: int | None = None,
    stdin_file: Path | None = None,
    m0_ref_case: dict[str, Any] | None = None,
    description: str = "",
) -> dict[str, Any]:
    if oracle_meta is None and native_oracle_meta is None:
        raise ValueError(f"{name}: at least one baseline oracle (JVM or native-image) is required")
    if native_oracle_meta is not None and native_cmd_fn is None:
        raise ValueError(f"{name}: native_cmd_fn is required when native_oracle_meta is provided")

    expected_exit = expected_exit_override if expected_exit_override is not None else (1 if expected_error else 0)
    base_env = (oracle_meta or native_oracle_meta)["environment"]  # type: ignore[index]
    ni_pf_err: bytes | None = None

    # Preflight oracle(s) + replay + Rust (100% raw-byte check BEFORE timing!)
    if oracle_meta is not None:
        j_pf_state = folder / "state/java-preflight"
        j_pf = capture_wait4(
            java_cmd_fn(j_pf_state),
            root,
            build_isolated_env(base_env, j_pf_state),
            folder / "preflight-java",
            stdin_file=stdin_file,
        )
        exp_out, exp_err = verify_capture_raw(j_pf, expected_exit=expected_exit)
        if not expected_error and exp_err:
            raise ValueError(f"{name}: unexpected Java preflight stderr: {exp_err[:300]!r}")
        if expected_error and (exp_out or not exp_err):
            raise ValueError(f"{name}: unexpected Java error-case channels")

        j_rep_state = folder / "state/java-replay"
        j_rep = capture_wait4(
            java_cmd_fn(j_rep_state),
            root,
            build_isolated_env(base_env, j_rep_state),
            folder / "preflight-java-replay",
            stdin_file=stdin_file,
        )
        verify_capture_raw(j_rep, expected_exit=expected_exit, expected_stdout=exp_out, expected_stderr=exp_err)

        if native_oracle_meta is not None:
            ni_pf_state = folder / "state/native-image-preflight"
            ni_pf = capture_wait4(
                native_cmd_fn(ni_pf_state),
                root,
                build_isolated_env(base_env, ni_pf_state),
                folder / "preflight-native-image",
                stdin_file=stdin_file,
            )
            if allow_native_jline_stderr:
                _, ni_pf_err = verify_capture_raw(ni_pf, expected_exit=expected_exit, expected_stdout=exp_out)
                if not is_benign_jline_repl_stderr(ni_pf_err):
                    raise ValueError(
                        f"{name}: unexpected official native-image repl stderr: {ni_pf_err[:300]!r}"
                    )
            else:
                _, ni_pf_err = verify_capture_raw(
                    ni_pf, expected_exit=expected_exit, expected_stdout=exp_out, expected_stderr=exp_err
                )
    else:
        ni_pf_state = folder / "state/native-image-preflight"
        ni_pf = capture_wait4(
            native_cmd_fn(ni_pf_state),
            root,
            build_isolated_env(base_env, ni_pf_state),
            folder / "preflight-native-image",
            stdin_file=stdin_file,
        )
        ni_out, ni_pf_err = verify_capture_raw(ni_pf, expected_exit=expected_exit)
        if allow_native_jline_stderr:
            if not is_benign_jline_repl_stderr(ni_pf_err):
                raise ValueError(
                    f"{name}: unexpected official native-image repl stderr: {ni_pf_err[:300]!r}"
                )
            exp_out, exp_err = ni_out, b""
        else:
            if not expected_error and ni_pf_err:
                raise ValueError(f"{name}: unexpected native-image preflight stderr: {ni_pf_err[:300]!r}")
            if expected_error and (ni_out or not ni_pf_err):
                raise ValueError(f"{name}: unexpected native-image error-case channels")
            exp_out, exp_err = ni_out, ni_pf_err

        ni_rep_state = folder / "state/native-image-replay"
        ni_rep = capture_wait4(
            native_cmd_fn(ni_rep_state),
            root,
            build_isolated_env(base_env, ni_rep_state),
            folder / "preflight-native-image-replay",
            stdin_file=stdin_file,
        )
        if allow_native_jline_stderr:
            _, ni_rep_err = verify_capture_raw(ni_rep, expected_exit=expected_exit, expected_stdout=exp_out)
            if not is_benign_jline_repl_stderr(ni_rep_err):
                raise ValueError(
                    f"{name}: unexpected official native-image replay repl stderr: {ni_rep_err[:300]!r}"
                )
        else:
            verify_capture_raw(ni_rep, expected_exit=expected_exit, expected_stdout=exp_out, expected_stderr=exp_err)

    # Rust preflight ALWAYS checks 100% raw stdout AND raw stderr (including stderr == b"" for repl-session!)
    r_pf_state = folder / "state/rust-preflight"
    r_pf = capture_wait4(
        rust_cmd_fn(r_pf_state),
        root,
        build_isolated_env(base_env, r_pf_state),
        folder / "preflight-rust",
        stdin_file=stdin_file,
    )
    verify_capture_raw(r_pf, expected_exit=expected_exit, expected_stdout=exp_out, expected_stderr=exp_err)

    java_samples: list[dict[str, Any]] = []
    native_samples: list[dict[str, Any]] = []
    rust_samples: list[dict[str, Any]] = []
    for idx in range(cold_samples):
        if oracle_meta is not None:
            js = folder / f"state/java-cold-{idx}"
            jc = capture_wait4(
                java_cmd_fn(js),
                root,
                build_isolated_env(base_env, js),
                folder / "java-cold" / str(idx),
                stdin_file=stdin_file,
            )
            verify_capture_raw(jc, expected_exit=expected_exit, expected_stdout=exp_out, expected_stderr=exp_err)
            java_samples.append(jc)

        if native_oracle_meta is not None:
            ns = folder / f"state/native-image-cold-{idx}"
            nc = capture_wait4(
                native_cmd_fn(ns),
                root,
                build_isolated_env(base_env, ns),
                folder / "native-image-cold" / str(idx),
                stdin_file=stdin_file,
            )
            if allow_native_jline_stderr:
                _, n_err = verify_capture_raw(nc, expected_exit=expected_exit, expected_stdout=exp_out)
                if not is_benign_jline_repl_stderr(n_err):
                    raise ValueError(
                        f"{name}: unexpected official native-image sample {idx} repl stderr: {n_err[:300]!r}"
                    )
            else:
                verify_capture_raw(nc, expected_exit=expected_exit, expected_stdout=exp_out, expected_stderr=exp_err)
            native_samples.append(nc)

        rs = folder / f"state/rust-cold-{idx}"
        rc = capture_wait4(
            rust_cmd_fn(rs),
            root,
            build_isolated_env(base_env, rs),
            folder / "rust-cold" / str(idx),
            stdin_file=stdin_file,
        )
        verify_capture_raw(rc, expected_exit=expected_exit, expected_stdout=exp_out, expected_stderr=exp_err)
        rust_samples.append(rc)

    r_time = distribution([s["elapsed_ns"] for s in rust_samples])
    r_rss = distribution([s["peak_rss_bytes"] for s in rust_samples])

    java_block = None
    vs_temurin = None
    if oracle_meta is not None:
        j_time = distribution([s["elapsed_ns"] for s in java_samples])
        j_rss = distribution([s["peak_rss_bytes"] for s in java_samples])
        java_block = {
            "elapsed_ns": j_time,
            "peak_rss_bytes": j_rss,
            "samples": java_samples,
            "memory_boundary": "Whole Java JVM cold CLI process via os.wait4 ru_maxrss",
        }
        vs_temurin = evaluate_ratio_verdict(
            r_time["median"],
            j_time["median"],
            r_rss["median"],
            j_rss["median"],
            correctness_verified=True,
            converged=True,
        )

    native_block = None
    vs_native = None
    if native_oracle_meta is not None:
        n_time = distribution([s["elapsed_ns"] for s in native_samples])
        n_rss = distribution([s["peak_rss_bytes"] for s in native_samples])
        native_block = {
            "elapsed_ns": n_time,
            "peak_rss_bytes": n_rss,
            "samples": native_samples,
            "memory_boundary": "Whole Official Apple Pkl 0.32.0 native-image cold CLI process via os.wait4 ru_maxrss",
        }
        if allow_native_jline_stderr and ni_pf_err is not None:
            native_block["stderr_policy"] = "jline-history-log-recorded"
            native_block["preflight_stderr_bytes"] = len(ni_pf_err)
            native_block["preflight_stderr_sha256"] = digest(ni_pf_err)
            native_block["raw_stderr_bytes_match"] = ni_pf_err == b""
            native_block["sample_stderr_bytes"] = [s["stderr"]["bytes"] for s in native_samples]
            native_block["stderr_note"] = REPL_JLINE_STDERR_NOTE
        vs_native = evaluate_ratio_verdict(
            r_time["median"],
            n_time["median"],
            r_rss["median"],
            n_rss["median"],
            correctness_verified=True,
            converged=True,
            baseline_key_prefix="native_image",
        )

    vs_graalvm = None
    if m0_ref_case is not None:
        m0_cold = m0_ref_case["cold_cli"]
        vs_graalvm = evaluate_ratio_verdict(
            r_time["median"],
            m0_cold["elapsed_ns"]["median"],
            r_rss["median"],
            m0_cold["peak_rss_bytes"]["median"],
            correctness_verified=True,
            converged=True,
        )

    return {
        "id": f"{name}:cold-cli",
        "workload_id": name,
        "family": family,
        "mode": "cold-cli",
        "description": description,
        "expected_error": expected_error,
        "correctness_verified": True,
        "expected_stdout_sha256": digest(exp_out),
        "expected_stderr_sha256": digest(exp_err),
        "java": java_block,
        "native_image": native_block,
        "rust": {
            "elapsed_ns": r_time,
            "peak_rss_bytes": r_rss,
            "samples": rust_samples,
            "memory_boundary": "Whole native Rust cold CLI process via os.wait4 ru_maxrss",
        },
        "vs_official_native_image": vs_native,
        "vs_live_temurin_21": vs_temurin,
        "vs_m0_graalvm_25_0_1": vs_graalvm,
    }


def _run_api_probe_pair(
    name: str,
    family: str,
    mode: str,
    module: Path,
    folder: Path,
    oracle_meta: dict[str, Any],
    classes_dir: Path,
    probe_bin: Path,
    root: Path,
    sampling: dict[str, Any],
    *,
    expected_error: bool = False,
    m0_ref_case: dict[str, Any] | None = None,
    description: str = "",
) -> dict[str, Any]:
    expected_api = folder / "expected-api.bin"
    if not expected_api.exists():
        pf_state = folder / "state/api-preflight-java"
        j_verify = java_benchmark_cmd(
            oracle_meta,
            classes_dir,
            pf_state,
            "verify",
            module,
            expected_api,
            expected_error,
            sampling,
            folder / "first-api-java.bin",
            write_expected=True,
        )
        j_cap = capture_wait4(
            j_verify,
            root,
            build_isolated_env(oracle_meta["environment"], pf_state),
            folder / "preflight-api-java",
        )
        verify_capture_raw(j_cap, expected_exit=0, helper=True)

        pf_r_state = folder / "state/api-preflight-rust"
        r_verify = rust_probe_cmd(
            probe_bin,
            "verify",
            module,
            expected_api,
            expected_error,
            sampling,
            folder / "first-api-rust.bin",
            write_expected=False,
        )
        r_cap = capture_wait4(
            r_verify,
            root,
            build_isolated_env(oracle_meta["environment"], pf_r_state),
            folder / "preflight-api-rust",
        )
        verify_capture_raw(r_cap, expected_exit=0, helper=True)
        if (folder / "first-api-rust.bin").read_bytes() != expected_api.read_bytes():
            raise ValueError(f"{name}:{mode}: Rust API preflight bytes differ from Java")

    api_bytes = expected_api.read_bytes()
    timeout = max(float(sampling["maximum_warmup_seconds"]) + 180.0, 240.0)

    java_forks: list[dict[str, Any]] = []
    rust_forks: list[dict[str, Any]] = []
    for idx in range(sampling["independent_api_forks"]):
        js = folder / f"state/java-{mode}-{idx}"
        j_first = folder / f"first-java-{mode}-{idx}.bin"
        j_cmd = java_benchmark_cmd(
            oracle_meta,
            classes_dir,
            js,
            mode,
            module,
            expected_api,
            expected_error,
            sampling,
            j_first,
        )
        j_cap = capture_wait4(
            j_cmd,
            root,
            build_isolated_env(oracle_meta["environment"], js),
            folder / f"java-{mode}" / str(idx),
            timeout=timeout,
        )
        if j_first.read_bytes() != api_bytes:
            raise ValueError(f"{name}:{mode}: Java fork {idx} first bytes differ")
        j_parsed = parse_probe_series(
            j_cap,
            sampling,
            mode,
            "Whole Java JVM fork including init, verification, warmup, and measurement via os.wait4",
        )
        java_forks.append({"capture": j_cap, **j_parsed})

        rs = folder / f"state/rust-{mode}-{idx}"
        r_first = folder / f"first-rust-{mode}-{idx}.bin"
        r_cmd = rust_probe_cmd(
            probe_bin,
            mode,
            module,
            expected_api,
            expected_error,
            sampling,
            r_first,
        )
        r_cap = capture_wait4(
            r_cmd,
            root,
            build_isolated_env(oracle_meta["environment"], rs),
            folder / f"rust-{mode}" / str(idx),
            timeout=timeout,
        )
        if r_first.read_bytes() != api_bytes:
            raise ValueError(f"{name}:{mode}: Rust fork {idx} first bytes differ")
        r_parsed = parse_probe_series(
            r_cap,
            sampling,
            mode,
            "Whole native Rust probe fork including init, verification, warmup, and measurement via os.wait4",
        )
        rust_forks.append({"capture": r_cap, **r_parsed})

    j_warmup = "converged" if all(f["warmup"] == "converged" for f in java_forks) else "inconclusive"
    r_warmup = "converged" if all(f["warmup"] == "converged" for f in rust_forks) else "inconclusive"
    j_op_dist = distribution([f["operation_ns"]["median"] for f in java_forks])
    j_rss_dist = distribution([f["peak_rss_bytes"] for f in java_forks])
    r_op_dist = distribution([f["operation_ns"]["median"] for f in rust_forks])
    r_rss_dist = distribution([f["peak_rss_bytes"] for f in rust_forks])

    vs_temurin = evaluate_ratio_verdict(
        r_op_dist["median"],
        j_op_dist["median"],
        r_rss_dist["median"],
        j_rss_dist["median"],
        correctness_verified=True,
        converged=(j_warmup == "converged" and r_warmup == "converged"),
    )

    vs_graalvm = None
    if m0_ref_case is not None and mode in m0_ref_case.get("api", {}):
        m0_api = m0_ref_case["api"][mode]
        m0_converged = m0_api["warmup"] == "converged" and r_warmup == "converged"
        vs_graalvm = evaluate_ratio_verdict(
            r_op_dist["median"],
            m0_api["fork_median_operation_ns"]["median"],
            r_rss_dist["median"],
            m0_api["fork_peak_rss_bytes"]["median"],
            correctness_verified=True,
            converged=m0_converged,
        )

    return {
        "id": f"{name}:{mode}",
        "workload_id": name,
        "family": family,
        "mode": mode,
        "description": description,
        "expected_error": expected_error,
        "correctness_verified": True,
        "expected_api_sha256": digest(api_bytes),
        "expected_api_bytes": len(api_bytes),
        "java": {
            "warmup": j_warmup,
            "fork_median_operation_ns": j_op_dist,
            "fork_peak_rss_bytes": j_rss_dist,
            "forks": java_forks,
        },
        "native_image": None,
        "native_image_not_applicable_reason": NATIVE_IMAGE_API_NA_REASON,
        "rust": {
            "warmup": r_warmup,
            "fork_median_operation_ns": r_op_dist,
            "fork_peak_rss_bytes": r_rss_dist,
            "forks": rust_forks,
        },
        "vs_official_native_image": None,
        "vs_live_temurin_21": vs_temurin,
        "vs_m0_graalvm_25_0_1": vs_graalvm,
    }


def _run_output_files_workload(
    workload: dict[str, Any],
    folder: Path,
    oracle_meta: dict[str, Any] | None,
    pkl_bin: Path,
    root: Path,
    cold_samples: int,
    *,
    native_oracle_meta: dict[str, Any] | None = None,
) -> dict[str, Any]:
    if oracle_meta is None and native_oracle_meta is None:
        raise ValueError("renderers-files requires at least one baseline oracle")
    wid = workload["id"]
    module = (root / workload["module"]).resolve()
    base_env = (oracle_meta or native_oracle_meta)["environment"]  # type: ignore[index]

    shared_out_dir = folder / "shared-output-files"

    def run_once(runner: str, tag: str) -> tuple[dict[str, Any], dict[str, bytes]]:
        state = folder / f"state/{tag}"
        if shared_out_dir.exists():
            shutil.rmtree(shared_out_dir)
        shared_out_dir.mkdir(parents=True, exist_ok=True)
        out_dir = shared_out_dir
        common = [
            "eval",
            "--no-cache",
            "--no-project",
            "--settings",
            "pkl:settings",
            "--color=never",
            "-m",
            str(out_dir),
            str(module),
        ]
        if runner == "rust":
            cmd = [str(pkl_bin), *common]
        elif runner == "native-image":
            assert native_oracle_meta is not None
            cmd = [*native_oracle_meta["command"], *common]
        else:
            assert oracle_meta is not None
            cmd = [*java_prefix_for_state(oracle_meta["command"], state), *common]
        cap = capture_wait4(cmd, root, build_isolated_env(base_env, state), folder / tag)
        files = m4_differential.collect_tree_bytes(out_dir, set(), prefix="")
        shutil.rmtree(out_dir, ignore_errors=True)
        return cap, files

    if oracle_meta is not None:
        j_pf, exp_files = run_once("java", "preflight-java")
        _, j_err = verify_capture_raw(j_pf, expected_exit=0)
        if j_err or not exp_files:
            raise ValueError(f"{wid}: Java preflight failed or emitted no files")
        if native_oracle_meta is not None:
            ni_pf, ni_files = run_once("native-image", "preflight-native-image")
            _, ni_err = verify_capture_raw(ni_pf, expected_exit=0)
            if ni_err or ni_files != exp_files:
                raise ValueError(f"{wid}: official native-image generated output.files differ from Java preflight")
    else:
        ni_pf, exp_files = run_once("native-image", "preflight-native-image")
        _, ni_err = verify_capture_raw(ni_pf, expected_exit=0)
        if ni_err or not exp_files:
            raise ValueError(f"{wid}: official native-image preflight failed or emitted no files")

    r_pf, r_files = run_once("rust", "preflight-rust")
    _, r_err = verify_capture_raw(r_pf, expected_exit=0)
    if r_err or r_files != exp_files:
        raise ValueError(f"{wid}: Rust generated output.files differ from oracle preflight")

    java_samples, native_samples, rust_samples = [], [], []
    for idx in range(cold_samples):
        if oracle_meta is not None:
            jc, jf = run_once("java", f"java-cold/{idx}")
            verify_capture_raw(jc, expected_exit=0, expected_stderr=b"")
            if jf != exp_files:
                raise ValueError(f"{wid}: Java sample {idx} files drifted")
            java_samples.append(jc)

        if native_oracle_meta is not None:
            nc, nf = run_once("native-image", f"native-image-cold/{idx}")
            verify_capture_raw(nc, expected_exit=0, expected_stderr=b"")
            if nf != exp_files:
                raise ValueError(f"{wid}: official native-image sample {idx} files drifted")
            native_samples.append(nc)

        rc, rf = run_once("rust", f"rust-cold/{idx}")
        verify_capture_raw(rc, expected_exit=0, expected_stderr=b"")
        if rf != exp_files:
            raise ValueError(f"{wid}: Rust sample {idx} files differ from oracle")
        rust_samples.append(rc)

    r_time = distribution([s["elapsed_ns"] for s in rust_samples])
    r_rss = distribution([s["peak_rss_bytes"] for s in rust_samples])

    java_block = None
    vs_temurin = None
    if oracle_meta is not None:
        j_time = distribution([s["elapsed_ns"] for s in java_samples])
        j_rss = distribution([s["peak_rss_bytes"] for s in java_samples])
        java_block = {
            "elapsed_ns": j_time,
            "peak_rss_bytes": j_rss,
            "samples": java_samples,
            "memory_boundary": "Whole Java JVM cold CLI process via os.wait4 ru_maxrss",
        }
        vs_temurin = evaluate_ratio_verdict(
            r_time["median"], j_time["median"], r_rss["median"], j_rss["median"], correctness_verified=True
        )

    native_block = None
    vs_native = None
    if native_oracle_meta is not None:
        n_time = distribution([s["elapsed_ns"] for s in native_samples])
        n_rss = distribution([s["peak_rss_bytes"] for s in native_samples])
        native_block = {
            "elapsed_ns": n_time,
            "peak_rss_bytes": n_rss,
            "samples": native_samples,
            "memory_boundary": "Whole Official Apple Pkl 0.32.0 native-image cold CLI process via os.wait4 ru_maxrss",
        }
        vs_native = evaluate_ratio_verdict(
            r_time["median"],
            n_time["median"],
            r_rss["median"],
            n_rss["median"],
            correctness_verified=True,
            baseline_key_prefix="native_image",
        )

    return {
        "id": f"{wid}:cold-cli",
        "workload_id": wid,
        "family": workload["family"],
        "mode": "cold-cli",
        "description": workload.get("description", ""),
        "expected_error": False,
        "correctness_verified": True,
        "generated_files_verified": {k: digest(v) for k, v in sorted(exp_files.items())},
        "java": java_block,
        "native_image": native_block,
        "rust": {
            "elapsed_ns": r_time,
            "peak_rss_bytes": r_rss,
            "samples": rust_samples,
            "memory_boundary": "Whole native Rust cold CLI process via os.wait4 ru_maxrss",
        },
        "vs_official_native_image": vs_native,
        "vs_live_temurin_21": vs_temurin,
        "vs_m0_graalvm_25_0_1": None,
    }


def _run_pkldoc_workload(
    workload: dict[str, Any],
    folder: Path,
    oracle_meta: dict[str, Any] | None,
    pkldoc_bin: Path,
    root: Path,
    cold_samples: int,
    *,
    native_oracle_meta: dict[str, Any] | None = None,
) -> dict[str, Any]:
    if oracle_meta is None and native_oracle_meta is None:
        raise ValueError("pkldoc workload requires at least one baseline oracle")
    wid = workload["id"]
    input_dir = (root / workload["input_dir"]).resolve()
    input_files = sorted(str(p) for p in input_dir.rglob("*") if p.is_file())
    base_env = (oracle_meta or native_oracle_meta)["environment"]  # type: ignore[index]

    shared_site_dir = folder / "shared-pkldoc-site"

    def run_once(runner: str, tag: str) -> tuple[dict[str, Any], dict[str, bytes]]:
        state = folder / f"state/{tag}"
        if shared_site_dir.exists():
            shutil.rmtree(shared_site_dir)
        shared_site_dir.mkdir(parents=True, exist_ok=True)
        out_dir = shared_site_dir
        args = ["--output-dir", str(out_dir), "--no-cache", "--test-mode", *input_files]
        if runner == "rust":
            cmd = [str(pkldoc_bin), *args]
        elif runner == "native-image":
            assert native_oracle_meta is not None
            cmd = [*native_oracle_meta["pkldoc_command"], *args]
        else:
            assert oracle_meta is not None
            cmd = [*java_prefix_for_state(oracle_meta["command"], state, main_class="org.pkl.doc.Main"), *args]
        cap = capture_wait4(cmd, root, build_isolated_env(base_env, state), folder / tag)
        tree = m5_pkldoc_tree.collect_pkldoc_tree(out_dir)
        shutil.rmtree(out_dir, ignore_errors=True)
        return cap, tree

    if oracle_meta is not None:
        j_pf, exp_tree = run_once("java", "preflight-java")
        ref_stdout, j_err = verify_capture_raw(j_pf, expected_exit=0)
        if j_err or not exp_tree:
            raise ValueError(f"{wid}: Java pkldoc preflight failed or generated empty site")
        if native_oracle_meta is not None:
            ni_pf, ni_tree = run_once("native-image", "preflight-native-image")
            ni_stdout, ni_err = verify_capture_raw(ni_pf, expected_exit=0)
            if ni_err or ni_tree != exp_tree:
                raise ValueError(
                    f"{wid}: official native-image pkldoc tree differs from Java ({len(ni_tree)} vs {len(exp_tree)})"
                )
            ns_ok, ns_det = m5_differential.check_pkldoc_progress_stdout(ref_stdout, ni_stdout)
            if not ns_ok:
                raise ValueError(f"{wid}: official native-image pkldoc progress stdout failed: {ns_det}")
    else:
        ni_pf, exp_tree = run_once("native-image", "preflight-native-image")
        ref_stdout, ni_err = verify_capture_raw(ni_pf, expected_exit=0)
        if ni_err or not exp_tree:
            raise ValueError(f"{wid}: official native-image pkldoc preflight failed or generated empty site")

    r_pf, r_tree = run_once("rust", "preflight-rust")
    r_stdout, r_err = verify_capture_raw(r_pf, expected_exit=0)
    r_tree_diffs = m5_pkldoc_tree.compare_pkldoc_trees(exp_tree, r_tree, root)
    if r_err or r_tree_diffs:
        raise ValueError(
            f"{wid}: Rust pkldoc generated file/symlink tree differs from oracle "
            f"({len(r_tree)} vs {len(exp_tree)} entries, diffs={r_tree_diffs[:8]})"
        )
    stdout_ok, stdout_detail = m5_differential.check_pkldoc_progress_stdout(ref_stdout, r_stdout)
    if not stdout_ok:
        raise ValueError(f"{wid}: pkldoc progress stdout contract failed: {stdout_detail}")

    java_samples, native_samples, rust_samples = [], [], []
    for idx in range(cold_samples):
        sample_ref_stdout = ref_stdout
        if oracle_meta is not None:
            jc, jt = run_once("java", f"java-cold/{idx}")
            js_out, _ = verify_capture_raw(jc, expected_exit=0, expected_stderr=b"")
            if jt != exp_tree:
                raise ValueError(f"{wid}: Java sample {idx} pkldoc tree drifted")
            sample_ref_stdout = js_out
            java_samples.append(jc)

        if native_oracle_meta is not None:
            nc, nt = run_once("native-image", f"native-image-cold/{idx}")
            ns_out, _ = verify_capture_raw(nc, expected_exit=0, expected_stderr=b"")
            if nt != exp_tree:
                raise ValueError(f"{wid}: official native-image sample {idx} pkldoc tree drifted")
            s_ok, s_det = m5_differential.check_pkldoc_progress_stdout(sample_ref_stdout, ns_out)
            if not s_ok:
                raise ValueError(f"{wid}: official native-image sample {idx} pkldoc stdout failed: {s_det}")
            native_samples.append(nc)

        rc, rt = run_once("rust", f"rust-cold/{idx}")
        rs_out, _ = verify_capture_raw(rc, expected_exit=0, expected_stderr=b"")
        rt_diffs = m5_pkldoc_tree.compare_pkldoc_trees(exp_tree, rt, root)
        if rt_diffs:
            raise ValueError(f"{wid}: Rust sample {idx} pkldoc tree differs from oracle ({rt_diffs[:8]})")
        s_ok, s_det = m5_differential.check_pkldoc_progress_stdout(sample_ref_stdout, rs_out)
        if not s_ok:
            raise ValueError(f"{wid}: sample {idx} pkldoc progress stdout contract failed: {s_det}")
        rust_samples.append(rc)

    r_time = distribution([s["elapsed_ns"] for s in rust_samples])
    r_rss = distribution([s["peak_rss_bytes"] for s in rust_samples])

    java_block = None
    vs_temurin = None
    if oracle_meta is not None:
        j_time = distribution([s["elapsed_ns"] for s in java_samples])
        j_rss = distribution([s["peak_rss_bytes"] for s in java_samples])
        java_block = {
            "elapsed_ns": j_time,
            "peak_rss_bytes": j_rss,
            "samples": java_samples,
            "memory_boundary": "Whole Java JVM pkldoc process via os.wait4 ru_maxrss",
        }
        vs_temurin = evaluate_ratio_verdict(
            r_time["median"], j_time["median"], r_rss["median"], j_rss["median"], correctness_verified=True
        )

    native_block = None
    vs_native = None
    if native_oracle_meta is not None:
        n_time = distribution([s["elapsed_ns"] for s in native_samples])
        n_rss = distribution([s["peak_rss_bytes"] for s in native_samples])
        native_block = {
            "elapsed_ns": n_time,
            "peak_rss_bytes": n_rss,
            "samples": native_samples,
            "memory_boundary": "Whole Official Apple Pkl 0.32.0 native-image pkldoc process via os.wait4 ru_maxrss",
        }
        vs_native = evaluate_ratio_verdict(
            r_time["median"],
            n_time["median"],
            r_rss["median"],
            n_rss["median"],
            correctness_verified=True,
            baseline_key_prefix="native_image",
        )

    return {
        "id": f"{wid}:cold-cli",
        "workload_id": wid,
        "family": workload["family"],
        "mode": "cold-cli",
        "description": workload.get("description", ""),
        "expected_error": False,
        "correctness_verified": True,
        "comparison_mode": "pkldoc-progress",
        "raw_stdout_bytes_match": ref_stdout == r_stdout,
        "pkldoc_stdout_contract_ok": True,
        "pkldoc_tree_summary": m5_pkldoc_tree.summarize_pkldoc_tree(exp_tree),
        "java": java_block,
        "native_image": native_block,
        "rust": {
            "elapsed_ns": r_time,
            "peak_rss_bytes": r_rss,
            "samples": rust_samples,
            "memory_boundary": "Whole native Rust pkldoc process via os.wait4 ru_maxrss",
        },
        "vs_official_native_image": vs_native,
        "vs_live_temurin_21": vs_temurin,
        "vs_m0_graalvm_25_0_1": None,
    }


def _run_server_session_wait4(
    command: list[str],
    cwd: Path,
    env: dict[str, str],
    module_uri: str,
    steady_requests: int,
    destination: Path,
    *,
    expected_result_bytes: bytes | None = None,
    timeout: float = 120.0,
) -> dict[str, Any]:
    """Drive `pkl server` (0x20-0x25) while capturing per-request latency and exact `os.wait4` RSS."""
    if not hasattr(os, "wait4"):
        raise ValueError("os.wait4 unavailable")
    destination.mkdir(parents=True, exist_ok=False)
    stderr_path = destination / "stderr.bin"
    stderr_fh = stderr_path.open("wb")
    started = time.perf_counter_ns()
    proc = subprocess.Popen(
        command,
        cwd=cwd,
        env=env,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=stderr_fh,
        start_new_session=True,
    )
    assert proc.stdin is not None and proc.stdout is not None

    msg_q: queue.Queue[tuple[str, Any]] = queue.Queue()

    def _reader() -> None:
        try:
            while True:
                item = m5_server_client.read_message(proc.stdout)
                if item is None:
                    msg_q.put(("eof", None))
                    return
                msg_q.put(("msg", item))
        except BaseException as exc:
            msg_q.put(("err", str(exc)))

    t = threading.Thread(target=_reader, daemon=True)
    t.start()

    def recv(deadline: float) -> tuple[int, dict[str, Any]]:
        rem = deadline - time.monotonic()
        if rem <= 0:
            raise TimeoutError("server response timed out")
        kind, payload = msg_q.get(timeout=rem)
        if kind != "msg":
            raise RuntimeError(f"server reader ended unexpectedly: {kind}={payload}")
        return payload

    deadline = time.monotonic() + timeout
    request_latencies_ns: list[int] = []
    first_bytes = b""
    try:
        t_init = time.perf_counter_ns()
        m5_server_client.write_message(
            proc.stdin,
            m5_server_client.CREATE_EVALUATOR_REQUEST,
            {"requestId": 1, "allowedModules": [".*"], "allowedResources": [".*"], "color": False},
        )
        code, resp = recv(deadline)
        if code != m5_server_client.CREATE_EVALUATOR_RESPONSE or "evaluatorId" not in resp:
            raise ValueError(f"unexpected CreateEvaluatorResponse: code=0x{code:02x} resp={resp}")
        eid = resp["evaluatorId"]
        init_ns = time.perf_counter_ns() - t_init

        # Warmup (20 requests) + steady_requests timed requests
        total_ops = 20 + steady_requests
        for i in range(total_ops):
            req_id = 100 + i
            t0 = time.perf_counter_ns()
            m5_server_client.write_message(
                proc.stdin,
                m5_server_client.EVALUATE_REQUEST,
                {"requestId": req_id, "evaluatorId": eid, "moduleUri": module_uri, "expr": "run(0)"},
            )
            rc, rbody = recv(deadline)
            dt = time.perf_counter_ns() - t0
            if rc != m5_server_client.EVALUATE_RESPONSE or rbody.get("requestId") != req_id or "error" in rbody:
                raise ValueError(f"unexpected EvaluateResponse on op {i}: code=0x{rc:02x} body={rbody}")
            res_bytes = rbody.get("result")
            if not isinstance(res_bytes, bytes):
                raise ValueError(f"missing binary result on op {i}")
            if i == 0:
                first_bytes = res_bytes
                if expected_result_bytes is not None and res_bytes != expected_result_bytes:
                    raise ValueError(
                        f"server binary result differs from Java preflight ({len(res_bytes)}B vs {len(expected_result_bytes)}B)"
                    )
            else:
                ref = expected_result_bytes if expected_result_bytes is not None else first_bytes
                if res_bytes != ref:
                    raise ValueError(f"server binary result drifted on op {i}")
            if i >= 20:
                request_latencies_ns.append(dt)

        m5_server_client.write_message(proc.stdin, m5_server_client.CLOSE_EVALUATOR, {"evaluatorId": eid})
        proc.stdin.close()
        wait_timed_out = threading.Event()
        rem_wait = max(1.0, deadline - time.monotonic())

        def _expire_wait() -> None:
            try:
                os.killpg(proc.pid, signal.SIGKILL)
                wait_timed_out.set()
            except (ProcessLookupError, PermissionError):
                pass

        wait_timer = threading.Timer(rem_wait, _expire_wait)
        wait_timer.daemon = True
        wait_timer.start()
        try:
            _, status, usage = os.wait4(proc.pid, 0)
            elapsed_ns = time.perf_counter_ns() - started
            exit_code = os.waitstatus_to_exitcode(status)
        finally:
            wait_timer.cancel()
        if wait_timed_out.is_set():
            raise TimeoutError("server process timed out during shutdown wait4")
    except BaseException:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
        try:
            os.wait4(proc.pid, 0)
        except ChildProcessError:
            pass
        raise
    finally:
        stderr_fh.close()
        t.join(timeout=2.0)

    stderr_bytes = stderr_path.read_bytes()
    if exit_code != 0 or stderr_bytes:
        raise ValueError(
            f"server exited rc={exit_code}, stderr={stderr_bytes[:300].decode('utf-8', errors='replace')}"
        )
    (destination / "first-result.bin").write_bytes(first_bytes)
    rec = {
        "command": command,
        "status": "executed",
        "exit_code": exit_code,
        "elapsed_ns": elapsed_ns,
        "init_ns": init_ns,
        "peak_rss_bytes": per_process_rss_bytes(usage),
        "first_result_sha256": digest(first_bytes),
        "first_result_bytes": len(first_bytes),
        "steady_request_ns": distribution(request_latencies_ns),
        "stderr_sha256": digest(stderr_bytes),
    }
    write_json(destination / "result.json", rec)
    return rec


def _run_server_workload(
    workload: dict[str, Any],
    folder: Path,
    oracle_meta: dict[str, Any] | None,
    pkl_bin: Path,
    root: Path,
    profile_cfg: dict[str, Any],
    *,
    native_oracle_meta: dict[str, Any] | None = None,
) -> dict[str, Any]:
    if oracle_meta is None and native_oracle_meta is None:
        raise ValueError("server workload requires at least one baseline oracle")
    wid = workload["id"]
    module_uri = (root / workload["module"]).resolve().as_uri()
    steady_reqs = int(profile_cfg["server_steady_requests"])
    forks = int(profile_cfg["independent_api_forks"])
    base_env = (oracle_meta or native_oracle_meta)["environment"]  # type: ignore[index]

    # Preflight oracle(s) -> get exact expected MessagePack PklBinary result bytes
    if oracle_meta is not None:
        pf_j_state = folder / "state/preflight-java"
        pf_ref = _run_server_session_wait4(
            [*java_prefix_for_state(oracle_meta["command"], pf_j_state), "server"],
            root,
            build_isolated_env(base_env, pf_j_state),
            module_uri,
            5,
            folder / "preflight-java",
        )
        expected_bytes = (folder / "preflight-java/first-result.bin").read_bytes()
        if native_oracle_meta is not None:
            pf_ni_state = folder / "state/preflight-native-image"
            _run_server_session_wait4(
                [*native_oracle_meta["command"], "server"],
                root,
                build_isolated_env(base_env, pf_ni_state),
                module_uri,
                5,
                folder / "preflight-native-image",
                expected_result_bytes=expected_bytes,
            )
    else:
        assert native_oracle_meta is not None
        pf_ni_state = folder / "state/preflight-native-image"
        pf_ref = _run_server_session_wait4(
            [*native_oracle_meta["command"], "server"],
            root,
            build_isolated_env(base_env, pf_ni_state),
            module_uri,
            5,
            folder / "preflight-native-image",
        )
        expected_bytes = (folder / "preflight-native-image/first-result.bin").read_bytes()

    pf_r_state = folder / "state/preflight-rust"
    _run_server_session_wait4(
        [str(pkl_bin), "server"],
        root,
        build_isolated_env(base_env, pf_r_state),
        module_uri,
        5,
        folder / "preflight-rust",
        expected_result_bytes=expected_bytes,
    )

    java_runs, native_runs, rust_runs = [], [], []
    for idx in range(forks):
        if oracle_meta is not None:
            js = folder / f"state/java-{idx}"
            jr = _run_server_session_wait4(
                [*java_prefix_for_state(oracle_meta["command"], js), "server"],
                root,
                build_isolated_env(base_env, js),
                module_uri,
                steady_reqs,
                folder / "java" / str(idx),
                expected_result_bytes=expected_bytes,
            )
            java_runs.append(jr)

        if native_oracle_meta is not None:
            ns = folder / f"state/native-image-{idx}"
            nr = _run_server_session_wait4(
                [*native_oracle_meta["command"], "server"],
                root,
                build_isolated_env(base_env, ns),
                module_uri,
                steady_reqs,
                folder / "native-image" / str(idx),
                expected_result_bytes=expected_bytes,
            )
            native_runs.append(nr)

        rs = folder / f"state/rust-{idx}"
        rr = _run_server_session_wait4(
            [str(pkl_bin), "server"],
            root,
            build_isolated_env(base_env, rs),
            module_uri,
            steady_reqs,
            folder / "rust" / str(idx),
            expected_result_bytes=expected_bytes,
        )
        rust_runs.append(rr)

    r_op = distribution([r["steady_request_ns"]["median"] for r in rust_runs])
    r_rss = distribution([r["peak_rss_bytes"] for r in rust_runs])

    java_block = None
    vs_temurin = None
    if oracle_meta is not None:
        j_op = distribution([r["steady_request_ns"]["median"] for r in java_runs])
        j_rss = distribution([r["peak_rss_bytes"] for r in java_runs])
        java_block = {
            "fork_median_operation_ns": j_op,
            "fork_peak_rss_bytes": j_rss,
            "runs": java_runs,
            "memory_boundary": "Whole Java JVM pkl server process via os.wait4 ru_maxrss",
        }
        vs_temurin = evaluate_ratio_verdict(
            r_op["median"], j_op["median"], r_rss["median"], j_rss["median"], correctness_verified=True
        )

    native_block = None
    vs_native = None
    if native_oracle_meta is not None:
        n_op = distribution([r["steady_request_ns"]["median"] for r in native_runs])
        n_rss = distribution([r["peak_rss_bytes"] for r in native_runs])
        native_block = {
            "fork_median_operation_ns": n_op,
            "fork_peak_rss_bytes": n_rss,
            "runs": native_runs,
            "memory_boundary": "Whole Official Apple Pkl 0.32.0 native-image pkl server process via os.wait4 ru_maxrss",
        }
        vs_native = evaluate_ratio_verdict(
            r_op["median"],
            n_op["median"],
            r_rss["median"],
            n_rss["median"],
            correctness_verified=True,
            baseline_key_prefix="native_image",
        )

    return {
        "id": f"{wid}:steady-server",
        "workload_id": wid,
        "family": workload["family"],
        "mode": "steady-server",
        "description": workload.get("description", ""),
        "expected_error": False,
        "correctness_verified": True,
        "expected_result_sha256": pf_ref["first_result_sha256"],
        "java": java_block,
        "native_image": native_block,
        "rust": {
            "fork_median_operation_ns": r_op,
            "fork_peak_rss_bytes": r_rss,
            "runs": rust_runs,
            "memory_boundary": "Whole native Rust pkl server process via os.wait4 ru_maxrss",
        },
        "vs_official_native_image": vs_native,
        "vs_live_temurin_21": vs_temurin,
        "vs_m0_graalvm_25_0_1": None,
    }


def _run_packages_http_workload(
    workload: dict[str, Any],
    folder: Path,
    oracle_meta: dict[str, Any] | None,
    pkl_bin: Path,
    root: Path,
    cold_samples: int,
    servers: m4_differential.M4FixtureServers,
    *,
    native_oracle_meta: dict[str, Any] | None = None,
) -> dict[str, Any]:
    wid = workload["id"]
    cache_mode = workload["cache_mode"]
    tmpl = (root / workload["template"]).read_text(encoding="utf-8")
    module_path = folder / "staged_packages_http.pkl"
    module_path.parent.mkdir(parents=True, exist_ok=True)
    module_path.write_text(tmpl.replace("{http_port}", str(servers.http_port)), encoding="utf-8")
    rewrite = f"https://localhost:0/=https://localhost:{servers.https_port}/"

    def prepare_cache(state_dir: Path) -> Path:
        cache_dir = state_dir / "cache"
        cache_dir.mkdir(parents=True, exist_ok=True)
        if cache_mode == "warm":
            dest = cache_dir / "package-2/localhost(3a)0/alpha@1.0.0"
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copytree(root / M4_REGISTRY_DIR / "alpha@1.0.0", dest, dirs_exist_ok=True)
        return cache_dir

    def eval_args(state_dir: Path) -> list[str]:
        cache_dir = prepare_cache(state_dir)
        return [
            "eval",
            "--no-project",
            "--settings",
            "pkl:settings",
            "--color=never",
            "--allowed-resources",
            "prop:,env:,file:,http:,https:,package:,projectpackage:",
            "--cache-dir",
            str(cache_dir),
            "--ca-certificates",
            str(servers.ca_path),
            "--http-rewrite",
            rewrite,
            str(module_path),
        ]

    def java_cmd(state_dir: Path) -> list[str]:
        assert oracle_meta is not None
        return [*java_prefix_for_state(oracle_meta["command"], state_dir), *eval_args(state_dir)]

    def native_cmd(state_dir: Path) -> list[str]:
        assert native_oracle_meta is not None
        return [*native_oracle_meta["command"], *eval_args(state_dir)]

    def rust_cmd(state_dir: Path) -> list[str]:
        return [str(pkl_bin), *eval_args(state_dir)]

    before_lines = len(servers.request_log.read_text(encoding="utf-8").splitlines()) if servers.request_log.exists() else 0
    row = _run_cold_cli_pair(
        wid,
        workload["family"],
        folder,
        java_cmd if oracle_meta is not None else None,
        rust_cmd,
        oracle_meta,
        root,
        cold_samples,
        native_oracle_meta=native_oracle_meta,
        native_cmd_fn=native_cmd if native_oracle_meta is not None else None,
        description=workload.get("description", ""),
    )
    all_log_lines = servers.request_log.read_text(encoding="utf-8").splitlines() if servers.request_log.exists() else []
    new_lines = all_log_lines[before_lines:]
    server_hits = len(new_lines)
    http_resource_hits = 0
    https_package_hits = 0
    for line in new_lines:
        try:
            rec = json.loads(line)
        except json.JSONDecodeError:
            continue
        path_str = str(rec.get("path", ""))
        scheme_str = str(rec.get("scheme", "") or rec.get("server", "")).lower()
        if "/resources/data.txt" in path_str:
            http_resource_hits += 1
        if "alpha@1.0.0" in path_str and (path_str.endswith(".json") or path_str.endswith(".zip") or "https" in scheme_str):
            https_package_hits += 1
    if http_resource_hits <= 0:
        raise ValueError(f"{wid}: expected localhost HTTP resource hits > 0, got {http_resource_hits}")
    if cache_mode == "cold" and https_package_hits <= 0:
        raise ValueError(f"{wid}: cold package cache expected HTTPS package fetch hits > 0, got {https_package_hits}")
    if cache_mode == "warm" and https_package_hits != 0:
        raise ValueError(f"{wid}: warm package cache must NOT fetch package over HTTPS (got {https_package_hits} hits)")
    row["server_request_hits"] = server_hits
    row["http_resource_hits"] = http_resource_hits
    row["https_package_hits"] = https_package_hits
    row["cache_mode"] = cache_mode
    return row


def _run_js_binding_workload(
    workload: dict[str, Any],
    folder: Path,
    destination: Path,
    oracle_meta: dict[str, Any],
    root: Path,
    js_sampling: dict[str, Any],
    java_persistent_ref: dict[str, Any],
) -> dict[str, Any]:
    wid = workload["id"]
    target = workload["target"]  # "napi" | "wasm-node" | "wasm-browser"
    op_mode = workload["op_mode"]
    module = (root / workload["module"]).resolve()
    node_bin = shutil.which("node")
    if not node_bin:
        raise RuntimeError("node executable not found on PATH")

    # Preflight Java exact raw text (and pkl-binary when typed-conversion) BEFORE running JS!
    pf_state = folder / "state/java-preflight-text"
    j_txt = capture_wait4(
        [
            *java_prefix_for_state(oracle_meta["command"], pf_state),
            "eval",
            "--no-cache",
            "--no-project",
            "--settings",
            "pkl:settings",
            "--color=never",
            str(module),
        ],
        root,
        build_isolated_env(oracle_meta["environment"], pf_state),
        folder / "preflight-java-text",
    )
    exp_text, _ = verify_capture_raw(j_txt, expected_exit=0)
    expected_text_path = folder / "expected-text.bin"
    expected_text_path.write_bytes(exp_text)

    expected_binary_path = None
    if op_mode == "typed-conversion":
        pfb_state = folder / "state/java-preflight-binary"
        j_bin = capture_wait4(
            [
                *java_prefix_for_state(oracle_meta["command"], pfb_state),
                "eval",
                "--no-cache",
                "--no-project",
                "--settings",
                "pkl:settings",
                "--color=never",
                "-f",
                "pkl-binary",
                str(module),
            ],
            root,
            build_isolated_env(oracle_meta["environment"], pfb_state),
            folder / "preflight-java-binary",
        )
        exp_bin, _ = verify_capture_raw(j_bin, expected_exit=0)
        expected_binary_path = folder / "expected-binary.bin"
        expected_binary_path.write_bytes(exp_bin)

    driver_map = {
        "napi": root / "benchmarks/m7_fixtures/bench_napi.mjs",
        "wasm-node": root / "benchmarks/m7_fixtures/bench_wasm_node.mjs",
        "wasm-browser": root / "benchmarks/m7_fixtures/bench_wasm_browser.mjs",
    }
    driver = driver_map[target]
    plan_path = folder / "plan.json"
    result_path = folder / "js-result.json"
    plan = {
        "repo_root": str(root),
        "napi_pkg_dir": str(root / "packages/pkl-napi"),
        "wasm_pkg_dir": str(root / "packages/pkl-wasm"),
        "module_path": str(module),
        "module_uri": module.as_uri(),
        "expected_text_path": str(expected_text_path),
        "expected_binary_path": str(expected_binary_path) if expected_binary_path else None,
        "op_mode": op_mode,
        "expr": "run(0)",
        "sampling": js_sampling,
    }
    write_json(plan_path, plan)

    run_env = dict(os.environ)
    run_env.update({"LC_ALL": "C", "LANG": "C", "TZ": "UTC", "NO_COLOR": "1"})
    cap = capture_wait4(
        [node_bin, str(driver), str(plan_path), str(result_path)],
        root,
        run_env,
        folder / "js-driver",
        timeout=max(float(js_sampling["maximum_warmup_seconds"]) + 120.0, 180.0),
    )
    verify_capture_raw(cap, expected_exit=0)
    js_doc = json.loads(result_path.read_bytes())
    if not js_doc.get("preflight_raw_byte_verified"):
        raise ValueError(f"{wid}: JS driver did not confirm preflight_raw_byte_verified")
    if target == "wasm-browser" and js_doc.get("browserErrors"):
        raise ValueError(f"{wid}: Chromium reported browserErrors: {js_doc['browserErrors']}")

    op_dist = distribution([r["ns_per_op"] for r in js_doc["measurements"]])
    warmup_converged = bool(js_doc["warmup"]["converged"])

    if target == "wasm-browser":
        rust_rss_bytes = float(js_doc["chromium_peak_tree_rss_bytes"])
        if rust_rss_bytes <= 0:
            raise ValueError(f"{wid}: failed to sample Chromium process-tree RSS")
        mem_boundary = js_doc["memory_boundary"]
    else:
        rust_rss_bytes = float(cap["peak_rss_bytes"])
        mem_boundary = f"Whole Node.js host process running {target} via os.wait4 ru_maxrss"

    java_time_ns = float(java_persistent_ref["fork_median_operation_ns"]["median"])
    java_rss_bytes = float(java_persistent_ref["fork_peak_rss_bytes"]["median"])
    java_converged = java_persistent_ref.get("warmup") == "converged"

    if target == "wasm-browser":
        contract = load_contract(root)
        wasm_lin_mem = float(js_doc.get("wasm_linear_memory_bytes_peak") or 0)
        if wasm_lin_mem <= 0:
            raise ValueError(f"{wid}: missing wasm_linear_memory_bytes_peak in browser result")
        js_heap_info = js_doc.get("js_heap_after_eval") or {}
        js_heap_used = js_heap_info.get("usedJSHeapSize")
        wasm_gzip = int(js_doc.get("wasm_binary_gzip_bytes") or 0)
        if wasm_gzip <= 0:
            raise ValueError(f"{wid}: missing wasm_binary_gzip_bytes in browser result")
        workload_key = "typed-conversion" if op_mode == "typed-conversion" else "eval-text"
        vs_temurin = evaluate_browser_wasm_verdict(
            op_dist["median"],
            java_time_ns,
            rust_rss_bytes,
            java_rss_bytes,
            wasm_linear_memory_bytes=wasm_lin_mem,
            js_heap_used_bytes=float(js_heap_used) if js_heap_used is not None else None,
            wasm_binary_gzip_bytes=wasm_gzip,
            correctness_verified=True,
            converged=(warmup_converged and java_converged),
            browser_targets=contract["browser_wasm_targets"],
            browser_engine=str(js_doc.get("browser_engine") or "chromium"),
            workload_key=workload_key,
        )
    else:
        vs_temurin = evaluate_ratio_verdict(
            op_dist["median"],
            java_time_ns,
            rust_rss_bytes,
            java_rss_bytes,
            correctness_verified=True,
            converged=(warmup_converged and java_converged),
        )

    return {
        "id": f"{wid}:{target}",
        "workload_id": wid,
        "family": workload["family"],
        "mode": target,
        "op_mode": op_mode,
        "description": workload.get("description", ""),
        "expected_error": False,
        "correctness_verified": True,
        "java_reference_id": workload["java_reference_workload"],
        "java": {
            "warmup": java_persistent_ref.get("warmup"),
            "fork_median_operation_ns": java_persistent_ref["fork_median_operation_ns"],
            "fork_peak_rss_bytes": java_persistent_ref["fork_peak_rss_bytes"],
        },
        "native_image": None,
        "native_image_not_applicable_reason": NATIVE_IMAGE_API_NA_REASON,
        "rust": {
            "runtime": target,
            "warmup": "converged" if warmup_converged else "inconclusive",
            "initialize_ns": js_doc["initialize_ns"],
            "wasm_init_only_ns": js_doc.get("wasm_init_only_ns"),
            "first_request_ns": js_doc["first_request_ns"],
            "operation_ns": op_dist,
            "peak_rss_bytes": rust_rss_bytes,
            "node_driver_wait4_peak_rss_bytes": cap["peak_rss_bytes"],
            "node_memory_usage": js_doc.get("node_memory_usage"),
            "chromium_peak_tree_rss_bytes": js_doc.get("chromium_peak_tree_rss_bytes"),
            "peak_renderer_rss_bytes": js_doc.get("peak_renderer_rss_bytes"),
            "peak_role_rss_bytes": js_doc.get("peak_role_rss_bytes"),
            "wasm_linear_memory_bytes_peak": js_doc.get("wasm_linear_memory_bytes_peak"),
            "js_heap_after_eval": js_doc.get("js_heap_after_eval"),
            "wasm_binary_bytes": js_doc.get("wasm_binary_bytes"),
            "wasm_binary_gzip_bytes": js_doc.get("wasm_binary_gzip_bytes"),
            "browser_engine": js_doc.get("browser_engine"),
            "browser": js_doc.get("browser"),
            "playwright": js_doc.get("playwright"),
            "browserErrors": js_doc.get("browserErrors"),
            "cross_origin_isolated": js_doc.get("cross_origin_isolated"),
            "memory_boundary": mem_boundary,
            "js_result_rel_path": result_path.relative_to(destination).as_posix(),
            "driver_capture": cap,
        },
        "vs_official_native_image": None,
        "vs_live_temurin_21": vs_temurin,
        "vs_m0_graalvm_25_0_1": None,
    }


def _format_live_progress(row: dict[str, Any], *, extra: str = "") -> str:
    parts = [f"[m7-bench] {row['id']}:"]
    vn = row.get("vs_official_native_image")
    if vn is not None:
        parts.append(
            f"vs-Native(time={vn['time_ratio']:.3f}x rss={vn['rss_ratio']:.3f}x verdict={vn['verdict']})"
        )
    vt = row.get("vs_live_temurin_21")
    if vt is not None:
        parts.append(
            f"vs-T21(time={vt['time_ratio']:.3f}x rss={vt['rss_ratio']:.3f}x verdict={vt['verdict']})"
        )
    vg = row.get("vs_m0_graalvm_25_0_1")
    if vg is not None:
        parts.append(f"vs-M0-GraalVM25(time={vg['time_ratio']:.3f}x rss={vg['rss_ratio']:.3f}x)")
    if extra:
        parts.append(extra)
    return " ".join(parts)


def run_suite(
    root: Path = ROOT,
    output: Path | None = None,
    *,
    profile: str = "full",
    families: set[str] | None = None,
    skip_build: bool = False,
    baseline: str = "both",
) -> dict[str, Any]:
    root = root.resolve()
    contract = load_contract(root)
    if profile not in contract["profiles"]:
        raise ValueError(f"unknown profile: {profile}")
    if baseline not in VALID_BASELINES:
        raise ValueError(f"unknown baseline: {baseline!r} (expected one of {VALID_BASELINES})")
    profile_cfg = contract["profiles"][profile]
    js_sampling = contract["js_sampling"][profile]
    selected_families = set(families) if families else set(ALL_FAMILIES)
    unknown = selected_families - set(ALL_FAMILIES)
    if unknown:
        raise ValueError(f"unknown benchmark families: {sorted(unknown)}")
    use_jvm = baseline in ("both", "jvm")
    use_native = baseline in ("both", "native-image")
    if baseline == "native-image" and not (selected_families & NATIVE_IMAGE_ELIGIBLE_FAMILIES):
        raise ValueError(
            f"selected families {sorted(selected_families)} have no official native-image CLI/server/pkldoc workloads"
        )
    complete_catalog = selected_families == set(ALL_FAMILIES) and baseline == "both"

    destination = (output or (root / f"target/m7/bench-{profile}")).resolve()
    if destination.exists():
        raise ValueError(f"refusing to overwrite existing benchmark output directory: {destination}")
    destination.mkdir(parents=True, exist_ok=False)

    oracle_meta = ensure_pinned_oracle(root) if use_jvm else None
    native_oracle_meta = ensure_pinned_native_image_oracle(root) if use_native else None
    m0_ref_path = root / contract["m0_graalvm_reference"]
    m0_ref_bytes = m0_ref_path.read_bytes()
    m0_ref = json.loads(m0_ref_bytes)
    m0_cases_by_id = {c["id"]: c for c in m0_ref.get("cases", [])}

    started_machine = m0_benchmark.machine()
    build_info = build_rust_artifacts(
        root,
        destination,
        skip_build=skip_build,
        selected_families=selected_families,
        baseline=baseline,
    )
    before_sources, before_sources_sha = source_snapshot(root)
    pkl_bin = Path(build_info["artifacts"]["pkl"]["path"])
    pkldoc_bin = Path(build_info["artifacts"]["pkldoc"]["path"]) if "pkldoc" in build_info["artifacts"] else None
    probe_bin = (
        Path(build_info["artifacts"]["m7_bench_probe"]["path"])
        if "m7_bench_probe" in build_info["artifacts"]
        else None
    )

    classes_dir = None
    java_compile_info = None
    if oracle_meta is not None:
        classes_dir, java_compile_info = compile_benchmark_java(oracle_meta, destination, root)

    results: list[dict[str, Any]] = []
    java_persistent_refs: dict[str, dict[str, Any]] = {}

    # 1. Core 6 workloads
    if "core-6" in selected_families or (use_jvm and selected_families & {"napi", "wasm-node", "wasm-browser"}):
        for w in contract["core_workloads"]:
            wid = w["id"]
            if "core-6" not in selected_families and wid != "stdlib-collections":
                continue
            module = (root / w["module"]).resolve()
            m0_case = m0_cases_by_id.get(w.get("m0_graalvm_id", wid))
            w_folder = destination / "core" / wid
            w_folder.mkdir(parents=True, exist_ok=True)

            if "core-6" in selected_families:
                cold_row = _run_cold_cli_pair(
                    wid,
                    "core-6",
                    w_folder / "cold-cli",
                    (
                        lambda st, m=module: [
                            *java_prefix_for_state(oracle_meta["command"], st),  # type: ignore[index]
                            "eval",
                            "--no-cache",
                            "--no-project",
                            "--settings",
                            "pkl:settings",
                            "--color=never",
                            str(m),
                        ]
                    )
                    if oracle_meta is not None
                    else None,
                    lambda st, m=module: [
                        str(pkl_bin),
                        "eval",
                        "--no-cache",
                        "--no-project",
                        "--settings",
                        "pkl:settings",
                        "--color=never",
                        str(m),
                    ],
                    oracle_meta,
                    root,
                    profile_cfg["cold_samples"],
                    native_oracle_meta=native_oracle_meta,
                    native_cmd_fn=(
                        lambda st, m=module: [
                            *native_oracle_meta["command"],  # type: ignore[index]
                            "eval",
                            "--no-cache",
                            "--no-project",
                            "--settings",
                            "pkl:settings",
                            "--color=never",
                            str(m),
                        ]
                    )
                    if native_oracle_meta is not None
                    else None,
                    expected_error=w["expected_error"],
                    m0_ref_case=m0_case,
                    description=f"Core 6 cold CLI: {wid}",
                )
                results.append(cold_row)
                print(_format_live_progress(cold_row), flush=True)

            if oracle_meta is not None:
                assert classes_dir is not None and probe_bin is not None
                for api_mode in ("persistent", "fresh-evaluator"):
                    if "core-6" not in selected_families and api_mode != "persistent":
                        continue
                    api_row = _run_api_probe_pair(
                        wid,
                        "core-6",
                        api_mode,
                        module,
                        w_folder,
                        oracle_meta,
                        classes_dir,
                        probe_bin,
                        root,
                        profile_cfg,
                        expected_error=w["expected_error"],
                        m0_ref_case=m0_case,
                        description=f"Core 6 {api_mode}: {wid}",
                    )
                    if api_mode == "persistent":
                        java_persistent_refs[wid] = api_row["java"]
                    if "core-6" in selected_families:
                        results.append(api_row)
                        print(_format_live_progress(api_row), flush=True)

    # Ensure apples-to-apples Java persistent `Evaluator.evaluate(ModuleSource)` (`PModule`)
    # reference exists for js_boundary_payload.pkl if JS workloads selected
    if use_jvm and selected_families & {"napi", "wasm-node", "wasm-browser"}:
        assert oracle_meta is not None and classes_dir is not None
        js_mod = (root / "benchmarks/m7_fixtures/js_boundary_payload.pkl").resolve()
        js_ref_dir = destination / "js-java-typed-ref"
        js_ref_dir.mkdir(parents=True, exist_ok=True)
        java_persistent_refs["js-boundary-payload"] = _run_java_typed_persistent_ref(
            js_mod,
            js_ref_dir,
            oracle_meta,
            classes_dir,
            root,
            profile_cfg,
        )

    # Optional M4 localhost HTTP/HTTPS server for packages-http-cache
    need_servers = "packages-http-cache" in selected_families
    server_oracle_meta = (oracle_meta or ensure_pinned_oracle(root)) if need_servers else None
    server_ctx = (
        m4_differential.M4FixtureServers(
            server_oracle_meta,
            root / M4_HOST_IO_DIR,
            root / M4_REGISTRY_DIR,
            destination / "m4-http-server",
        )
        if need_servers and server_oracle_meta is not None
        else None
    )

    try:
        servers = server_ctx.__enter__() if server_ctx is not None else None
        if servers is not None:
            st = servers.self_test()
            if not st.get("passed"):
                raise RuntimeError(f"M4FixtureServers self_test failed: {st}")

        for w in contract["product_workloads"]:
            if w["family"] not in selected_families:
                continue
            wid = w["id"]
            kind = w["kind"]
            if not use_jvm and kind == "js_binding":
                continue
            w_folder = destination / "product" / wid
            w_folder.mkdir(parents=True, exist_ok=True)

            if kind == "eval_cold_and_persistent":
                module = (root / w["module"]).resolve()
                cold_row = _run_cold_cli_pair(
                    wid,
                    w["family"],
                    w_folder / "cold-cli",
                    (
                        lambda st, m=module: [
                            *java_prefix_for_state(oracle_meta["command"], st),  # type: ignore[index]
                            "eval",
                            "--no-cache",
                            "--no-project",
                            "--settings",
                            "pkl:settings",
                            "--color=never",
                            str(m),
                        ]
                    )
                    if oracle_meta is not None
                    else None,
                    lambda st, m=module: [
                        str(pkl_bin),
                        "eval",
                        "--no-cache",
                        "--no-project",
                        "--settings",
                        "pkl:settings",
                        "--color=never",
                        str(m),
                    ],
                    oracle_meta,
                    root,
                    profile_cfg["cold_samples"],
                    native_oracle_meta=native_oracle_meta,
                    native_cmd_fn=(
                        lambda st, m=module: [
                            *native_oracle_meta["command"],  # type: ignore[index]
                            "eval",
                            "--no-cache",
                            "--no-project",
                            "--settings",
                            "pkl:settings",
                            "--color=never",
                            str(m),
                        ]
                    )
                    if native_oracle_meta is not None
                    else None,
                    expected_error=w["expected_error"],
                    description=w.get("description", ""),
                )
                results.append(cold_row)
                print(_format_live_progress(cold_row), flush=True)
                if oracle_meta is not None:
                    assert classes_dir is not None and probe_bin is not None
                    pers_row = _run_api_probe_pair(
                        wid,
                        w["family"],
                        "persistent",
                        module,
                        w_folder,
                        oracle_meta,
                        classes_dir,
                        probe_bin,
                        root,
                        profile_cfg,
                        expected_error=w["expected_error"],
                        description=f"{w.get('description', '')} (persistent)",
                    )
                    results.append(pers_row)
                    print(_format_live_progress(pers_row), flush=True)

            elif kind == "cli_output_files":
                row = _run_output_files_workload(
                    w,
                    w_folder,
                    oracle_meta,
                    pkl_bin,
                    root,
                    profile_cfg["cold_samples"],
                    native_oracle_meta=native_oracle_meta,
                )
                results.append(row)
                print(_format_live_progress(row), flush=True)

            elif kind == "cli_eval_format":
                module = (root / w["module"]).resolve()
                fmt = w["output_format"]
                row = _run_cold_cli_pair(
                    wid,
                    w["family"],
                    w_folder,
                    (
                        lambda st, m=module, f=fmt: [
                            *java_prefix_for_state(oracle_meta["command"], st),  # type: ignore[index]
                            "eval",
                            "--no-cache",
                            "--no-project",
                            "--settings",
                            "pkl:settings",
                            "--color=never",
                            "-f",
                            f,
                            str(m),
                        ]
                    )
                    if oracle_meta is not None
                    else None,
                    lambda st, m=module, f=fmt: [
                        str(pkl_bin),
                        "eval",
                        "--no-cache",
                        "--no-project",
                        "--settings",
                        "pkl:settings",
                        "--color=never",
                        "-f",
                        f,
                        str(m),
                    ],
                    oracle_meta,
                    root,
                    profile_cfg["cold_samples"],
                    native_oracle_meta=native_oracle_meta,
                    native_cmd_fn=(
                        lambda st, m=module, f=fmt: [
                            *native_oracle_meta["command"],  # type: ignore[index]
                            "eval",
                            "--no-cache",
                            "--no-project",
                            "--settings",
                            "pkl:settings",
                            "--color=never",
                            "-f",
                            f,
                            str(m),
                        ]
                    )
                    if native_oracle_meta is not None
                    else None,
                    expected_error=w["expected_error"],
                    description=w.get("description", ""),
                )
                results.append(row)
                print(_format_live_progress(row), flush=True)

            elif kind == "packages_http":
                assert servers is not None
                row = _run_packages_http_workload(
                    w,
                    w_folder,
                    oracle_meta,
                    pkl_bin,
                    root,
                    profile_cfg["cold_samples"],
                    servers,
                    native_oracle_meta=native_oracle_meta,
                )
                results.append(row)
                print(_format_live_progress(row, extra=f"hits={row['server_request_hits']}"), flush=True)

            elif kind == "repl_pipe":
                sess = (root / w["session_file"]).resolve()
                row = _run_cold_cli_pair(
                    wid,
                    w["family"],
                    w_folder,
                    (
                        lambda st: [
                            *java_prefix_for_state(oracle_meta["command"], st),  # type: ignore[index]
                            "repl",
                            "--no-cache",
                            "--no-project",
                            "--color=never",
                        ]
                    )
                    if oracle_meta is not None
                    else None,
                    lambda st: [str(pkl_bin), "repl", "--no-cache", "--no-project", "--color=never"],
                    oracle_meta,
                    root,
                    profile_cfg["cold_samples"],
                    native_oracle_meta=native_oracle_meta,
                    native_cmd_fn=(
                        lambda st: [
                            *native_oracle_meta["command"],  # type: ignore[index]
                            "repl",
                            "--no-cache",
                            "--no-project",
                            "--color=never",
                        ]
                    )
                    if native_oracle_meta is not None
                    else None,
                    allow_native_jline_stderr=True,
                    stdin_file=sess,
                    description=w.get("description", ""),
                )
                results.append(row)
                print(_format_live_progress(row), flush=True)

            elif kind == "server_msgpack":
                row = _run_server_workload(
                    w,
                    w_folder,
                    oracle_meta,
                    pkl_bin,
                    root,
                    profile_cfg,
                    native_oracle_meta=native_oracle_meta,
                )
                results.append(row)
                print(_format_live_progress(row), flush=True)

            elif kind == "formatter_stdin":
                inp = (root / w["input_file"]).resolve()
                row = _run_cold_cli_pair(
                    wid,
                    w["family"],
                    w_folder,
                    (lambda st: [*java_prefix_for_state(oracle_meta["command"], st), "format", "-"])  # type: ignore[index]
                    if oracle_meta is not None
                    else None,
                    lambda st: [str(pkl_bin), "format", "-"],
                    oracle_meta,
                    root,
                    profile_cfg["cold_samples"],
                    native_oracle_meta=native_oracle_meta,
                    native_cmd_fn=(lambda st: [*native_oracle_meta["command"], "format", "-"])  # type: ignore[index]
                    if native_oracle_meta is not None
                    else None,
                    expected_exit_override=int(w.get("expected_exit", 11)),
                    stdin_file=inp,
                    description=w.get("description", ""),
                )
                results.append(row)
                print(_format_live_progress(row), flush=True)

            elif kind == "pkldoc_site":
                assert pkldoc_bin is not None
                row = _run_pkldoc_workload(
                    w,
                    w_folder,
                    oracle_meta,
                    pkldoc_bin,
                    root,
                    profile_cfg["cold_samples"],
                    native_oracle_meta=native_oracle_meta,
                )
                results.append(row)
                print(_format_live_progress(row), flush=True)

            elif kind == "js_binding":
                assert oracle_meta is not None
                ref_id = w["java_reference_workload"]
                row = _run_js_binding_workload(
                    w, w_folder, destination, oracle_meta, root, js_sampling, java_persistent_refs[ref_id]
                )
                results.append(row)
                print(_format_live_progress(row), flush=True)

            else:
                raise ValueError(f"unknown product workload kind: {kind}")
    finally:
        if server_ctx is not None:
            server_ctx.__exit__(None, None, None)

    after_sources, after_sources_sha = source_snapshot(root)
    source_stability = before_sources == after_sources
    write_json(
        destination / "sources.snapshot.json",
        {
            "before_sha256": before_sources_sha,
            "after_sha256": after_sources_sha,
            "source_stability": source_stability,
            "before": before_sources,
            "after": after_sources,
        },
    )

    temurin_rows = [r for r in results if r.get("vs_live_temurin_21") is not None]
    verdict_counts = dict(sorted(Counter(r["vs_live_temurin_21"]["verdict"] for r in temurin_rows).items()))
    native_rows = [r for r in results if r.get("vs_official_native_image") is not None]
    native_verdict_counts = dict(sorted(Counter(r["vs_official_native_image"]["verdict"] for r in native_rows).items()))
    graal_rows = [r for r in results if r.get("vs_m0_graalvm_25_0_1") is not None]
    graal_verdict_counts = dict(sorted(Counter(r["vs_m0_graalvm_25_0_1"]["verdict"] for r in graal_rows).items()))
    failing_native = [
        {"id": r["id"], "reason": r["vs_official_native_image"]["reason"]}
        for r in native_rows
        if r["vs_official_native_image"]["verdict"] != "pass"
    ]
    failing_temurin = [
        {"id": r["id"], "reason": r["vs_live_temurin_21"]["reason"]}
        for r in temurin_rows
        if r["vs_live_temurin_21"]["verdict"] != "pass"
    ]
    failing_graalvm = [
        {"id": r["id"], "reason": r["vs_m0_graalvm_25_0_1"]["reason"]}
        for r in graal_rows
        if r["vs_m0_graalvm_25_0_1"]["verdict"] != "pass"
    ]

    report: dict[str, Any] = {
        "schema_version": 1,
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "baseline": contract["baseline"],
        "baseline_mode": baseline,
        "profile": profile,
        "strict_eligible": bool(profile_cfg["strict_eligible"]) and baseline == "both",
        "complete_catalog": complete_catalog,
        "selected_families": sorted(selected_families),
        "target_ratios": contract["target_ratios"],
        "native_image_targets": contract["native_image_targets"],
        "browser_wasm_targets": contract["browser_wasm_targets"],
        "sampling": profile_cfg,
        "js_sampling": js_sampling,
        "contract_sha256": file_digest(root / WORKLOADS_PATH),
        "oracle": oracle_meta,
        "native_image_oracle": native_oracle_meta,
        "m0_graalvm_reference": {
            "path": contract["m0_graalvm_reference"],
            "sha256": digest(m0_ref_bytes),
            "jdk_properties": m0_ref["baseline"]["jdk_properties"],
            "recursive_optimization": m0_ref.get("recursive_optimization"),
            "provenance_note": (
                "Archived optimizing GraalVM CE 25.0.1 baseline from M0 (reports/m0/benchmarks.json), "
                "strictly separated from live Official Apple Pkl 0.32.0 native-image and "
                "Eclipse Adoptium Temurin 21.0.13.1+2-LTS head-to-head runs."
            ),
        },
        "machine_start": started_machine,
        "machine_end": m0_benchmark.machine(),
        "build": build_info,
        "java_harness": java_compile_info,
        "sources_snapshot_sha256": before_sources_sha,
        "sources_after_sha256": after_sources_sha,
        "source_file_count": len(before_sources),
        "source_stability": source_stability,
        "counts": {
            "total_workload_modes": len(results),
            "correctness_verified": sum(1 for r in results if r.get("correctness_verified")),
            "vs_official_native_image_compared": len(native_rows),
            "vs_official_native_image_verdicts": native_verdict_counts,
            "vs_live_temurin_21_compared": len(temurin_rows),
            "vs_live_temurin_21_verdicts": verdict_counts,
            "vs_m0_graalvm_25_0_1_compared": len(graal_rows),
            "vs_m0_graalvm_25_0_1_verdicts": graal_verdict_counts,
        },
        "non_passing_official_native_image": failing_native,
        "non_passing_live_temurin_21": failing_temurin,
        "non_passing_m0_graalvm_25_0_1": failing_graalvm,
        "results": results,
    }
    report["evidence_files"] = seal_evidence_files(destination)
    write_json(destination / "report.json", report)
    verify_report(destination, root=root, strict=False)
    return report


def verify_report(
    directory: Path,
    *,
    root: Path = ROOT,
    strict: bool = False,
) -> dict[str, Any]:
    directory = directory.resolve()
    report_path = directory / "report.json"
    if not report_path.is_file():
        raise ValueError(f"missing report.json in {directory}")
    report = json.loads(report_path.read_bytes())
    if report.get("schema_version") != 1 or report.get("baseline") != "0.32.0":
        raise ValueError("unknown M7 benchmark report schema_version or baseline")

    # 1. Verify sealed evidence integrity
    expected_files = report.get("evidence_files")
    if not isinstance(expected_files, list) or not expected_files:
        raise ValueError("missing or empty evidence_files manifest")
    actual_files = seal_evidence_files(directory)
    if actual_files != expected_files:
        exp_map = {e["path"]: e["sha256"] for e in expected_files}
        act_map = {e["path"]: e["sha256"] for e in actual_files}
        missing = sorted(set(exp_map) - set(act_map))
        extra = sorted(set(act_map) - set(exp_map))
        changed = sorted(p for p in set(exp_map) & set(act_map) if exp_map[p] != act_map[p])
        raise ValueError(
            f"evidence_files integrity mismatch: missing={missing[:3]} extra={extra[:3]} changed={changed[:3]}"
        )

    # 2. Verify oracle provenance (JVM & pinned Official Native Image), thresholds, snapshot, and sampling
    baseline_mode = report.get("baseline_mode")
    if baseline_mode is None:
        baseline_mode = "both" if report.get("native_image_oracle") is not None else "jvm"
    if baseline_mode not in VALID_BASELINES:
        raise ValueError(f"unknown report baseline_mode: {baseline_mode!r}")
    expect_jvm = baseline_mode in ("both", "jvm")
    expect_native = baseline_mode in ("both", "native-image")

    contract = load_contract(root)
    native_lock = validate_native_image_lock(root)

    if expect_jvm:
        oracle_meta = report.get("oracle")
        if not isinstance(oracle_meta, dict) or not oracle_meta.get("exact_jdk_lock_match", False):
            raise ValueError("report oracle does not have exact_jdk_lock_match == true")
        if oracle_meta.get("version") != "0.32.0":
            raise ValueError("report oracle version != 0.32.0")

    if expect_native or report.get("native_image_oracle") is not None:
        ni_oracle = report.get("native_image_oracle")
        if not isinstance(ni_oracle, dict) or not ni_oracle.get("exact_native_image_lock_match", False):
            raise ValueError("report native_image_oracle does not have exact_native_image_lock_match == true")
        if ni_oracle.get("version") != "0.32.0":
            raise ValueError("report native_image_oracle version != 0.32.0")
        ni_target = ni_oracle.get("target")
        dists = native_lock["distributions"]
        if ni_target not in dists:
            raise ValueError(f"report native_image_oracle target {ni_target!r} is not pinned in oracle.lock.json")
        locked_dist = dists[ni_target]
        for bin_label in ("pkl", "pkldoc"):
            rec = ni_oracle.get(bin_label)
            locked_spec = locked_dist[bin_label]
            if (
                not isinstance(rec, dict)
                or rec.get("sha256") != locked_spec["sha256"]
                or rec.get("bytes") != locked_spec["bytes"]
            ):
                raise ValueError(
                    f"report native_image_oracle.{bin_label} SHA-256/bytes disagrees with pinned oracle.lock.json"
                )

    targets = report.get("target_ratios", {})
    if (
        isinstance(targets.get("execution_time_max"), bool)
        or isinstance(targets.get("peak_rss_max"), bool)
        or targets.get("execution_time_max") != 1.0
        or targets.get("peak_rss_max") != 1.0
    ):
        raise ValueError("report target_ratios weakened or missing (must be <= 1.0)")

    report_ni_targets = report.get("native_image_targets")
    if expect_native or report_ni_targets is not None:
        if report_ni_targets != contract["native_image_targets"]:
            raise ValueError("report native_image_targets weakened, missing, or differs from contract")

    sealed_map = {e["path"]: e["sha256"] for e in expected_files}
    snap_path = directory / "sources.snapshot.json"
    if not snap_path.is_file() or "sources.snapshot.json" not in sealed_map:
        raise ValueError("missing required sealed sources.snapshot.json")
    snap_doc = json.loads(snap_path.read_bytes())
    before_map = snap_doc.get("before")
    after_map = snap_doc.get("after")
    if not isinstance(before_map, dict) or not isinstance(after_map, dict):
        raise ValueError("sources.snapshot.json missing before/after maps")
    recomp_before_sha = digest(json.dumps(before_map, sort_keys=True, separators=(",", ":")).encode("utf-8"))
    recomp_after_sha = digest(json.dumps(after_map, sort_keys=True, separators=(",", ":")).encode("utf-8"))
    if (
        snap_doc.get("before_sha256") != recomp_before_sha
        or report.get("sources_snapshot_sha256") != recomp_before_sha
    ):
        raise ValueError("sources_snapshot_sha256 disagrees with recomputed sources.snapshot.json before map")
    if (
        snap_doc.get("after_sha256") != recomp_after_sha
        or report.get("sources_after_sha256") != recomp_after_sha
    ):
        raise ValueError("sources_after_sha256 disagrees with recomputed sources.snapshot.json after map")
    if (before_map == after_map) != report.get("source_stability"):
        raise ValueError("recomputed source_stability disagrees with sources.snapshot.json")

    # Cross-check built JS/Wasm artifact SHA-256s against sources.snapshot.json
    build_artifacts = (report.get("build") or {}).get("artifacts") or {}
    for art_key in ("napi_addon", "wasm_bg"):
        art = build_artifacts.get(art_key)
        if isinstance(art, dict) and art.get("rel_path") in after_map:
            if after_map[art["rel_path"]] != art.get("sha256"):
                raise ValueError(f"build artifact {art_key} sha256 disagrees with sources.snapshot.json")

    report_browser_targets = report.get("browser_wasm_targets")
    if report_browser_targets is not None and report_browser_targets != contract["browser_wasm_targets"]:
        raise ValueError("report browser_wasm_targets weakened or differs from contract")
    prof = report.get("profile")
    if prof not in contract["profiles"]:
        raise ValueError(f"unknown report profile: {prof}")
    sampling = report.get("sampling")
    if sampling != contract["profiles"][prof]:
        raise ValueError(f"report sampling missing or differs from contract profile {prof}")
    js_sampling = report.get("js_sampling")
    if js_sampling != contract["js_sampling"][prof]:
        raise ValueError(f"report js_sampling missing or differs from contract js_sampling {prof}")

    # 3. Verify M0 GraalVM reference digest matches repository copy
    m0_ref = report.get("m0_graalvm_reference", {})
    m0_path = root / m0_ref.get("path", "")
    if not m0_path.is_file() or file_digest(m0_path) != m0_ref.get("sha256"):
        raise ValueError("M0 GraalVM reference SHA-256 mismatch")
    m0_doc = json.loads(m0_path.read_bytes())
    m0_cases_by_id = {c["id"]: c for c in m0_doc.get("cases", [])}

    # 4. Revalidate every workload row's raw distributions, ratio math, verdicts, and cross-checks
    results = report.get("results", [])
    if not results:
        raise ValueError("report has no workload results")
    seen_ids: set[str] = set()
    temurin_rows: list[dict[str, Any]] = []
    native_rows: list[dict[str, Any]] = []
    graal_rows: list[dict[str, Any]] = []
    for row in results:
        rid = row["id"]
        if rid in seen_ids:
            raise ValueError(f"duplicate result id: {rid}")
        seen_ids.add(rid)
        mode = row.get("mode")
        vs_t = row.get("vs_live_temurin_21")
        vs_ni = row.get("vs_official_native_image")

        if expect_jvm and not isinstance(vs_t, dict):
            raise ValueError(f"{rid}: missing vs_live_temurin_21")
        if expect_native and mode in NATIVE_IMAGE_ELIGIBLE_MODES and not isinstance(vs_ni, dict):
            raise ValueError(f"{rid}: missing required vs_official_native_image for {mode}")
        if mode not in NATIVE_IMAGE_ELIGIBLE_MODES and vs_ni is not None:
            raise ValueError(f"{rid}: non-CLI mode {mode!r} must not carry vs_official_native_image")

        if mode == "cold-cli":
            sides = ["rust"]
            if vs_t is not None:
                sides.append("java")
            if vs_ni is not None:
                sides.append("native_image")
            for side in sides:
                samps = (row.get(side) or {}).get("samples")
                if not isinstance(samps, list) or len(samps) != sampling["cold_samples"]:
                    raise ValueError(f"{rid}: {side} cold-cli sample count != {sampling['cold_samples']}")
                if distribution([s["elapsed_ns"] for s in samps]) != row[side]["elapsed_ns"]:
                    raise ValueError(f"{rid}: {side} cold-cli elapsed_ns disagrees with raw samples")
                if distribution([s["peak_rss_bytes"] for s in samps]) != row[side]["peak_rss_bytes"]:
                    raise ValueError(f"{rid}: {side} cold-cli peak_rss_bytes disagrees with raw samples")
            rust_med_time = float(row["rust"]["elapsed_ns"]["median"])
            rust_med_rss = float(row["rust"]["peak_rss_bytes"]["median"])
            if vs_t is not None:
                if (
                    not math.isclose(vs_t["rust_time_ns"], rust_med_time, rel_tol=1e-9)
                    or not math.isclose(vs_t["java_time_ns"], row["java"]["elapsed_ns"]["median"], rel_tol=1e-9)
                    or not math.isclose(vs_t["rust_peak_rss_bytes"], rust_med_rss, rel_tol=1e-9)
                    or not math.isclose(vs_t["java_peak_rss_bytes"], row["java"]["peak_rss_bytes"]["median"], rel_tol=1e-9)
                ):
                    raise ValueError(f"{rid}: vs_live_temurin_21 medians disagree with raw distributions")
            if vs_ni is not None:
                if (
                    not math.isclose(vs_ni["rust_time_ns"], rust_med_time, rel_tol=1e-9)
                    or not math.isclose(
                        vs_ni["native_image_time_ns"], row["native_image"]["elapsed_ns"]["median"], rel_tol=1e-9
                    )
                    or not math.isclose(vs_ni["rust_peak_rss_bytes"], rust_med_rss, rel_tol=1e-9)
                    or not math.isclose(
                        vs_ni["native_image_peak_rss_bytes"],
                        row["native_image"]["peak_rss_bytes"]["median"],
                        rel_tol=1e-9,
                    )
                ):
                    raise ValueError(f"{rid}: vs_official_native_image medians disagree with raw distributions")
        elif mode in ("persistent", "fresh-evaluator"):
            for side in ("java", "rust"):
                forks = (row.get(side) or {}).get("forks")
                if not isinstance(forks, list) or len(forks) != sampling["independent_api_forks"]:
                    raise ValueError(f"{rid}: {side} {mode} fork count != {sampling['independent_api_forks']}")
                if distribution([f["operation_ns"]["median"] for f in forks]) != row[side]["fork_median_operation_ns"]:
                    raise ValueError(f"{rid}: {side} {mode} fork_median_operation_ns disagrees with raw forks")
                if distribution([f["peak_rss_bytes"] for f in forks]) != row[side]["fork_peak_rss_bytes"]:
                    raise ValueError(f"{rid}: {side} {mode} fork_peak_rss_bytes disagrees with raw forks")
                exp_w = "converged" if all(f["warmup"] == "converged" for f in forks) else "inconclusive"
                if row[side].get("warmup") != exp_w:
                    raise ValueError(f"{rid}: {side} {mode} warmup disagrees with forks")
            rust_med_time = float(row["rust"]["fork_median_operation_ns"]["median"])
            rust_med_rss = float(row["rust"]["fork_peak_rss_bytes"]["median"])
            if (
                vs_t is None
                or not math.isclose(vs_t["rust_time_ns"], rust_med_time, rel_tol=1e-9)
                or not math.isclose(vs_t["java_time_ns"], row["java"]["fork_median_operation_ns"]["median"], rel_tol=1e-9)
                or not math.isclose(vs_t["rust_peak_rss_bytes"], rust_med_rss, rel_tol=1e-9)
                or not math.isclose(vs_t["java_peak_rss_bytes"], row["java"]["fork_peak_rss_bytes"]["median"], rel_tol=1e-9)
            ):
                raise ValueError(f"{rid}: vs_live_temurin_21 medians disagree with raw fork distributions")
        elif mode == "steady-server":
            sides = ["rust"]
            if vs_t is not None:
                sides.append("java")
            if vs_ni is not None:
                sides.append("native_image")
            for side in sides:
                runs = (row.get(side) or {}).get("runs")
                if not isinstance(runs, list) or len(runs) != sampling["independent_api_forks"]:
                    raise ValueError(f"{rid}: {side} steady-server run count != {sampling['independent_api_forks']}")
                if distribution([r["steady_request_ns"]["median"] for r in runs]) != row[side]["fork_median_operation_ns"]:
                    raise ValueError(f"{rid}: {side} steady-server fork_median_operation_ns disagrees with runs")
                if distribution([r["peak_rss_bytes"] for r in runs]) != row[side]["fork_peak_rss_bytes"]:
                    raise ValueError(f"{rid}: {side} steady-server fork_peak_rss_bytes disagrees with runs")
            rust_med_time = float(row["rust"]["fork_median_operation_ns"]["median"])
            rust_med_rss = float(row["rust"]["fork_peak_rss_bytes"]["median"])
            if vs_t is not None:
                if (
                    not math.isclose(vs_t["rust_time_ns"], rust_med_time, rel_tol=1e-9)
                    or not math.isclose(vs_t["java_time_ns"], row["java"]["fork_median_operation_ns"]["median"], rel_tol=1e-9)
                    or not math.isclose(vs_t["rust_peak_rss_bytes"], rust_med_rss, rel_tol=1e-9)
                    or not math.isclose(vs_t["java_peak_rss_bytes"], row["java"]["fork_peak_rss_bytes"]["median"], rel_tol=1e-9)
                ):
                    raise ValueError(f"{rid}: steady-server vs_live_temurin_21 medians disagree with runs")
            if vs_ni is not None:
                if (
                    not math.isclose(vs_ni["rust_time_ns"], rust_med_time, rel_tol=1e-9)
                    or not math.isclose(
                        vs_ni["native_image_time_ns"],
                        row["native_image"]["fork_median_operation_ns"]["median"],
                        rel_tol=1e-9,
                    )
                    or not math.isclose(vs_ni["rust_peak_rss_bytes"], rust_med_rss, rel_tol=1e-9)
                    or not math.isclose(
                        vs_ni["native_image_peak_rss_bytes"],
                        row["native_image"]["fork_peak_rss_bytes"]["median"],
                        rel_tol=1e-9,
                    )
                ):
                    raise ValueError(f"{rid}: steady-server vs_official_native_image medians disagree with runs")
        elif mode in ("napi", "wasm-node", "wasm-browser"):
            r_side = row.get("rust") or {}
            js_rel = r_side.get("js_result_rel_path")
            if not js_rel or js_rel not in sealed_map:
                raise ValueError(f"{rid}: missing sealed js_result_rel_path ({js_rel!r})")
            js_doc = json.loads((directory / js_rel).read_bytes())
            if not js_doc.get("preflight_raw_byte_verified"):
                raise ValueError(f"{rid}: sealed js-result.json lacks preflight_raw_byte_verified=true")
            meas = js_doc.get("measurements") or []
            if len(meas) != js_sampling["samples"]:
                raise ValueError(f"{rid}: JS measurement window count {len(meas)} != {js_sampling['samples']}")
            if distribution([m["ns_per_op"] for m in meas]) != r_side.get("operation_ns"):
                raise ValueError(f"{rid}: rust.operation_ns disagrees with sealed js-result.json")
            exp_js_w = "converged" if js_doc.get("warmup", {}).get("converged") else "inconclusive"
            if r_side.get("warmup") != exp_js_w:
                raise ValueError(f"{rid}: rust.warmup disagrees with sealed js-result.json")
            dcap = r_side.get("driver_capture") or {}
            if dcap.get("status") != "executed" or dcap.get("exit_code") != 0 or (dcap.get("peak_rss_bytes") or 0) <= 0:
                raise ValueError(f"{rid}: invalid JS driver_capture")
            if mode == "wasm-browser":
                if js_doc.get("browserErrors") or r_side.get("browserErrors"):
                    raise ValueError(f"{rid}: non-empty Chromium browserErrors")
                exp_rss = float(js_doc.get("chromium_peak_tree_rss_bytes") or 0)
                if exp_rss <= 0 or not math.isclose(float(r_side.get("peak_rss_bytes") or 0), exp_rss, rel_tol=1e-9):
                    raise ValueError(f"{rid}: rust.peak_rss_bytes disagrees with sealed chromium_peak_tree_rss_bytes")
            else:
                exp_rss = float(dcap["peak_rss_bytes"])
                if not math.isclose(float(r_side.get("peak_rss_bytes") or 0), exp_rss, rel_tol=1e-9):
                    raise ValueError(f"{rid}: rust.peak_rss_bytes disagrees with driver_capture.peak_rss_bytes")
            rust_med_time = float(r_side["operation_ns"]["median"])
            rust_med_rss = exp_rss
            if (
                vs_t is None
                or not math.isclose(vs_t["rust_time_ns"], rust_med_time, rel_tol=1e-9)
                or not math.isclose(vs_t["rust_peak_rss_bytes"], rust_med_rss, rel_tol=1e-9)
                or not math.isclose(vs_t["java_time_ns"], row["java"]["fork_median_operation_ns"]["median"], rel_tol=1e-9)
                or not math.isclose(vs_t["java_peak_rss_bytes"], row["java"]["fork_peak_rss_bytes"]["median"], rel_tol=1e-9)
            ):
                raise ValueError(f"{rid}: JS vs_live_temurin_21 medians disagree with sealed captures")
        else:
            raise ValueError(f"{rid}: unknown mode {mode!r}")

        if vs_t is not None:
            temurin_rows.append(row)
            if mode == "wasm-browser" and report_browser_targets is not None:
                js_doc_b = json.loads((directory / row["rust"]["js_result_rel_path"]).read_bytes())
                exp_lin = float(js_doc_b.get("wasm_linear_memory_bytes_peak") or 0)
                exp_gzip = int(js_doc_b.get("wasm_binary_gzip_bytes") or 0)
                js_heap_raw = (js_doc_b.get("js_heap_after_eval") or {}).get("usedJSHeapSize")
                exp_js_heap = float(js_heap_raw) if js_heap_raw is not None else None
                if (
                    exp_lin <= 0
                    or not math.isclose(float(vs_t.get("wasm_linear_memory_bytes") or 0), exp_lin, rel_tol=1e-9)
                    or exp_gzip <= 0
                    or int(vs_t.get("wasm_binary_gzip_bytes") or 0) != exp_gzip
                ):
                    raise ValueError(f"{rid}: browser Wasm memory metrics disagree with sealed js-result.json")
                wkey = "typed-conversion" if row.get("op_mode") == "typed-conversion" else "eval-text"
                recomputed = evaluate_browser_wasm_verdict(
                    vs_t["rust_time_ns"],
                    vs_t["java_time_ns"],
                    vs_t["rust_peak_rss_bytes"],
                    vs_t["java_peak_rss_bytes"],
                    wasm_linear_memory_bytes=exp_lin,
                    js_heap_used_bytes=exp_js_heap,
                    wasm_binary_gzip_bytes=exp_gzip,
                    correctness_verified=bool(row.get("correctness_verified")),
                    converged=bool(vs_t.get("converged")),
                    browser_targets=report_browser_targets,
                    browser_engine=str(js_doc_b.get("browser_engine") or "chromium"),
                    workload_key=wkey,
                )
                if (
                    not math.isclose(recomputed["time_ratio"], vs_t["time_ratio"], rel_tol=1e-9)
                    or not math.isclose(recomputed["rss_ratio"], vs_t["rss_ratio"], rel_tol=1e-9)
                    or not math.isclose(
                        recomputed["wasm_linear_memory_ratio"], vs_t["wasm_linear_memory_ratio"], rel_tol=1e-9
                    )
                    or recomputed["verdict"] != vs_t["verdict"]
                ):
                    raise ValueError(f"{rid}: tampered ratio or verdict in vs_live_temurin_21")
            else:
                recomputed = evaluate_ratio_verdict(
                    vs_t["rust_time_ns"],
                    vs_t["java_time_ns"],
                    vs_t["rust_peak_rss_bytes"],
                    vs_t["java_peak_rss_bytes"],
                    correctness_verified=bool(row.get("correctness_verified")),
                    converged=bool(vs_t.get("converged")),
                )
                if (
                    not math.isclose(recomputed["time_ratio"], vs_t["time_ratio"], rel_tol=1e-9)
                    or not math.isclose(recomputed["rss_ratio"], vs_t["rss_ratio"], rel_tol=1e-9)
                    or recomputed["verdict"] != vs_t["verdict"]
                ):
                    raise ValueError(f"{rid}: tampered ratio or verdict in vs_live_temurin_21")

        if vs_ni is not None:
            native_rows.append(row)
            re_ni = evaluate_ratio_verdict(
                vs_ni["rust_time_ns"],
                vs_ni["native_image_time_ns"],
                vs_ni["rust_peak_rss_bytes"],
                vs_ni["native_image_peak_rss_bytes"],
                correctness_verified=bool(row.get("correctness_verified")),
                converged=bool(vs_ni.get("converged")),
                baseline_key_prefix="native_image",
            )
            if (
                not math.isclose(re_ni["time_ratio"], vs_ni["time_ratio"], rel_tol=1e-9)
                or not math.isclose(re_ni["rss_ratio"], vs_ni["rss_ratio"], rel_tol=1e-9)
                or re_ni["verdict"] != vs_ni["verdict"]
            ):
                raise ValueError(f"{rid}: tampered ratio or verdict in vs_official_native_image")

        vs_g = row.get("vs_m0_graalvm_25_0_1")
        if vs_g is not None:
            graal_rows.append(row)
            if (
                not math.isclose(vs_g["rust_time_ns"], rust_med_time, rel_tol=1e-9)
                or not math.isclose(vs_g["rust_peak_rss_bytes"], rust_med_rss, rel_tol=1e-9)
            ):
                raise ValueError(f"{rid}: vs_m0_graalvm_25_0_1 rust metrics disagree with raw rust distributions")
            m0_case = m0_cases_by_id.get(row.get("workload_id"))
            if not m0_case:
                raise ValueError(f"{rid}: unknown M0 GraalVM reference workload_id {row.get('workload_id')!r}")
            if mode == "cold-cli":
                exp_m0_t = float(m0_case["cold_cli"]["elapsed_ns"]["median"])
                exp_m0_r = float(m0_case["cold_cli"]["peak_rss_bytes"]["median"])
            else:
                exp_m0_t = float(m0_case["api"][mode]["fork_median_operation_ns"]["median"])
                exp_m0_r = float(m0_case["api"][mode]["fork_peak_rss_bytes"]["median"])
            if (
                not math.isclose(vs_g["java_time_ns"], exp_m0_t, rel_tol=1e-9)
                or not math.isclose(vs_g["java_peak_rss_bytes"], exp_m0_r, rel_tol=1e-9)
            ):
                raise ValueError(f"{rid}: vs_m0_graalvm_25_0_1 Java baseline disagrees with reports/m0/benchmarks.json")
            re_g = evaluate_ratio_verdict(
                vs_g["rust_time_ns"],
                vs_g["java_time_ns"],
                vs_g["rust_peak_rss_bytes"],
                vs_g["java_peak_rss_bytes"],
                correctness_verified=bool(row.get("correctness_verified")),
                converged=bool(vs_g.get("converged")),
            )
            if (
                not math.isclose(re_g["time_ratio"], vs_g["time_ratio"], rel_tol=1e-9)
                or not math.isclose(re_g["rss_ratio"], vs_g["rss_ratio"], rel_tol=1e-9)
                or re_g["verdict"] != vs_g["verdict"]
            ):
                raise ValueError(f"{rid}: tampered ratio or verdict in vs_m0_graalvm_25_0_1")

    counts = report.get("counts", {})
    recounted = dict(sorted(Counter(r["vs_live_temurin_21"]["verdict"] for r in temurin_rows).items()))
    if recounted != counts.get("vs_live_temurin_21_verdicts"):
        raise ValueError("counts.vs_live_temurin_21_verdicts disagrees with results")
    if "vs_live_temurin_21_compared" in counts and len(temurin_rows) != counts.get("vs_live_temurin_21_compared"):
        raise ValueError("counts.vs_live_temurin_21_compared disagrees with results")
    recounted_ni = dict(sorted(Counter(r["vs_official_native_image"]["verdict"] for r in native_rows).items()))
    if expect_native or "vs_official_native_image_compared" in counts or "vs_official_native_image_verdicts" in counts:
        if (
            len(native_rows) != counts.get("vs_official_native_image_compared")
            or recounted_ni != counts.get("vs_official_native_image_verdicts")
        ):
            raise ValueError("counts.vs_official_native_image_* disagrees with results")
    recounted_g = dict(sorted(Counter(r["vs_m0_graalvm_25_0_1"]["verdict"] for r in graal_rows).items()))
    if (
        len(graal_rows) != counts.get("vs_m0_graalvm_25_0_1_compared")
        or recounted_g != counts.get("vs_m0_graalvm_25_0_1_verdicts")
    ):
        raise ValueError("counts.vs_m0_graalvm_25_0_1_* disagrees with results")

    if strict:
        failures: list[str] = []
        if baseline_mode != "both":
            failures.append(f"baseline_mode={baseline_mode!r} != 'both' (strict gate requires both native-image and JVM)")
        if not report.get("strict_eligible") or report.get("profile") != "full":
            failures.append(f"profile={report.get('profile')!r} is not strict_eligible")
        if not report.get("complete_catalog"):
            failures.append(f"incomplete family catalog: {report.get('selected_families')}")
        if not isinstance(report.get("native_image_oracle"), dict) or not report["native_image_oracle"].get(
            "exact_native_image_lock_match"
        ):
            failures.append("missing pinned native_image_oracle with exact_native_image_lock_match == true")
        if report.get("native_image_targets") != contract["native_image_targets"]:
            failures.append("missing or mismatched native_image_targets contract")
        if report.get("build", {}).get("source_binding") != "recorded-build":
            failures.append(f"source_binding={report.get('build', {}).get('source_binding')!r} != 'recorded-build'")
        for art_name in ("pkl", "pkldoc", "m7_bench_probe", "napi_addon", "wasm_bg"):
            art = build_artifacts.get(art_name)
            if not isinstance(art, dict) or art.get("source_binding") != "recorded-build":
                failures.append(f"artifact {art_name} source_binding != 'recorded-build'")
        if not report.get("source_stability"):
            failures.append("source_stability is false")
        expected_ids, expected_core_ids, expected_native_ids = expected_workload_sets(contract)
        missing_ids = sorted(expected_ids - seen_ids)
        if missing_ids:
            failures.append(f"missing required workloads: {missing_ids}")
        for r in results:
            if not r.get("correctness_verified"):
                failures.append(f"{r['id']}: correctness_verified=false")
            if r["id"] in expected_native_ids:
                vni = r.get("vs_official_native_image")
                if not isinstance(vni, dict):
                    failures.append(f"{r['id']}: missing required Official Apple Pkl 0.32.0 native-image comparison")
                elif vni["verdict"] != "pass":
                    failures.append(
                        f"{r['id']}: vs_official_native_image={vni['verdict']} "
                        f"(time={vni['time_ratio']:.3f}x, rss={vni['rss_ratio']:.3f}x; {vni['reason']})"
                    )
            if r["id"] in expected_core_ids and r.get("vs_m0_graalvm_25_0_1") is None:
                failures.append(f"{r['id']}: missing required M0 GraalVM 25.0.1 cross-comparison")
            vt = r.get("vs_live_temurin_21")
            if not isinstance(vt, dict):
                failures.append(f"{r['id']}: missing required vs_live_temurin_21 comparison")
            elif vt["verdict"] != "pass":
                failures.append(
                    f"{r['id']}: vs_live_temurin_21={vt['verdict']} "
                    f"(time={vt['time_ratio']:.3f}x, rss={vt['rss_ratio']:.3f}x; {vt['reason']})"
                )
        if failures:
            raise ValueError("strict M7 benchmark gate failed:\n  - " + "\n  - ".join(failures))

    return report


def format_markdown_summary(report: dict[str, Any]) -> str:
    def fmt_ns(ns: float) -> str:
        if ns >= 1e9:
            return f"{ns / 1e9:.3f} s"
        if ns >= 1e6:
            return f"{ns / 1e6:.2f} ms"
        if ns >= 1e3:
            return f"{ns / 1e3:.2f} us"
        return f"{ns:.0f} ns"

    def fmt_mb(b: float) -> str:
        return f"{b / (1024 * 1024):.1f} MiB"

    oracle_meta = report.get("oracle") or {}
    ni_meta = report.get("native_image_oracle") or {}
    lines = [
        f"### M7 Benchmark Summary (`profile={report['profile']}`, `baseline={report.get('baseline_mode', 'both')}`, "
        f"`exact_native_image_lock_match={ni_meta.get('exact_native_image_lock_match')}`, "
        f"`exact_jdk_lock_match={oracle_meta.get('exact_jdk_lock_match')}`)",
        "",
        "| Workload | Rust Time / RSS | Official Native Image (Time / RSS) | vs Native Image | Temurin 21 JVM (Time / RSS) | vs Temurin 21 | vs M0 GraalVM 25 |",
        "| --- | ---: | ---: | :---: | ---: | :---: | :---: |",
    ]
    for r in report["results"]:
        vni = r.get("vs_official_native_image")
        vt = r.get("vs_live_temurin_21")
        vg = r.get("vs_m0_graalvm_25_0_1")
        active = vni or vt
        rust_cell = (
            f"{fmt_ns(active['rust_time_ns'])} / {fmt_mb(active['rust_peak_rss_bytes'])}"
            if active is not None
            else "—"
        )
        if vni is not None:
            ni_raw = f"{fmt_ns(vni['native_image_time_ns'])} / {fmt_mb(vni['native_image_peak_rss_bytes'])}"
            ni_ratio = f"**{vni['time_ratio']:.3f}x / {vni['rss_ratio']:.3f}x** (`{vni['verdict']}`)"
        else:
            ni_raw = "— (CLI-only)"
            ni_ratio = "—"
        if vt is not None:
            j_raw = f"{fmt_ns(vt['java_time_ns'])} / {fmt_mb(vt['java_peak_rss_bytes'])}"
            j_ratio = f"**{vt['time_ratio']:.3f}x / {vt['rss_ratio']:.3f}x** (`{vt['verdict']}`)"
        else:
            j_raw = "—"
            j_ratio = "—"
        g_cell = (
            f"{vg['time_ratio']:.3f}x / {vg['rss_ratio']:.3f}x (`{vg['verdict']}`)"
            if vg is not None
            else "—"
        )
        lines.append(
            f"| `{r['id']}` | {rust_cell} | {ni_raw} | {ni_ratio} | {j_raw} | {j_ratio} | {g_cell} |"
        )
    lines.append("")
    counts = report.get("counts", {})
    if counts.get("vs_official_native_image_compared"):
        lines.append(
            f"- Official Apple Pkl 0.32.0 native-image verdicts ({counts['vs_official_native_image_compared']} CLI/server/pkldoc): "
            f"`{json.dumps(counts['vs_official_native_image_verdicts'], sort_keys=True)}`"
        )
    if counts.get("vs_live_temurin_21_verdicts"):
        lines.append(
            f"- Live Temurin 21 JVM verdicts: `{json.dumps(counts['vs_live_temurin_21_verdicts'], sort_keys=True)}`"
        )
    if counts.get("vs_m0_graalvm_25_0_1_compared"):
        lines.append(
            f"- vs M0 GraalVM CE 25.0.1 verdicts: `{json.dumps(counts['vs_m0_graalvm_25_0_1_verdicts'], sort_keys=True)}`"
        )
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--profile", choices=("full", "quick"), default="full")
    parser.add_argument(
        "--baseline",
        choices=VALID_BASELINES,
        default="both",
        help="baseline oracle(s): both (default, required for strict), native-image (CLI/server/pkldoc only), or jvm",
    )
    parser.add_argument("--families", type=str, default=None, help="comma-separated subset of workload families")
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--check-contract", action="store_true")
    parser.add_argument("--verify-report", type=Path, default=None)
    parser.add_argument("--verify-strict", type=Path, default=None)
    parser.add_argument("--summary", type=Path, default=None)
    args = parser.parse_args()
    root = args.root.resolve()
    try:
        if args.check_contract:
            c = load_contract(root)
            ni_lock = validate_native_image_lock(root)
            all_ids, _, native_ids = expected_workload_sets(c)
            print(
                json.dumps(
                    {
                        "baseline": c["baseline"],
                        "core_workloads": len(c["core_workloads"]),
                        "product_workloads": len(c["product_workloads"]),
                        "total_workload_modes": len(all_ids),
                        "native_image_eligible_workloads": len(native_ids),
                        "native_image_pinned_platforms": sorted(ni_lock["distributions"].keys()),
                        "families": list(ALL_FAMILIES),
                    },
                    sort_keys=True,
                )
            )
            return 0
        if args.verify_report:
            rep = verify_report(args.verify_report, root=root, strict=False)
            print(json.dumps(rep["counts"], sort_keys=True))
            return 0
        if args.verify_strict:
            rep = verify_report(args.verify_strict, root=root, strict=True)
            print(json.dumps(rep["counts"], sort_keys=True))
            return 0
        if args.summary:
            rep = verify_report(args.summary, root=root, strict=False)
            print(format_markdown_summary(rep))
            return 0

        fam_set = {f.strip() for f in args.families.split(",") if f.strip()} if args.families else None
        rep = run_suite(
            root=root,
            output=args.output,
            profile=args.profile,
            families=fam_set,
            skip_build=args.skip_build,
            baseline=args.baseline,
        )
        print(format_markdown_summary(rep))
        return 0
    except (OSError, ValueError, KeyError, RuntimeError, TimeoutError, subprocess.SubprocessError, oracle.OracleError) as err:
        print(f"m7_benchmark: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
