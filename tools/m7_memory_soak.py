#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""M7 Long-Running Memory Reclamation Soak Suite (Native Rust, Node N-API, and Wasm).

Executes all 6 required M7 memory reclamation scenarios from PLAN.md:
  1. repeated-success               : thousands of evaluations on one persistent evaluator
  2. repeated-failure               : thousands of failing evaluations (type/constraint/runtime/syntax/lazy/caught)
  3. unique-sources-churn           : hundreds of unique module URIs/sources (uncached reader + fresh evaluator close)
  4. cyclic-graphs                  : cyclic objects, class rings, cyclic closures/collections (+ SourceSession GC audit)
  5. lifecycle-cancel-timeout-close : repeated create -> eval -> cancel()/timeout -> idempotent close() (FD & PID leak checks)
  6. cache-churn                    : repeated module/package/modulepath cache loads, invalidations, and reloads

Runs across:
  - native     : recorded-build `crates/pkl/examples/m7_memory_soak_probe.rs`
  - napi-node  : `@pkl-rs/napi` under Node.js (`--expose-gc`), tracking RSS, heapUsed, FDs, child PIDs
  - wasm-node  : `@pkl-rs/wasm` under Node.js (`--expose-gc`), tracking Wasm linear memory pages + Node RSS/heapUsed

Measurement honesty & integrity:
  - Each (target, scenario) executes in a fresh isolated OS child process so RSS/Wasm pages never bleed across scenarios.
  - Synchronized checkpoint sampling (`--sync-stdin`) captures exact kernel OS RSS, virtual memory, open FD count,
    and child PIDs (via macOS `proc_pidinfo`/`proc_listchildpids` or Linux `/proc/<pid>`) PLUS `os.wait4` `ru_maxrss`.
  - Objective leak verdicts (bounded plateau vs monotonic unbounded growth) are deterministically recomputed from raw
    sealed time-series files during `--verify-report` and `--verify-strict`.
  - Every evidence file is sealed under SHA-256 in `report.json` (`--verify-report` preserves recorded leaks;
    `--verify-strict` requires 0 leaks, 0 errors, 0 orphan children, `recorded-build`, and `source_stability == true`).
"""
from __future__ import annotations

import argparse
from collections import Counter
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import select
import shutil
import signal
import struct
import subprocess
import sys
import time
import zipfile

import compat
import oracle

ROOT = Path(__file__).resolve().parents[1]

ALL_SCENARIOS = (
    "repeated-success",
    "repeated-failure",
    "unique-sources-churn",
    "cyclic-graphs",
    "lifecycle-cancel-timeout-close",
    "cache-churn",
)
ALL_TARGETS = ("native", "napi-node", "wasm-node")

NATIVE_PROBE_EXAMPLE = "m7_memory_soak_probe"
NAPI_SOAK_RUNNER = Path("tests/fixtures/m7/soak/run_napi_soak.mjs")
WASM_SOAK_RUNNER = Path("tests/fixtures/m7/soak/run_wasm_soak.mjs")
NAPI_PKG_ENTRY = Path("packages/pkl-napi/dist/index.mjs")
WASM_PKG_ENTRY = Path("packages/pkl-wasm/dist/index.mjs")
M4_EXT_READER_SCRIPT = Path("tests/fixtures/m4/readers/m4_ext_reader.py")

WASM_PAGE_BYTES = 65536
MIB = 1024 * 1024

# Default iteration budgets (standard vs quick profile). Every scenario yields >= 10 post-warmup samples.
PROFILE_ITERATIONS: dict[str, dict[str, dict[str, tuple[int, int]]]] = {
    "standard": {
        "native": {
            "repeated-success": (1500, 300),
            "repeated-failure": (1500, 150),
            "unique-sources-churn": (500, 50),
            "cyclic-graphs": (1000, 500),
            "lifecycle-cancel-timeout-close": (120, 12),
            "cache-churn": (300, 30),
        },
        "napi-node": {
            "repeated-success": (1000, 100),
            "repeated-failure": (1000, 100),
            "unique-sources-churn": (400, 40),
            "cyclic-graphs": (600, 60),
            "lifecycle-cancel-timeout-close": (100, 10),
            "cache-churn": (240, 24),
        },
        "wasm-node": {
            "repeated-success": (600, 60),
            "repeated-failure": (600, 60),
            "unique-sources-churn": (300, 30),
            "cyclic-graphs": (400, 40),
            "lifecycle-cancel-timeout-close": (100, 10),
            "cache-churn": (240, 24),
        },
    },
    "quick": {
        "native": {
            "repeated-success": (300, 70),
            "repeated-failure": (300, 70),
            "unique-sources-churn": (120, 20),
            "cyclic-graphs": (200, 70),
            "lifecycle-cancel-timeout-close": (30, 5),
            "cache-churn": (50, 10),
        },
        "napi-node": {
            "repeated-success": (250, 25),
            "repeated-failure": (250, 25),
            "unique-sources-churn": (100, 15),
            "cyclic-graphs": (160, 20),
            "lifecycle-cancel-timeout-close": (30, 5),
            "cache-churn": (40, 8),
        },
        "wasm-node": {
            "repeated-success": (200, 20),
            "repeated-failure": (200, 20),
            "unique-sources-churn": (80, 12),
            "cyclic-graphs": (120, 15),
            "lifecycle-cancel-timeout-close": (30, 5),
            "cache-churn": (40, 8),
        },
    },
}

# Explicit, objective reclamation thresholds recorded verbatim in report.json.
# Calibrated against real plateau noise (lifecycle-cancel-timeout-close: <=192 KiB RSS / 0 KiB Wasm)
# so sustained second-half linear leaks are caught even on shorter-iteration churn scenarios.
DEFAULT_THRESHOLDS = {
    "native": {
        "rss_noise_floor_bytes": 2 * MIB,
        "max_rss_delta_bytes": 24 * MIB,
        "max_rss_growth_ratio": 1.60,
        "max_second_half_delta_bytes": 2 * MIB,
        "max_second_half_slope_bytes_per_iter": 2048.0,
        "monotonic_r_squared_threshold": 0.70,
        "max_fd_delta": 4,
        "max_fd_peak_delta": 16,
        "max_orphan_child_pids": 0,
        "max_heap_probe_live_delta": 64,
    },
    "napi-node": {
        "rss_noise_floor_bytes": 4 * MIB,
        "max_rss_delta_bytes": 48 * MIB,
        "max_rss_growth_ratio": 1.85,
        "max_second_half_delta_bytes": 4 * MIB,
        "max_second_half_slope_bytes_per_iter": 4096.0,
        "monotonic_r_squared_threshold": 0.70,
        "max_fd_delta": 6,
        "max_fd_peak_delta": 24,
        "max_orphan_child_pids": 0,
    },
    "wasm-node": {
        "rss_noise_floor_bytes": 4 * MIB,
        "max_rss_delta_bytes": 48 * MIB,
        "max_rss_growth_ratio": 1.85,
        "max_second_half_delta_bytes": 4 * MIB,
        "max_second_half_slope_bytes_per_iter": 4096.0,
        "monotonic_r_squared_threshold": 0.70,
        "max_wasm_delta_bytes": 16 * MIB,
        "max_wasm_second_half_delta_bytes": 512 * 1024,
        "max_wasm_slope_bytes_per_iter": 2048.0,
        "max_wasm_second_half_slope_bytes_per_iter": 2048.0,
        "max_fd_delta": 6,
        "max_fd_peak_delta": 24,
        "max_orphan_child_pids": 0,
    },
}


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


def snapshot_sources(root: Path) -> dict[str, str]:
    tracked_dirs = (
        "crates/pkl-runtime/src",
        "crates/pkl-host-native/src",
        "crates/pkl/src",
        "crates/pkl/examples",
        "crates/pkl-wasm/src",
        "crates/pkl-napi/src",
        "packages/pkl-wasm/dist",
        "packages/pkl-napi/dist",
        "packages/pkl-napi/native",
        "tests/fixtures/m7/soak",
    )
    tracked_files = (
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "tools/m7_memory_soak.py",
        "tests/fixtures/m4/readers/m4_ext_reader.py",
    )
    out: dict[str, str] = {}
    for rel in tracked_files:
        p = root / rel
        if p.is_file():
            out[rel] = oracle.sha256(p)
    for drel in tracked_dirs:
        d = root / drel
        if not d.is_dir():
            continue
        for current, dirs, files in os.walk(d, followlinks=False):
            dirs[:] = sorted(x for x in dirs if x != "__pycache__")
            for name in sorted(files):
                p = Path(current) / name
                if p.is_file() and not p.is_symlink():
                    out[p.relative_to(root).as_posix()] = oracle.sha256(p)
    return dict(sorted(out.items()))


# ---- Kernel-level OS process telemetry (macOS libSystem / Linux /proc) ----

_LIBSYSTEM = None
if sys.platform == "darwin":
    try:
        _LIBSYSTEM = ctypes.CDLL("/usr/lib/libSystem.B.dylib")
    except OSError:
        _LIBSYSTEM = None


def sample_os_process(pid: int) -> dict:
    """Inspect live OS RSS, VMS, open FDs, and direct child PIDs without spawning `ps` or `lsof`."""
    rss_bytes = None
    vms_bytes = None
    open_fds = None
    child_pids: list[int] = []

    if sys.platform == "darwin" and _LIBSYSTEM is not None:
        buf = ctypes.create_string_buffer(256)
        # PROC_PIDTASKINFO = 4; first two uint64 fields are pti_virtual_size and pti_resident_size
        n = _LIBSYSTEM.proc_pidinfo(ctypes.c_int(pid), ctypes.c_int(4), ctypes.c_uint64(0), buf, ctypes.c_int(len(buf)))
        if n >= 16:
            vms_bytes, rss_bytes = struct.unpack_from("<QQ", buf.raw, 0)
        # PROC_PIDLISTFDS = 1; pass real buffer so kernel returns exact active FD byte count (8 bytes per struct proc_fdinfo)
        hint_bytes = _LIBSYSTEM.proc_pidinfo(ctypes.c_int(pid), ctypes.c_int(1), ctypes.c_uint64(0), None, ctypes.c_int(0))
        if hint_bytes > 0:
            fd_buf = ctypes.create_string_buffer(max(hint_bytes * 2, 1024))
            actual_bytes = _LIBSYSTEM.proc_pidinfo(
                ctypes.c_int(pid), ctypes.c_int(1), ctypes.c_uint64(0), fd_buf, ctypes.c_int(len(fd_buf))
            )
            if actual_bytes >= 0:
                open_fds = actual_bytes // 8
        cbuf = (ctypes.c_int * 256)()
        cn = _LIBSYSTEM.proc_listchildpids(ctypes.c_int(pid), cbuf, ctypes.c_int(ctypes.sizeof(cbuf)))
        if cn > 0:
            count = min(cn // ctypes.sizeof(ctypes.c_int), 256)
            child_pids = sorted(int(cbuf[i]) for i in range(count) if int(cbuf[i]) > 0)
    elif sys.platform.startswith("linux"):
        try:
            page_size = os.sysconf("SC_PAGE_SIZE")
            statm = Path(f"/proc/{pid}/statm").read_text(encoding="utf-8").split()
            vms_bytes = int(statm[0]) * page_size
            rss_bytes = int(statm[1]) * page_size
        except OSError:
            pass
        try:
            open_fds = len(os.listdir(f"/proc/{pid}/fd"))
        except OSError:
            pass
        try:
            found: set[int] = set()
            for child_file in Path(f"/proc/{pid}/task").glob("*/children"):
                for tok in child_file.read_text(encoding="utf-8").split():
                    found.add(int(tok))
            child_pids = sorted(found)
        except OSError:
            pass

    return {
        "os_rss_bytes": rss_bytes,
        "os_vms_bytes": vms_bytes,
        "os_open_fds": open_fds,
        "os_child_pids": child_pids,
        "os_child_count": len(child_pids),
    }


def wait4_ru_maxrss_bytes(rusage) -> int:
    """Normalize `os.wait4` ru_maxrss to bytes (bytes on macOS, KiB on Linux)."""
    raw = int(rusage.ru_maxrss)
    return raw if sys.platform == "darwin" else raw * 1024


# ---- Linear regression & objective leak evaluation ----

def ols_linear_regression(xs: list[float], ys: list[float]) -> tuple[float, float]:
    """Return (slope_per_unit_x, r_squared) for Ordinary Least Squares linear regression."""
    n = len(xs)
    if n < 2:
        return 0.0, 0.0
    mx = sum(xs) / n
    my = sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    syy = sum((y - my) ** 2 for y in ys)
    if sxx <= 0.0:
        return 0.0, 0.0
    sxy = sum((x - mx) * (y - my) for x, y in zip(xs, ys, strict=True))
    slope = sxy / sxx
    if syy <= 0.0:
        return slope, 0.0
    r = sxy / ((sxx * syy) ** 0.5)
    return slope, max(0.0, min(1.0, r * r))


def analyze_series(
    target: str,
    scenario: str,
    raw_run: dict,
    thresholds: dict,
) -> dict:
    """Deterministically compute memory/FD/PID metrics and leak verdict from raw sealed samples."""
    t_cfg = thresholds[target]
    samples = raw_run.get("samples", [])
    summary = raw_run.get("summary") or {}
    exit_code = raw_run.get("exit_code")
    timed_out = bool(raw_run.get("timed_out", False))
    expected_iters = int(raw_run.get("requested_iterations", 0))
    completed_iters = int(summary.get("completed_iterations", 0))
    assertion_failures = list(summary.get("assertion_failures") or [])

    # Extract post-warmup RSS series (preferring kernel os_rss_bytes, falling back to self_rss_bytes)
    iters: list[float] = []
    rss_series: list[int] = []
    heap_used_series: list[int] = []
    wasm_series: list[int] = []
    fd_series: list[int] = []
    child_counts: list[int] = []

    for s in samples:
        it = float(s.get("iter", 0))
        rss = s.get("os_rss_bytes")
        if rss is None:
            rss = s.get("self_rss_bytes")
        if rss is not None:
            iters.append(it)
            rss_series.append(int(rss))
        if s.get("heap_used_bytes") is not None:
            heap_used_series.append(int(s["heap_used_bytes"]))
        if s.get("wasm_linear_memory_bytes") is not None:
            wasm_series.append(int(s["wasm_linear_memory_bytes"]))
        fd = s.get("os_open_fds")
        if fd is None:
            fd = s.get("open_fds")
        if fd is not None:
            fd_series.append(int(fd))
        if s.get("os_child_count") is not None:
            child_counts.append(int(s["os_child_count"]))

    errors: list[str] = []
    leak_reasons: list[str] = []

    if timed_out:
        errors.append("process-timed-out")
    if exit_code != 0:
        errors.append(f"non-zero-exit:{exit_code}")
    if completed_iters != expected_iters:
        errors.append(f"incomplete-iterations:{completed_iters}/{expected_iters}")
    if assertion_failures:
        errors.append(f"assertion-failures:{len(assertion_failures)}")
    if len(rss_series) < 3:
        errors.append(f"insufficient-rss-samples:{len(rss_series)}")

    # RSS stats
    rss_stats: dict = {}
    if rss_series:
        mid_idx = len(rss_series) // 2
        baseline = rss_series[0]
        midpoint = rss_series[mid_idx]
        final = rss_series[-1]
        peak = max(rss_series)
        delta = final - baseline
        second_half_delta = final - midpoint
        growth_ratio = round(final / max(baseline, 1), 6)
        peak_ratio = round(peak / max(baseline, 1), 6)
        slope, r2 = ols_linear_regression(iters, [float(v) for v in rss_series])
        sh_slope, sh_r2 = ols_linear_regression(iters[mid_idx:], [float(v) for v in rss_series[mid_idx:]])
        rss_stats = {
            "sample_count": len(rss_series),
            "baseline_bytes": baseline,
            "midpoint_bytes": midpoint,
            "final_bytes": final,
            "peak_sample_bytes": peak,
            "wait4_peak_rss_bytes": raw_run.get("wait4_peak_rss_bytes"),
            "delta_bytes": delta,
            "second_half_delta_bytes": second_half_delta,
            "growth_ratio": growth_ratio,
            "peak_ratio": peak_ratio,
            "slope_bytes_per_iter": round(slope, 3),
            "r_squared": round(r2, 6),
            "second_half_slope_bytes_per_iter": round(sh_slope, 3),
            "second_half_r_squared": round(sh_r2, 6),
        }

        if delta > t_cfg["rss_noise_floor_bytes"]:
            if delta > t_cfg["max_rss_delta_bytes"] and growth_ratio > t_cfg["max_rss_growth_ratio"]:
                leak_reasons.append(
                    f"rss-total-growth:delta={delta}B,ratio={growth_ratio:.3f}>"
                    f"{t_cfg['max_rss_growth_ratio']:.2f}"
                )
        # Evaluate sustained second-half monotonic trend using second-half R^2 (sh_r2)
        if (
            second_half_delta > t_cfg["max_second_half_delta_bytes"]
            and sh_slope > t_cfg["max_second_half_slope_bytes_per_iter"]
            and sh_r2 >= t_cfg["monotonic_r_squared_threshold"]
        ):
            leak_reasons.append(
                f"rss-monotonic-second-half-growth:sh_delta={second_half_delta}B,"
                f"sh_slope={sh_slope:.1f}B/iter,sh_r2={sh_r2:.3f}"
            )

    # V8 JS heapUsed stats (informational + diagnostic for Node targets)
    js_heap_stats = None
    if heap_used_series:
        mid_idx = len(heap_used_series) // 2
        js_heap_stats = {
            "baseline_bytes": heap_used_series[0],
            "midpoint_bytes": heap_used_series[mid_idx],
            "final_bytes": heap_used_series[-1],
            "peak_bytes": max(heap_used_series),
            "delta_bytes": heap_used_series[-1] - heap_used_series[0],
        }

    # Wasm linear memory stats (strict plateau check for wasm-node)
    wasm_stats = None
    if target == "wasm-node":
        if len(wasm_series) < 3:
            errors.append(f"insufficient-wasm-memory-samples:{len(wasm_series)}")
        else:
            mid_idx = len(wasm_series) // 2
            w_base = wasm_series[0]
            w_mid = wasm_series[mid_idx]
            w_final = wasm_series[-1]
            w_peak = max(wasm_series)
            w_delta = w_final - w_base
            w_sh_delta = w_final - w_mid
            w_iters = [float(s.get("iter", 0)) for s in samples if s.get("wasm_linear_memory_bytes") is not None]
            w_slope, w_r2 = ols_linear_regression(w_iters, [float(v) for v in wasm_series])
            w_sh_slope, w_sh_r2 = ols_linear_regression(
                w_iters[mid_idx:], [float(v) for v in wasm_series[mid_idx:]]
            )
            wasm_stats = {
                "baseline_bytes": w_base,
                "midpoint_bytes": w_mid,
                "final_bytes": w_final,
                "peak_bytes": w_peak,
                "baseline_pages": w_base // WASM_PAGE_BYTES,
                "midpoint_pages": w_mid // WASM_PAGE_BYTES,
                "final_pages": w_final // WASM_PAGE_BYTES,
                "peak_pages": w_peak // WASM_PAGE_BYTES,
                "delta_bytes": w_delta,
                "delta_pages": w_delta // WASM_PAGE_BYTES,
                "second_half_delta_bytes": w_sh_delta,
                "second_half_delta_pages": w_sh_delta // WASM_PAGE_BYTES,
                "slope_bytes_per_iter": round(w_slope, 3),
                "r_squared": round(w_r2, 6),
                "second_half_slope_bytes_per_iter": round(w_sh_slope, 3),
                "second_half_r_squared": round(w_sh_r2, 6),
            }
            if w_delta > t_cfg["max_wasm_delta_bytes"]:
                leak_reasons.append(
                    f"wasm-linear-memory-delta:{w_delta}B({w_delta // WASM_PAGE_BYTES}pages)>"
                    f"{t_cfg['max_wasm_delta_bytes']}B"
                )
            if w_sh_delta > t_cfg["max_wasm_second_half_delta_bytes"]:
                leak_reasons.append(
                    f"wasm-linear-memory-second-half-growth:{w_sh_delta}B>"
                    f"{t_cfg['max_wasm_second_half_delta_bytes']}B"
                )
            if w_slope > t_cfg["max_wasm_slope_bytes_per_iter"] and w_sh_delta > 0:
                leak_reasons.append(
                    f"wasm-linear-memory-slope:{w_slope:.1f}B/iter>{t_cfg['max_wasm_slope_bytes_per_iter']}"
                )
            max_w_sh_slope = t_cfg.get(
                "max_wasm_second_half_slope_bytes_per_iter",
                t_cfg["max_wasm_slope_bytes_per_iter"],
            )
            if w_sh_slope > max_w_sh_slope and w_sh_delta > 0:
                leak_reasons.append(
                    f"wasm-linear-memory-second-half-slope:{w_sh_slope:.1f}B/iter>{max_w_sh_slope}"
                )

    # FD stats
    fd_stats = None
    if fd_series:
        fd_base = fd_series[0]
        fd_final = fd_series[-1]
        fd_peak = max(fd_series)
        fd_delta = fd_final - fd_base
        fd_peak_delta = fd_peak - fd_base
        fd_stats = {
            "baseline": fd_base,
            "final": fd_final,
            "peak": fd_peak,
            "delta": fd_delta,
            "peak_delta": fd_peak_delta,
        }
        if fd_delta > t_cfg["max_fd_delta"]:
            leak_reasons.append(f"fd-leak:delta={fd_delta}>{t_cfg['max_fd_delta']}")
        if fd_peak_delta > t_cfg["max_fd_peak_delta"]:
            leak_reasons.append(f"fd-peak-spike:peak_delta={fd_peak_delta}>{t_cfg['max_fd_peak_delta']}")

    # Child PID / orphan process stats
    final_sample_children = child_counts[-1] if child_counts else 0
    post_exit_orphans = list(raw_run.get("post_exit_orphan_pids") or [])
    orphan_count = max(final_sample_children, len(post_exit_orphans))
    pid_stats = {
        "max_children_during_samples": max(child_counts) if child_counts else 0,
        "final_sample_children": final_sample_children,
        "post_exit_orphan_pids": post_exit_orphans,
        "orphan_child_count": orphan_count,
    }
    if orphan_count > t_cfg["max_orphan_child_pids"]:
        leak_reasons.append(f"orphan-child-processes:{orphan_count}")

    # Native SourceSession HeapStats cycle reclamation audit (cyclic-graphs)
    heap_probe = summary.get("heap_probe")
    if target == "native" and scenario == "cyclic-graphs":
        if not isinstance(heap_probe, dict):
            errors.append("missing-native-cyclic-heap-probe")
        else:
            w_live = int((heap_probe.get("after_warmup") or {}).get("live", 0))
            f_live = int((heap_probe.get("after_final") or {}).get("live", 0))
            f_col = int((heap_probe.get("after_final") or {}).get("collections", 0))
            f_rec = int((heap_probe.get("after_final") or {}).get("reclaimed", 0))
            if not heap_probe.get("assertions_ok"):
                errors.append("heap-probe-assertion-failed")
            if f_col <= 0 or f_rec <= 0:
                leak_reasons.append(f"heap-probe-no-gc-reclamation:collections={f_col},reclaimed={f_rec}")
            if f_live - w_live > t_cfg["max_heap_probe_live_delta"]:
                leak_reasons.append(
                    f"heap-probe-live-growth:warmup={w_live},final={f_live},delta={f_live - w_live}"
                )

    if errors:
        verdict = "error"
    elif leak_reasons:
        verdict = "leak-detected"
    else:
        verdict = "bounded"

    return {
        "target": target,
        "scenario": scenario,
        "verdict": verdict,
        "errors": errors,
        "leak_reasons": leak_reasons,
        "requested_iterations": expected_iters,
        "warmup_iterations": int(raw_run.get("requested_warmup", 0)),
        "completed_iterations": completed_iters,
        "assertions_passed": int(summary.get("assertions_passed", 0)),
        "assertion_failures": assertion_failures,
        "elapsed_ms": summary.get("elapsed_ms", raw_run.get("wall_elapsed_ms")),
        "rss": rss_stats,
        "js_heap": js_heap_stats,
        "wasm_linear_memory": wasm_stats,
        "fds": fd_stats,
        "child_processes": pid_stats,
        "heap_probe": heap_probe,
    }


def generate_pkg_pool(pool_dir: Path, count: int = 8) -> Path:
    """Create deterministic offline package-2 ZIPs, metadata JSONs, and modulepath JARs."""
    pool_dir.mkdir(parents=True, exist_ok=True)
    rows: list[str] = []
    for gen in range(count):
        zip_name = f"soakpkg_gen{gen}.zip"
        meta_name = f"soakpkg_gen{gen}.json"
        jar_name = f"mp_gen{gen}.jar"
        zip_path = pool_dir / zip_name
        with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_STORED) as zf:
            for ename, content in (
                ("PklProject", 'amends "pkl:Project"\n'),
                ("lib/info.pkl", f"pkgGen = {gen}\n"),
            ):
                info = zipfile.ZipInfo(ename, date_time=(1980, 2, 1, 0, 0, 0))
                zf.writestr(info, content.encode("utf-8"))
        zip_sha = oracle.sha256(zip_path)
        meta_obj = {
            "name": "soakpkg",
            "packageUri": "package://soak.local/soakpkg@1.0.0",
            "version": "1.0.0",
            "packageZipUrl": "https://soak.local/soakpkg@1.0.0.zip",
            "packageZipChecksums": {"sha256": zip_sha},
            "dependencies": {},
        }
        meta_bytes = json.dumps(meta_obj, separators=(",", ":")).encode("utf-8")
        meta_path = pool_dir / meta_name
        meta_path.write_bytes(meta_bytes)
        meta_sha = digest(meta_bytes)

        jar_path = pool_dir / jar_name
        with zipfile.ZipFile(jar_path, "w", compression=zipfile.ZIP_STORED) as zf:
            info = zipfile.ZipInfo("mp.pkl", date_time=(1980, 2, 1, 0, 0, 0))
            zf.writestr(info, f"mpGen = {gen}\n".encode("utf-8"))

        rows.append(f"{gen}\t{meta_sha}\t{meta_name}\t{zip_name}\t{jar_name}")
    (pool_dir / "pool.tsv").write_text("\n".join(rows) + "\n", encoding="utf-8")
    return pool_dir


def build_native_probe(root: Path, output: Path, cargo_target_dir: Path | None) -> tuple[Path, dict]:
    """Build and seal `crates/pkl/examples/m7_memory_soak_probe.rs` in release mode."""
    cmd = ["cargo", "build", "--release", "-p", "pkl-rs", "--example", NATIVE_PROBE_EXAMPLE, "--locked"]
    env = dict(os.environ)
    if cargo_target_dir is not None:
        env["CARGO_TARGET_DIR"] = str(cargo_target_dir)
    tdir = cargo_target_dir if cargo_target_dir is not None else (root / "target")
    t0 = time.perf_counter_ns()
    proc = subprocess.run(cmd, cwd=root, env=env, capture_output=True, timeout=600)
    elapsed_ns = time.perf_counter_ns() - t0
    (output / f"build-{NATIVE_PROBE_EXAMPLE}.stdout.bin").write_bytes(proc.stdout)
    (output / f"build-{NATIVE_PROBE_EXAMPLE}.stderr.bin").write_bytes(proc.stderr)
    if proc.returncode != 0:
        raise RuntimeError(
            f"failed building {NATIVE_PROBE_EXAMPLE} (rc={proc.returncode}): "
            f"{proc.stderr.decode('utf-8', errors='replace')[-2000:]}"
        )
    built = tdir / "release/examples" / NATIVE_PROBE_EXAMPLE
    if not built.is_file():
        raise RuntimeError(f"built example missing: {built}")
    art_dir = output / "artifacts"
    art_dir.mkdir(parents=True, exist_ok=True)
    sealed = art_dir / NATIVE_PROBE_EXAMPLE
    shutil.copy2(built, sealed)
    meta = {
        "name": NATIVE_PROBE_EXAMPLE,
        "sha256": oracle.sha256(sealed),
        "bytes": sealed.stat().st_size,
        "source_binding": "recorded-build",
        "build": {
            "bin": NATIVE_PROBE_EXAMPLE,
            "command": cmd,
            "exit_code": proc.returncode,
            "elapsed_ns": elapsed_ns,
            "stdout_sha256": digest(proc.stdout),
            "stderr_sha256": digest(proc.stderr),
        },
    }
    return sealed, meta


def run_synchronized_probe(
    command: list[str],
    cwd: Path,
    env: dict[str, str],
    timeout_seconds: float,
    requested_iterations: int,
    requested_warmup: int,
) -> dict:
    """Execute one soak probe with synchronized `--sync-stdin` kernel telemetry sampling."""
    t0 = time.perf_counter_ns()
    proc = subprocess.Popen(
        command,
        cwd=cwd,
        env=env,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    samples: list[dict] = []
    summary: dict | None = None
    unparsed_lines: list[str] = []
    timed_out = False
    deadline = time.monotonic() + timeout_seconds

    assert proc.stdout is not None and proc.stdin is not None and proc.stderr is not None
    stdout_fd = proc.stdout.fileno()
    buf = bytearray()

    def handle_line(raw_line: bytes) -> None:
        nonlocal summary
        text = raw_line.decode("utf-8", errors="replace").strip()
        if not text:
            return
        try:
            obj = json.loads(text)
        except json.JSONDecodeError:
            unparsed_lines.append(text[:400])
            return
        kind = obj.get("type")
        if kind == "sample":
            os_stats = sample_os_process(proc.pid)
            obj.update(os_stats)
            samples.append(obj)
            try:
                proc.stdin.write(b"\n")
                proc.stdin.flush()
            except BrokenPipeError:
                pass
        elif kind == "summary":
            summary = obj
        else:
            unparsed_lines.append(text[:400])

    try:
        eof = False
        while not eof:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                timed_out = True
                break
            ready, _, _ = select.select([stdout_fd], [], [], remaining)
            if not ready:
                timed_out = True
                break
            chunk = os.read(stdout_fd, 4096)
            if not chunk:
                eof = True
                break
            buf.extend(chunk)
            while True:
                nl = buf.find(b"\n")
                if nl < 0:
                    break
                line_bytes = bytes(buf[:nl])
                del buf[: nl + 1]
                handle_line(line_bytes)
        if buf and not timed_out:
            handle_line(bytes(buf))
    finally:
        if timed_out:
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass
        try:
            proc.stdin.close()
        except OSError:
            pass

    stderr_bytes = b""
    stderr_fd = proc.stderr.fileno()
    stderr_deadline = time.monotonic() + 5.0
    stderr_chunks: list[bytes] = []
    while True:
        rem = stderr_deadline - time.monotonic()
        if rem <= 0:
            break
        rdy, _, _ = select.select([stderr_fd], [], [], rem)
        if not rdy:
            break
        chunk = os.read(stderr_fd, 4096)
        if not chunk:
            break
        stderr_chunks.append(chunk)
    stderr_bytes = b"".join(stderr_chunks)
    proc.stdout.close()
    proc.stderr.close()

    wait4_peak_rss_bytes = None
    exit_code = None
    try:
        _, status, rusage = os.wait4(proc.pid, 0)
        exit_code = os.waitstatus_to_exitcode(status)
        proc.returncode = exit_code
        wait4_peak_rss_bytes = wait4_ru_maxrss_bytes(rusage)
    except ChildProcessError:
        exit_code = proc.wait(timeout=5)

    # Check if any process still exists in the child's process group (orphan child check!)
    post_exit_orphans: list[int] = []
    try:
        os.killpg(proc.pid, 0)
        # Process group still alive after parent exit -> orphan child exists!
        post_exit_orphans.append(proc.pid)
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
    except (ProcessLookupError, PermissionError):
        pass

    return {
        "command": command,
        "requested_iterations": requested_iterations,
        "requested_warmup": requested_warmup,
        "exit_code": exit_code,
        "timed_out": timed_out,
        "wall_elapsed_ms": round((time.perf_counter_ns() - t0) / 1_000_000.0, 2),
        "wait4_peak_rss_bytes": wait4_peak_rss_bytes,
        "post_exit_orphan_pids": post_exit_orphans,
        "samples": samples,
        "summary": summary,
        "unparsed_stdout_lines": unparsed_lines,
        "stderr_text": stderr_bytes.decode("utf-8", errors="replace"),
        "stderr_sha256": digest(stderr_bytes),
    }


def execute(
    root: Path,
    output: Path,
    targets: list[str],
    scenarios: list[str],
    profile: str,
    timeout_seconds: float,
    cargo_target_dir: Path | None,
) -> dict:
    if output.exists() and any(output.iterdir()):
        raise ValueError(f"output directory already exists and is non-empty: {output}")
    output.mkdir(parents=True, exist_ok=True)

    sources_before = snapshot_sources(root)

    candidates: dict[str, dict] = {}
    native_bin: Path | None = None
    if "native" in targets:
        native_bin, native_meta = build_native_probe(root, output, cargo_target_dir)
        candidates["native_probe"] = native_meta

    node_bin = shutil.which("node")
    node_version = None
    if any(t in targets for t in ("napi-node", "wasm-node")):
        if not node_bin:
            raise RuntimeError("node executable not found on PATH")
        nv = subprocess.run([node_bin, "--version"], capture_output=True, text=True, check=True, timeout=15)
        node_version = nv.stdout.strip()

    if "napi-node" in targets:
        napi_entry = root / NAPI_PKG_ENTRY
        napi_native_dir = root / "packages/pkl-napi/native"
        native_addons = sorted(p for p in napi_native_dir.glob("*.node") if p.is_file())
        if not napi_entry.is_file() or not native_addons:
            raise RuntimeError("built @pkl-rs/napi package/native addon missing under packages/pkl-napi/")
        candidates["napi_package"] = {
            "entry": NAPI_PKG_ENTRY.as_posix(),
            "entry_sha256": oracle.sha256(napi_entry),
            "native_addons": {
                p.relative_to(root).as_posix(): oracle.sha256(p) for p in native_addons
            },
            "runner": NAPI_SOAK_RUNNER.as_posix(),
            "runner_sha256": oracle.sha256(root / NAPI_SOAK_RUNNER),
            "source_binding": "workspace-dist",
        }

    if "wasm-node" in targets:
        wasm_entry = root / WASM_PKG_ENTRY
        wasm_bin = root / "packages/pkl-wasm/dist/wasm/pkl_wasm_bg.wasm"
        if not wasm_entry.is_file() or not wasm_bin.is_file():
            raise RuntimeError("built @pkl-rs/wasm package/wasm binary missing under packages/pkl-wasm/dist/")
        candidates["wasm_package"] = {
            "entry": WASM_PKG_ENTRY.as_posix(),
            "entry_sha256": oracle.sha256(wasm_entry),
            "wasm_binary": wasm_bin.relative_to(root).as_posix(),
            "wasm_binary_sha256": oracle.sha256(wasm_bin),
            "runner": WASM_SOAK_RUNNER.as_posix(),
            "runner_sha256": oracle.sha256(root / WASM_SOAK_RUNNER),
            "source_binding": "workspace-dist",
        }

    work_root = output / ".work"
    pkg_pool_dir = generate_pkg_pool(work_root / "pkg_pool", count=8)

    base_env = {
        "PATH": os.environ.get("PATH", os.defpath),
        "LC_ALL": "C",
        "LANG": "C",
        "TZ": "UTC",
        "NO_COLOR": "1",
        "PYTHONDONTWRITEBYTECODE": "1",
    }

    results: list[dict] = []
    runs_dir = output / "runs"
    runs_dir.mkdir(parents=True, exist_ok=True)

    try:
        for target in targets:
            for scenario in scenarios:
                iters, warmup = PROFILE_ITERATIONS[profile][target][scenario]
                sample_interval = max(1, iters // 10)
                case_work = work_root / f"{target}__{scenario}"
                case_work.mkdir(parents=True, exist_ok=True)
                home = case_work / "home"
                tmp = case_work / "tmp"
                home.mkdir(exist_ok=True)
                tmp.mkdir(exist_ok=True)
                env = {**base_env, "HOME": str(home), "TMPDIR": str(tmp), "TMP": str(tmp), "TEMP": str(tmp)}

                if target == "native":
                    assert native_bin is not None
                    cmd = [
                        str(native_bin),
                        "--scenario", scenario,
                        "--iterations", str(iters),
                        "--warmup", str(warmup),
                        "--sample-interval", str(sample_interval),
                        "--work-dir", str(case_work),
                        "--pkg-pool-dir", str(pkg_pool_dir),
                        "--ext-reader-python", sys.executable,
                        "--ext-reader-script", str(root / M4_EXT_READER_SCRIPT),
                        "--sync-stdin",
                    ]
                elif target == "napi-node":
                    assert node_bin is not None
                    cmd = [
                        node_bin,
                        "--expose-gc",
                        str(root / NAPI_SOAK_RUNNER),
                        "--scenario", scenario,
                        "--iterations", str(iters),
                        "--warmup", str(warmup),
                        "--sample-interval", str(sample_interval),
                        "--work-dir", str(case_work),
                        "--pkg-entry", str(root / NAPI_PKG_ENTRY),
                        "--pkg-pool-dir", str(pkg_pool_dir),
                        "--ext-reader-python", sys.executable,
                        "--ext-reader-script", str(root / M4_EXT_READER_SCRIPT),
                        "--sync-stdin",
                    ]
                elif target == "wasm-node":
                    assert node_bin is not None
                    cmd = [
                        node_bin,
                        "--expose-gc",
                        str(root / WASM_SOAK_RUNNER),
                        "--scenario", scenario,
                        "--iterations", str(iters),
                        "--warmup", str(warmup),
                        "--sample-interval", str(sample_interval),
                        "--work-dir", str(case_work),
                        "--pkg-entry", str(root / WASM_PKG_ENTRY),
                        "--pkg-pool-dir", str(pkg_pool_dir),
                        "--sync-stdin",
                    ]
                else:
                    raise ValueError(f"unsupported target: {target}")

                raw_run = run_synchronized_probe(cmd, root, env, timeout_seconds, iters, warmup)
                raw_rel = f"runs/{target}/{scenario}.json"
                compat.write_json(output / raw_rel, raw_run)

                analysis = analyze_series(target, scenario, raw_run, DEFAULT_THRESHOLDS)
                analysis["raw_run_file"] = raw_rel
                analysis["raw_run_sha256"] = oracle.sha256(output / raw_rel)
                results.append(analysis)
    finally:
        shutil.rmtree(work_root, ignore_errors=True)

    sources_after = snapshot_sources(root)
    source_stability = sources_before == sources_after
    compat.write_json(output / "sources.snapshot.json", {"before": sources_before, "after": sources_after})

    verdict_counts = dict(sorted(Counter(r["verdict"] for r in results).items()))
    target_verdict_counts = {
        t: dict(sorted(Counter(r["verdict"] for r in results if r["target"] == t).items()))
        for t in targets
    }
    scenario_verdict_counts = {
        s: dict(sorted(Counter(r["verdict"] for r in results if r["scenario"] == s).items()))
        for s in scenarios
    }

    report = {
        "schema_version": 1,
        "suite": "m7-memory-reclamation-soak",
        "baseline": "0.32.0",
        "profile": profile,
        "host": {
            "platform": platform.platform(),
            "system": platform.system(),
            "machine": platform.machine(),
            "python": platform.python_version(),
            "node": node_version,
        },
        "targets": targets,
        "scenarios": scenarios,
        "thresholds": DEFAULT_THRESHOLDS,
        "candidates": candidates,
        "sources_snapshot_sha256": oracle.sha256(output / "sources.snapshot.json"),
        "sources_file_count": len(sources_before),
        "source_stability": source_stability,
        "total_runs": len(results),
        "bounded_count": verdict_counts.get("bounded", 0),
        "leak_detected_count": verdict_counts.get("leak-detected", 0),
        "error_count": verdict_counts.get("error", 0),
        "verdict_counts": verdict_counts,
        "target_verdict_counts": target_verdict_counts,
        "scenario_verdict_counts": scenario_verdict_counts,
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
    if report.get("schema_version") != 1 or report.get("suite") != "m7-memory-reclamation-soak":
        raise ValueError("unsupported M7 memory soak report schema_version or suite")

    recorded_evidence = report.get("evidence_sha256", {})
    actual_files = {str(p.relative_to(directory).as_posix()) for p in evidence_paths(directory)}
    if not recorded_evidence or set(recorded_evidence) != actual_files:
        raise ValueError("evidence file set differs from recorded evidence_sha256")
    for rel, expected_sha in recorded_evidence.items():
        if oracle.sha256(directory / rel) != expected_sha:
            raise ValueError(f"evidence file modified: {rel}")

    src_snap = directory / "sources.snapshot.json"
    if not src_snap.is_file() or oracle.sha256(src_snap) != report.get("sources_snapshot_sha256"):
        raise ValueError("sources.snapshot.json missing or sha256 disagrees with report.json")
    snap_doc = json.loads(src_snap.read_bytes())
    if (snap_doc.get("before") == snap_doc.get("after")) != report.get("source_stability"):
        raise ValueError("recomputed source_stability disagrees with report.json")

    cands = report.get("candidates") or {}
    snap_before = snap_doc.get("before") or {}
    snap_after = snap_doc.get("after") or {}
    if "native_probe" in cands:
        np = cands["native_probe"]
        rel = f"artifacts/{np.get('name')}"
        if rel not in recorded_evidence or np.get("sha256") != recorded_evidence[rel]:
            raise ValueError("native_probe artifact sha256 mismatch")
        brec = np.get("build") or {}
        for stream, key in (("stdout", "stdout_sha256"), ("stderr", "stderr_sha256")):
            brel = f"build-{np.get('name')}.{stream}.bin"
            if brel not in recorded_evidence or brec.get(key) != recorded_evidence[brel]:
                raise ValueError(f"native_probe build {stream} sha256 mismatch")

    if "napi_package" in cands:
        napi_c = cands["napi_package"]
        for field, key in (("entry", "entry_sha256"), ("runner", "runner_sha256")):
            path_key = napi_c.get(field)
            if not path_key or snap_before.get(path_key) != napi_c.get(key) or snap_after.get(path_key) != napi_c.get(key):
                raise ValueError(f"napi_package {field} sha256 disagrees with sources.snapshot.json: {path_key}")
        addons = napi_c.get("native_addons") or {}
        if not addons:
            raise ValueError("napi_package missing native_addons provenance")
        for addon_rel, addon_sha in addons.items():
            if snap_before.get(addon_rel) != addon_sha or snap_after.get(addon_rel) != addon_sha:
                raise ValueError(f"napi_package native addon sha256 disagrees with sources.snapshot.json: {addon_rel}")

    if "wasm_package" in cands:
        wasm_c = cands["wasm_package"]
        for field, key in (
            ("entry", "entry_sha256"),
            ("wasm_binary", "wasm_binary_sha256"),
            ("runner", "runner_sha256"),
        ):
            path_key = wasm_c.get(field)
            if not path_key or snap_before.get(path_key) != wasm_c.get(key) or snap_after.get(path_key) != wasm_c.get(key):
                raise ValueError(f"wasm_package {field} sha256 disagrees with sources.snapshot.json: {path_key}")

    thresholds = report.get("thresholds")
    if not isinstance(thresholds, dict):
        raise ValueError("missing thresholds object in report.json")

    expected_pairs = [(t, s) for t in report["targets"] for s in report["scenarios"]]
    actual_pairs = [(r["target"], r["scenario"]) for r in report["results"]]
    if actual_pairs != expected_pairs or len(actual_pairs) != report.get("total_runs"):
        raise ValueError("result (target, scenario) membership or order disagrees with report header")

    for row in report["results"]:
        raw_rel = row.get("raw_run_file")
        if not raw_rel or raw_rel not in recorded_evidence:
            raise ValueError(f"missing sealed raw run file for {row['target']}/{row['scenario']}")
        if row.get("raw_run_sha256") != recorded_evidence[raw_rel]:
            raise ValueError(f"raw_run_sha256 disagrees for {raw_rel}")
        raw_run = json.loads((directory / raw_rel).read_bytes())
        recomputed = analyze_series(row["target"], row["scenario"], raw_run, thresholds)
        recomputed["raw_run_file"] = raw_rel
        recomputed["raw_run_sha256"] = row["raw_run_sha256"]
        if recomputed != row:
            raise ValueError(
                f"recomputed soak metrics/verdict disagree with report.json for "
                f"{row['target']}/{row['scenario']}: expected verdict={recomputed['verdict']}, got {row['verdict']}"
            )

    recomp_verdicts = dict(sorted(Counter(r["verdict"] for r in report["results"]).items()))
    if recomp_verdicts != report.get("verdict_counts"):
        raise ValueError("recomputed verdict_counts disagree with report.json")
    if report.get("bounded_count") != recomp_verdicts.get("bounded", 0):
        raise ValueError("bounded_count disagrees with results")
    if report.get("leak_detected_count") != recomp_verdicts.get("leak-detected", 0):
        raise ValueError("leak_detected_count disagrees with results")
    if report.get("error_count") != recomp_verdicts.get("error", 0):
        raise ValueError("error_count disagrees with results")

    recomp_by_target = {
        t: dict(sorted(Counter(r["verdict"] for r in report["results"] if r["target"] == t).items()))
        for t in report["targets"]
    }
    if recomp_by_target != report.get("target_verdict_counts"):
        raise ValueError("recomputed target_verdict_counts disagree with report.json")
    recomp_by_scen = {
        s: dict(sorted(Counter(r["verdict"] for r in report["results"] if r["scenario"] == s).items()))
        for s in report["scenarios"]
    }
    if recomp_by_scen != report.get("scenario_verdict_counts"):
        raise ValueError("recomputed scenario_verdict_counts disagree with report.json")

    if strict:
        if report.get("source_stability") is not True:
            raise ValueError("strict soak gate requires source_stability == true")
        if report.get("profile") != "standard":
            raise ValueError(f"strict soak gate requires profile == 'standard', got {report.get('profile')!r}")
        if thresholds != DEFAULT_THRESHOLDS:
            raise ValueError("strict soak gate requires exact DEFAULT_THRESHOLDS (custom/weakened thresholds rejected)")
        if set(report.get("scenarios", [])) != set(ALL_SCENARIOS):
            raise ValueError(f"strict soak gate requires all 6 scenarios: {ALL_SCENARIOS}")
        if set(report.get("targets", [])) != set(ALL_TARGETS):
            raise ValueError(f"strict soak gate requires all 3 targets: {ALL_TARGETS}")
        if (cands.get("native_probe") or {}).get("source_binding") != "recorded-build":
            raise ValueError("strict soak gate requires recorded-build native_probe")
        if "napi_package" not in cands or "wasm_package" not in cands:
            raise ValueError("strict soak gate requires napi_package and wasm_package provenance")
        bad = [r for r in report["results"] if r["verdict"] != "bounded"]
        if bad:
            detail = ", ".join(
                f"{r['target']}/{r['scenario']}={r['verdict']}({';'.join(r['errors'] + r['leak_reasons'])})"
                for r in bad
            )
            raise ValueError(f"strict soak gate failed ({len(bad)} non-bounded run(s)): {detail}")

    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--output", type=Path, help="fresh evidence directory for live soak run")
    action.add_argument("--verify-report", type=Path, help="verify integrity & recompute metrics (preserves leaks)")
    action.add_argument("--verify-strict", type=Path, help="verify integrity AND enforce strict zero-leak M7 gate")
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--target", action="append", choices=ALL_TARGETS, help="select target(s); default all 3")
    parser.add_argument("--scenario", action="append", choices=ALL_SCENARIOS, help="select scenario(s); default all 6")
    parser.add_argument("--profile", choices=("standard", "quick"), default="standard")
    parser.add_argument("--timeout", type=float, default=240.0, help="per-(target, scenario) timeout seconds")
    parser.add_argument("--record-only", action="store_true", help="exit 0 after sealing even if leaks detected")
    parser.add_argument("--cargo-target-dir", type=Path, help="Cargo target directory for recorded build")
    args = parser.parse_args()

    try:
        if args.output:
            targets = list(dict.fromkeys(args.target)) if args.target else list(ALL_TARGETS)
            scenarios = list(dict.fromkeys(args.scenario)) if args.scenario else list(ALL_SCENARIOS)
            report = execute(
                args.root.resolve(),
                args.output.resolve(),
                targets,
                scenarios,
                args.profile,
                args.timeout,
                args.cargo_target_dir,
            )
            print(json.dumps({
                "suite": report["suite"],
                "profile": report["profile"],
                "total_runs": report["total_runs"],
                "bounded_count": report["bounded_count"],
                "leak_detected_count": report["leak_detected_count"],
                "error_count": report["error_count"],
                "verdict_counts": report["verdict_counts"],
                "target_verdict_counts": report["target_verdict_counts"],
            }, sort_keys=True))
            if args.record_only:
                return 0
            verify_report(args.output.resolve(), strict=True)
            return 0
        target_dir = (args.verify_report or args.verify_strict).resolve()
        report = verify_report(target_dir, strict=bool(args.verify_strict))
        print(json.dumps({
            "suite": report["suite"],
            "profile": report["profile"],
            "total_runs": report["total_runs"],
            "bounded_count": report["bounded_count"],
            "leak_detected_count": report["leak_detected_count"],
            "error_count": report["error_count"],
            "verdict_counts": report["verdict_counts"],
            "target_verdict_counts": report["target_verdict_counts"],
        }, sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"M7 memory soak error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
