#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Unit, leak-detection, timeout, and tamper-detection tests for tools/m7_memory_soak.py."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import sys
import tempfile
import time
import unittest

import compat
import m7_memory_soak


def _make_raw_run(
    *,
    rss_values: list[int],
    wasm_values: list[int] | None = None,
    fd_values: list[int] | None = None,
    child_counts: list[int] | None = None,
    post_exit_orphans: list[int] | None = None,
    requested_iterations: int = 1000,
    completed_iterations: int | None = None,
    exit_code: int = 0,
    assertion_failures: list[str] | None = None,
    heap_probe: dict | None = None,
) -> dict:
    n = len(rss_values)
    step = requested_iterations // max(1, n - 1)
    samples = []
    for idx, rss in enumerate(rss_values):
        s = {
            "type": "sample",
            "phase": "warmup-end" if idx == 0 else ("final" if idx == n - 1 else "sample"),
            "iter": idx * step if idx < n - 1 else requested_iterations,
            "elapsed_ms": idx * 50,
            "os_rss_bytes": rss,
            "os_open_fds": fd_values[idx] if fd_values else 8,
            "os_child_count": child_counts[idx] if child_counts else 0,
        }
        if wasm_values is not None:
            s["wasm_linear_memory_bytes"] = wasm_values[idx]
            s["wasm_linear_memory_pages"] = wasm_values[idx] // m7_memory_soak.WASM_PAGE_BYTES
        samples.append(s)
    summary = {
        "type": "summary",
        "completed_iterations": requested_iterations if completed_iterations is None else completed_iterations,
        "assertions_passed": requested_iterations,
        "elapsed_ms": n * 50,
        "assertion_failures": assertion_failures or [],
    }
    if heap_probe is not None:
        summary["heap_probe"] = heap_probe
    return {
        "requested_iterations": requested_iterations,
        "requested_warmup": 100,
        "exit_code": exit_code,
        "timed_out": False,
        "wall_elapsed_ms": n * 50.0,
        "wait4_peak_rss_bytes": max(rss_values),
        "post_exit_orphan_pids": post_exit_orphans or [],
        "samples": samples,
        "summary": summary,
    }


def _build_synthetic_soak_report(
    rep_dir: Path,
    *,
    profile: str = "standard",
    source_stability: bool = True,
    source_binding: str = "recorded-build",
    inject_leak: bool = False,
    custom_thresholds: dict | None = None,
    tamper_napi_addon_sha: bool = False,
) -> None:
    rep_dir.mkdir(parents=True)
    art = rep_dir / "artifacts"
    art.mkdir()
    (art / m7_memory_soak.NATIVE_PROBE_EXAMPLE).write_bytes(b"FAKE-PROBE-BIN")
    (rep_dir / f"build-{m7_memory_soak.NATIVE_PROBE_EXAMPLE}.stdout.bin").write_bytes(b"")
    (rep_dir / f"build-{m7_memory_soak.NATIVE_PROBE_EXAMPLE}.stderr.bin").write_bytes(b"ok\n")

    snap_before = {
        "Cargo.toml": "111",
        "packages/pkl-napi/dist/index.mjs": "aaa",
        "packages/pkl-napi/native/pkl_napi.darwin-arm64.node": "bbb",
        "tests/fixtures/m7/soak/run_napi_soak.mjs": "ccc",
        "packages/pkl-wasm/dist/index.mjs": "ddd",
        "packages/pkl-wasm/dist/wasm/pkl_wasm_bg.wasm": "eee",
        "tests/fixtures/m7/soak/run_wasm_soak.mjs": "fff",
    }
    snap_after = dict(snap_before)
    if not source_stability:
        snap_after["Cargo.toml"] = "222"
    compat.write_json(rep_dir / "sources.snapshot.json", {"before": snap_before, "after": snap_after})

    thresholds = copy.deepcopy(custom_thresholds) if custom_thresholds is not None else copy.deepcopy(m7_memory_soak.DEFAULT_THRESHOLDS)
    results = []
    for target in m7_memory_soak.ALL_TARGETS:
        for scenario in m7_memory_soak.ALL_SCENARIOS:
            if inject_leak and target == "native" and scenario == "repeated-success":
                rss = [20 * m7_memory_soak.MIB + i * 4 * m7_memory_soak.MIB for i in range(11)]
            else:
                # Realistic warmup plateau: small bump early, flat second half
                rss = [20 * m7_memory_soak.MIB + min(i, 2) * 512 * 1024 for i in range(11)]
            wasm = (
                [4 * m7_memory_soak.MIB + min(i, 1) * 65536 for i in range(11)]
                if target == "wasm-node"
                else None
            )
            heap_probe = (
                {
                    "iterations": 160,
                    "warmup": 20,
                    "after_warmup": {"live": 500, "executable_live": 400, "slots": 600, "collections": 3, "reclaimed": 1200},
                    "midpoint": {"live": 502, "executable_live": 400, "slots": 620, "collections": 10, "reclaimed": 9000},
                    "after_final": {"live": 504, "executable_live": 400, "slots": 620, "collections": 20, "reclaimed": 18000},
                    "assertions_ok": True,
                }
                if target == "native" and scenario == "cyclic-graphs"
                else None
            )
            raw_run = _make_raw_run(rss_values=rss, wasm_values=wasm, heap_probe=heap_probe)
            raw_rel = f"runs/{target}/{scenario}.json"
            compat.write_json(rep_dir / raw_rel, raw_run)
            row = m7_memory_soak.analyze_series(target, scenario, raw_run, thresholds)
            row["raw_run_file"] = raw_rel
            row["raw_run_sha256"] = m7_memory_soak.digest((rep_dir / raw_rel).read_bytes())
            results.append(row)

    from collections import Counter
    vc = dict(sorted(Counter(r["verdict"] for r in results).items()))
    tvc = {t: dict(sorted(Counter(r["verdict"] for r in results if r["target"] == t).items())) for t in m7_memory_soak.ALL_TARGETS}
    svc = {s: dict(sorted(Counter(r["verdict"] for r in results if r["scenario"] == s).items())) for s in m7_memory_soak.ALL_SCENARIOS}

    report = {
        "schema_version": 1,
        "suite": "m7-memory-reclamation-soak",
        "baseline": "0.32.0",
        "profile": profile,
        "host": {"platform": "test", "system": "Darwin", "machine": "arm64", "python": "3.14", "node": "v26"},
        "targets": list(m7_memory_soak.ALL_TARGETS),
        "scenarios": list(m7_memory_soak.ALL_SCENARIOS),
        "thresholds": thresholds,
        "candidates": {
            "native_probe": {
                "name": m7_memory_soak.NATIVE_PROBE_EXAMPLE,
                "sha256": m7_memory_soak.digest((art / m7_memory_soak.NATIVE_PROBE_EXAMPLE).read_bytes()),
                "source_binding": source_binding,
                "build": {
                    "bin": m7_memory_soak.NATIVE_PROBE_EXAMPLE,
                    "stdout_sha256": m7_memory_soak.digest(b""),
                    "stderr_sha256": m7_memory_soak.digest(b"ok\n"),
                },
            },
            "napi_package": {
                "entry": "packages/pkl-napi/dist/index.mjs",
                "entry_sha256": "aaa",
                "native_addons": {
                    "packages/pkl-napi/native/pkl_napi.darwin-arm64.node": "TAMPERED" if tamper_napi_addon_sha else "bbb",
                },
                "runner": "tests/fixtures/m7/soak/run_napi_soak.mjs",
                "runner_sha256": "ccc",
                "source_binding": "workspace-dist",
            },
            "wasm_package": {
                "entry": "packages/pkl-wasm/dist/index.mjs",
                "entry_sha256": "ddd",
                "wasm_binary": "packages/pkl-wasm/dist/wasm/pkl_wasm_bg.wasm",
                "wasm_binary_sha256": "eee",
                "runner": "tests/fixtures/m7/soak/run_wasm_soak.mjs",
                "runner_sha256": "fff",
                "source_binding": "workspace-dist",
            },
        },
        "sources_snapshot_sha256": m7_memory_soak.digest((rep_dir / "sources.snapshot.json").read_bytes()),
        "sources_file_count": len(snap_before),
        "source_stability": source_stability,
        "total_runs": len(results),
        "bounded_count": vc.get("bounded", 0),
        "leak_detected_count": vc.get("leak-detected", 0),
        "error_count": vc.get("error", 0),
        "verdict_counts": vc,
        "target_verdict_counts": tvc,
        "scenario_verdict_counts": svc,
        "results": results,
    }
    report["evidence_sha256"] = {
        str(p.relative_to(rep_dir).as_posix()): m7_memory_soak.digest(p.read_bytes())
        for p in sorted(m7_memory_soak.evidence_paths(rep_dir))
    }
    compat.write_json(rep_dir / "report.json", report)


class M7MemorySoakTests(unittest.TestCase):
    def test_ols_linear_regression_exact(self) -> None:
        slope, r2 = m7_memory_soak.ols_linear_regression([0.0, 100.0, 200.0, 300.0], [1000.0, 2000.0, 3000.0, 4000.0])
        self.assertAlmostEqual(slope, 10.0, places=6)
        self.assertAlmostEqual(r2, 1.0, places=6)

        slope_flat, r2_flat = m7_memory_soak.ols_linear_regression([0.0, 100.0, 200.0], [500.0, 500.0, 500.0])
        self.assertAlmostEqual(slope_flat, 0.0, places=6)
        self.assertAlmostEqual(r2_flat, 0.0, places=6)

    def test_detects_monotonic_rss_leak_vs_plateau_and_short_churn_leaks(self) -> None:
        # Plateau: initial bump then flat second half -> bounded
        plateau_run = _make_raw_run(
            rss_values=[20 * m7_memory_soak.MIB, 26 * m7_memory_soak.MIB] + [27 * m7_memory_soak.MIB] * 9,
            requested_iterations=1000,
        )
        res_ok = m7_memory_soak.analyze_series("native", "repeated-success", plateau_run, m7_memory_soak.DEFAULT_THRESHOLDS)
        self.assertEqual(res_ok["verdict"], "bounded")

        # Short-churn linear leak (+38 KiB/iter over 200 iters = +7.4 MiB total, +3.7 MiB second-half) -> MUST be caught (S1)!
        short_leak = _make_raw_run(
            rss_values=[20 * m7_memory_soak.MIB + i * 760 * 1024 for i in range(11)],
            requested_iterations=200,
        )
        res_short = m7_memory_soak.analyze_series("native", "cache-churn", short_leak, m7_memory_soak.DEFAULT_THRESHOLDS)
        self.assertEqual(res_short["verdict"], "leak-detected")
        self.assertTrue(any("rss-monotonic-second-half-growth" in r for r in res_short["leak_reasons"]))

        # N-API moderate linear leak (+42 KiB/iter over 360 iters, sh_delta=7.5 MiB) -> MUST be caught (S1)!
        napi_leak = _make_raw_run(
            rss_values=[88 * m7_memory_soak.MIB + i * 1500 * 1024 for i in range(11)],
            requested_iterations=360,
        )
        res_napi = m7_memory_soak.analyze_series("napi-node", "unique-sources-churn", napi_leak, m7_memory_soak.DEFAULT_THRESHOLDS)
        self.assertEqual(res_napi["verdict"], "leak-detected")

    def test_detects_fd_leak_and_orphan_child_and_wasm_growth_and_cyclic_gc_leak(self) -> None:
        # FD leak
        fd_run = _make_raw_run(
            rss_values=[20 * m7_memory_soak.MIB] * 11,
            fd_values=[8 + i * 2 for i in range(11)],
        )
        res_fd = m7_memory_soak.analyze_series("native", "lifecycle-cancel-timeout-close", fd_run, m7_memory_soak.DEFAULT_THRESHOLDS)
        self.assertEqual(res_fd["verdict"], "leak-detected")
        self.assertTrue(any("fd-leak" in r for r in res_fd["leak_reasons"]))

        # Orphan child process
        orphan_run = _make_raw_run(
            rss_values=[20 * m7_memory_soak.MIB] * 11,
            post_exit_orphans=[99999],
        )
        res_orphan = m7_memory_soak.analyze_series("native", "lifecycle-cancel-timeout-close", orphan_run, m7_memory_soak.DEFAULT_THRESHOLDS)
        self.assertEqual(res_orphan["verdict"], "leak-detected")
        self.assertTrue(any("orphan-child-processes" in r for r in res_orphan["leak_reasons"]))

        # Wasm linear memory steady growth in second half (even small 11-page = 704 KiB growth!) -> MUST be caught (S1)!
        wasm_small_leak = _make_raw_run(
            rss_values=[180 * m7_memory_soak.MIB] * 11,
            wasm_values=[439 * 65536] * 6 + [(439 + (i - 5) * 2) * 65536 for i in range(6, 11)],
            requested_iterations=160,
        )
        res_wasm = m7_memory_soak.analyze_series("wasm-node", "cache-churn", wasm_small_leak, m7_memory_soak.DEFAULT_THRESHOLDS)
        self.assertEqual(res_wasm["verdict"], "leak-detected")
        self.assertIn("second_half_slope_bytes_per_iter", res_wasm["wasm_linear_memory"])
        self.assertIn("second_half_r_squared", res_wasm["wasm_linear_memory"])

        # Cyclic GC HeapStats growth
        bad_hp = {
            "iterations": 160,
            "warmup": 20,
            "after_warmup": {"live": 500, "executable_live": 400, "slots": 600, "collections": 3, "reclaimed": 1000},
            "midpoint": {"live": 1500, "executable_live": 400, "slots": 1600, "collections": 10, "reclaimed": 1000},
            "after_final": {"live": 2500, "executable_live": 400, "slots": 2600, "collections": 20, "reclaimed": 1000},
            "assertions_ok": True,
        }
        cyc_run = _make_raw_run(rss_values=[20 * m7_memory_soak.MIB] * 11, heap_probe=bad_hp)
        res_cyc = m7_memory_soak.analyze_series("native", "cyclic-graphs", cyc_run, m7_memory_soak.DEFAULT_THRESHOLDS)
        self.assertEqual(res_cyc["verdict"], "leak-detected")
        self.assertTrue(any("heap-probe-live-growth" in r for r in res_cyc["leak_reasons"]))

    def test_verify_report_strict_and_s2_loophole_rejection(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            rep_dir = Path(tmp) / "ok"
            _build_synthetic_soak_report(rep_dir)
            m7_memory_soak.verify_report(rep_dir, strict=False)
            m7_memory_soak.verify_report(rep_dir, strict=True)

            # Tamper verdict in report.json -> caught by recomputation
            rpath = rep_dir / "report.json"
            doc = json.loads(rpath.read_bytes())
            doc["results"][0]["verdict"] = "leak-detected"
            compat.write_json(rpath, doc)
            with self.assertRaisesRegex(ValueError, "recomputed soak metrics/verdict disagree"):
                m7_memory_soak.verify_report(rep_dir, strict=False)

            # S2: profile == "quick" MUST fail strict gate
            quick_dir = Path(tmp) / "quick"
            _build_synthetic_soak_report(quick_dir, profile="quick")
            m7_memory_soak.verify_report(quick_dir, strict=False)
            with self.assertRaisesRegex(ValueError, "profile == 'standard'"):
                m7_memory_soak.verify_report(quick_dir, strict=True)

            # S2: weakened thresholds MUST fail strict gate
            weak = copy.deepcopy(m7_memory_soak.DEFAULT_THRESHOLDS)
            weak["native"]["max_second_half_delta_bytes"] = 999 * m7_memory_soak.MIB
            weak_dir = Path(tmp) / "weak"
            _build_synthetic_soak_report(weak_dir, custom_thresholds=weak)
            m7_memory_soak.verify_report(weak_dir, strict=False)
            with self.assertRaisesRegex(ValueError, "DEFAULT_THRESHOLDS"):
                m7_memory_soak.verify_report(weak_dir, strict=True)

            # S2: tampered napi_package native addon SHA-256 rejected even in verify_report
            bad_addon_dir = Path(tmp) / "bad_addon"
            _build_synthetic_soak_report(bad_addon_dir, tamper_napi_addon_sha=True)
            with self.assertRaisesRegex(ValueError, "native addon sha256"):
                m7_memory_soak.verify_report(bad_addon_dir, strict=False)

            # Leak recorded honestly: verify_report passes, verify_strict rejects
            leak_dir = Path(tmp) / "leak"
            _build_synthetic_soak_report(leak_dir, inject_leak=True)
            rep = m7_memory_soak.verify_report(leak_dir, strict=False)
            self.assertEqual(rep["leak_detected_count"], 1)
            with self.assertRaisesRegex(ValueError, "strict soak gate failed"):
                m7_memory_soak.verify_report(leak_dir, strict=True)

            # Tampered raw file breaks SHA-256 evidence sealing
            raw_file = leak_dir / "runs/native/repeated-success.json"
            raw_file.write_bytes(raw_file.read_bytes() + b" ")
            with self.assertRaisesRegex(ValueError, "evidence file modified"):
                m7_memory_soak.verify_report(leak_dir, strict=False)

            # Unstable sources fail strict gate
            unstable_dir = Path(tmp) / "unstable"
            _build_synthetic_soak_report(unstable_dir, source_stability=False)
            m7_memory_soak.verify_report(unstable_dir, strict=False)
            with self.assertRaisesRegex(ValueError, "source_stability"):
                m7_memory_soak.verify_report(unstable_dir, strict=True)

    def test_hung_probe_respects_timeout_nonblocking(self) -> None:
        # S3: hung child writing nothing to stdout must be killed promptly at timeout_seconds
        t0 = time.monotonic()
        with tempfile.TemporaryDirectory() as tmp:
            res = m7_memory_soak.run_synchronized_probe(
                [sys.executable, "-c", "import time; time.sleep(30)"],
                Path(tmp),
                dict(m7_memory_soak.os.environ),
                timeout_seconds=0.35,
                requested_iterations=10,
                requested_warmup=2,
            )
        elapsed = time.monotonic() - t0
        self.assertTrue(res["timed_out"])
        self.assertLess(elapsed, 3.0)

    def test_generate_pkg_pool_produces_valid_archives(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pool_dir = m7_memory_soak.generate_pkg_pool(Path(tmp) / "pool", count=3)
            tsv = (pool_dir / "pool.tsv").read_text(encoding="utf-8").splitlines()
            self.assertEqual(len(tsv), 3)
            for idx, line in enumerate(tsv):
                gen, meta_sha, meta_name, zip_name, jar_name = line.split("\t")
                self.assertEqual(int(gen), idx)
                self.assertEqual(len(meta_sha), 64)
                self.assertEqual(m7_memory_soak.digest((pool_dir / meta_name).read_bytes()), meta_sha)
                self.assertTrue((pool_dir / zip_name).is_file())
                self.assertTrue((pool_dir / jar_name).is_file())


if __name__ == "__main__":
    unittest.main()
