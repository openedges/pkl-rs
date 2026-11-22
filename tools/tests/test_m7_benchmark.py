"""Unit and tamper-detection tests for tools/m7_benchmark.py (no performance parity claim)."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest

TOOLS = Path(__file__).resolve().parents[1]
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

import m7_benchmark
import m7_manifest


def _make_full_synthetic_report(root: Path, dest: Path, *, failing_id: str | None = None) -> dict:
    """Create a complete sealed synthetic M7 benchmark report for verifier tamper tests."""
    dest.mkdir(parents=True, exist_ok=True)
    contract = m7_benchmark.load_contract(root)
    sampling = contract["profiles"]["full"]
    js_sampling = contract["js_sampling"]["full"]
    all_ids = sorted(m7_manifest.expected_benchmark_workload_ids(contract))
    core_ids = {w["id"] for w in contract["core_workloads"]}

    m0_ref_path = root / contract["m0_graalvm_reference"]
    m0_sha = m7_benchmark.file_digest(m0_ref_path)
    m0_doc = json.loads(m0_ref_path.read_bytes())
    m0_cases_by_id = {c["id"]: c for c in m0_doc["cases"]}

    # Write sealed dummy preflight & snapshot files
    (dest / "preflight.bin").write_bytes(b"42\n")
    snap_files = {
        "packages/pkl-napi/native/pkl_napi.darwin-arm64.node": "a" * 64,
        "packages/pkl-wasm/dist/wasm/pkl_wasm_bg.wasm": "b" * 64,
        "tools/m7_benchmark.py": "c" * 64,
    }
    snap_sha = m7_benchmark.digest(json.dumps(snap_files, sort_keys=True, separators=(",", ":")).encode("utf-8"))
    m7_benchmark.write_json(
        dest / "sources.snapshot.json",
        {
            "before_sha256": snap_sha,
            "after_sha256": snap_sha,
            "source_stability": True,
            "before": snap_files,
            "after": snap_files,
        },
    )

    results = []
    graal_verdicts: dict[str, int] = {}
    graal_compared = 0

    for wid in all_ids:
        base_id, mode = wid.split(":", 1)
        is_fail = wid == failing_id
        rust_t = 105_000.0 if is_fail else 30_000.0
        java_t = 100_000.0
        rust_rss = 900_000_000.0 if mode == "wasm-browser" else 30_000_000.0
        java_rss = 250_000_000.0
        if mode == "wasm-browser":
            vs_t = m7_benchmark.evaluate_browser_wasm_verdict(
                rust_t,
                java_t,
                rust_rss,
                java_rss,
                wasm_linear_memory_bytes=16_000_000.0,
                js_heap_used_bytes=12_000_000.0,
                wasm_binary_gzip_bytes=2_400_000,
                correctness_verified=True,
                converged=True,
                browser_targets=contract["browser_wasm_targets"],
                browser_engine="chromium",
                workload_key="typed-conversion" if "typed" in base_id else "eval-text",
            )
        else:
            vs_t = m7_benchmark.evaluate_ratio_verdict(
                rust_t, java_t, rust_rss, java_rss, correctness_verified=True, converged=True
            )

        row: dict = {
            "id": wid,
            "workload_id": base_id,
            "family": "core-6" if base_id in core_ids else (mode if mode in ("napi", "wasm-node", "wasm-browser") else "other"),
            "mode": mode,
            "op_mode": "typed-conversion" if "typed" in base_id else "eval-expression",
            "correctness_verified": True,
            "vs_live_temurin_21": vs_t,
            "vs_m0_graalvm_25_0_1": None,
        }

        if mode == "cold-cli":
            j_samps = [{"elapsed_ns": java_t, "peak_rss_bytes": java_rss} for _ in range(sampling["cold_samples"])]
            r_samps = [{"elapsed_ns": rust_t, "peak_rss_bytes": rust_rss} for _ in range(sampling["cold_samples"])]
            row["java"] = {
                "elapsed_ns": m7_benchmark.distribution([s["elapsed_ns"] for s in j_samps]),
                "peak_rss_bytes": m7_benchmark.distribution([s["peak_rss_bytes"] for s in j_samps]),
                "samples": j_samps,
            }
            row["rust"] = {
                "elapsed_ns": m7_benchmark.distribution([s["elapsed_ns"] for s in r_samps]),
                "peak_rss_bytes": m7_benchmark.distribution([s["peak_rss_bytes"] for s in r_samps]),
                "samples": r_samps,
            }
        elif mode in ("persistent", "fresh-evaluator"):
            j_forks = [
                {"warmup": "converged", "operation_ns": m7_benchmark.distribution([java_t]), "peak_rss_bytes": java_rss}
                for _ in range(sampling["independent_api_forks"])
            ]
            r_forks = [
                {"warmup": "converged", "operation_ns": m7_benchmark.distribution([rust_t]), "peak_rss_bytes": rust_rss}
                for _ in range(sampling["independent_api_forks"])
            ]
            row["java"] = {
                "warmup": "converged",
                "fork_median_operation_ns": m7_benchmark.distribution([f["operation_ns"]["median"] for f in j_forks]),
                "fork_peak_rss_bytes": m7_benchmark.distribution([f["peak_rss_bytes"] for f in j_forks]),
                "forks": j_forks,
            }
            row["rust"] = {
                "warmup": "converged",
                "fork_median_operation_ns": m7_benchmark.distribution([f["operation_ns"]["median"] for f in r_forks]),
                "fork_peak_rss_bytes": m7_benchmark.distribution([f["peak_rss_bytes"] for f in r_forks]),
                "forks": r_forks,
            }
        elif mode == "steady-server":
            j_runs = [
                {"steady_request_ns": m7_benchmark.distribution([java_t]), "peak_rss_bytes": java_rss}
                for _ in range(sampling["independent_api_forks"])
            ]
            r_runs = [
                {"steady_request_ns": m7_benchmark.distribution([rust_t]), "peak_rss_bytes": rust_rss}
                for _ in range(sampling["independent_api_forks"])
            ]
            row["java"] = {
                "fork_median_operation_ns": m7_benchmark.distribution([r["steady_request_ns"]["median"] for r in j_runs]),
                "fork_peak_rss_bytes": m7_benchmark.distribution([r["peak_rss_bytes"] for r in j_runs]),
                "runs": j_runs,
            }
            row["rust"] = {
                "fork_median_operation_ns": m7_benchmark.distribution([r["steady_request_ns"]["median"] for r in r_runs]),
                "fork_peak_rss_bytes": m7_benchmark.distribution([r["peak_rss_bytes"] for r in r_runs]),
                "runs": r_runs,
            }
        elif mode in ("napi", "wasm-node", "wasm-browser"):
            js_rel = f"js/{base_id}-{mode}-result.json"
            meas = [{"phase": "measurement", "index": i, "elapsed_ns": 250_000_000, "iterations": 100, "ns_per_op": rust_t} for i in range(js_sampling["samples"])]
            js_doc = {
                "preflight_raw_byte_verified": True,
                "warmup": {"converged": True, "windows": js_sampling["convergence_windows"], "elapsed_ns": 3_000_000_000},
                "measurements": meas,
                "browser_engine": "chromium" if mode == "wasm-browser" else None,
                "browserErrors": [] if mode == "wasm-browser" else None,
                "chromium_peak_tree_rss_bytes": rust_rss if mode == "wasm-browser" else None,
                "wasm_linear_memory_bytes_peak": 16_000_000 if mode == "wasm-browser" else None,
                "js_heap_after_eval": {"usedJSHeapSize": 12_000_000} if mode == "wasm-browser" else None,
                "wasm_binary_gzip_bytes": 2_400_000 if mode == "wasm-browser" else None,
            }
            m7_benchmark.write_json(dest / js_rel, js_doc)
            row["java"] = {
                "warmup": "converged",
                "fork_median_operation_ns": m7_benchmark.distribution([java_t, java_t]),
                "fork_peak_rss_bytes": m7_benchmark.distribution([java_rss, java_rss]),
            }
            row["rust"] = {
                "runtime": mode,
                "warmup": "converged",
                "operation_ns": m7_benchmark.distribution([m["ns_per_op"] for m in meas]),
                "peak_rss_bytes": rust_rss,
                "browserErrors": [] if mode == "wasm-browser" else None,
                "chromium_peak_tree_rss_bytes": rust_rss if mode == "wasm-browser" else None,
                "js_result_rel_path": js_rel,
                "driver_capture": {"status": "executed", "exit_code": 0, "peak_rss_bytes": int(rust_rss)},
            }

        if base_id in core_ids:
            m0_c = m0_cases_by_id[base_id]
            if mode == "cold-cli":
                m0_t = float(m0_c["cold_cli"]["elapsed_ns"]["median"])
                m0_r = float(m0_c["cold_cli"]["peak_rss_bytes"]["median"])
            else:
                m0_t = float(m0_c["api"][mode]["fork_median_operation_ns"]["median"])
                m0_r = float(m0_c["api"][mode]["fork_peak_rss_bytes"]["median"])
            vs_g = m7_benchmark.evaluate_ratio_verdict(
                rust_t, m0_t, rust_rss, m0_r, correctness_verified=True, converged=True
            )
            row["vs_m0_graalvm_25_0_1"] = vs_g
            graal_compared += 1
            graal_verdicts[vs_g["verdict"]] = graal_verdicts.get(vs_g["verdict"], 0) + 1

        results.append(row)

    verdict_counts: dict[str, int] = {}
    for r in results:
        v = r["vs_live_temurin_21"]["verdict"]
        verdict_counts[v] = verdict_counts.get(v, 0) + 1

    report = {
        "schema_version": 1,
        "baseline": "0.32.0",
        "profile": "full",
        "strict_eligible": True,
        "complete_catalog": True,
        "selected_families": sorted(m7_benchmark.ALL_FAMILIES),
        "target_ratios": contract["target_ratios"],
        "browser_wasm_targets": contract["browser_wasm_targets"],
        "sampling": sampling,
        "js_sampling": js_sampling,
        "contract_sha256": m7_benchmark.file_digest(root / m7_benchmark.WORKLOADS_PATH),
        "oracle": {"version": "0.32.0", "exact_jdk_lock_match": True},
        "m0_graalvm_reference": {"path": contract["m0_graalvm_reference"], "sha256": m0_sha},
        "machine_start": {"system": "Darwin", "machine": "arm64"},
        "build": {
            "source_binding": "recorded-build",
            "artifacts": {
                "pkl": {"rel_path": "target/release/pkl", "sha256": "1" * 64, "source_binding": "recorded-build"},
                "pkldoc": {"rel_path": "target/release/pkldoc", "sha256": "2" * 64, "source_binding": "recorded-build"},
                "m7_bench_probe": {"rel_path": "target/release/examples/m7_bench_probe", "sha256": "3" * 64, "source_binding": "recorded-build"},
                "napi_addon": {
                    "rel_path": "packages/pkl-napi/native/pkl_napi.darwin-arm64.node",
                    "sha256": "a" * 64,
                    "source_binding": "recorded-build",
                },
                "wasm_bg": {
                    "rel_path": "packages/pkl-wasm/dist/wasm/pkl_wasm_bg.wasm",
                    "sha256": "b" * 64,
                    "source_binding": "recorded-build",
                },
            },
        },
        "sources_snapshot_sha256": snap_sha,
        "sources_after_sha256": snap_sha,
        "source_stability": True,
        "counts": {
            "total_workload_modes": len(results),
            "correctness_verified": len(results),
            "vs_live_temurin_21_verdicts": dict(sorted(verdict_counts.items())),
            "vs_m0_graalvm_25_0_1_compared": graal_compared,
            "vs_m0_graalvm_25_0_1_verdicts": dict(sorted(graal_verdicts.items())),
        },
        "results": results,
    }
    report["evidence_files"] = m7_benchmark.seal_evidence_files(dest)
    m7_benchmark.write_json(dest / "report.json", report)
    return report


class M7BenchmarkTests(unittest.TestCase):
    def test_contract_valid_and_rejects_relaxed_thresholds(self):
        contract = m7_benchmark.load_contract()
        self.assertEqual(contract["baseline"], "0.32.0")
        self.assertEqual(len(contract["core_workloads"]), 6)
        self.assertGreaterEqual(len(contract["product_workloads"]), 15)

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for rel in ("benchmarks/m7_workloads.json", "reports/m0/benchmarks.json"):
                dst = root / rel
                dst.parent.mkdir(parents=True, exist_ok=True)
                dst.write_bytes((m7_benchmark.ROOT / rel).read_bytes())
            for bad_case in (
                "relax-time",
                "relax-rss",
                "quick-strict",
                "relax-browser-time",
                "relax-browser-linear-mem",
                "relax-browser-js-heap",
                "relax-browser-gzip",
                "non-firefox-exception",
            ):
                mut = copy.deepcopy(contract)
                if bad_case == "relax-time":
                    mut["target_ratios"]["execution_time_max"] = 1.05
                elif bad_case == "relax-rss":
                    mut["target_ratios"]["peak_rss_max"] = 1.1
                elif bad_case == "quick-strict":
                    mut["profiles"]["quick"]["strict_eligible"] = True
                elif bad_case == "relax-browser-time":
                    mut["browser_wasm_targets"]["execution_time_max"] = 1.1
                elif bad_case == "relax-browser-linear-mem":
                    mut["browser_wasm_targets"]["wasm_linear_memory_ratio_max"] = 0.25
                elif bad_case == "relax-browser-js-heap":
                    mut["browser_wasm_targets"]["js_heap_used_max_bytes"] = 128 * 1024 * 1024
                elif bad_case == "relax-browser-gzip":
                    mut["browser_wasm_targets"]["wasm_binary_gzip_max_bytes"] = 4 * 1024 * 1024
                else:
                    mut["browser_wasm_targets"]["engine_time_exceptions"]["chromium"] = {
                        "typed-conversion": {"execution_time_max": 1.2, "reason": "disallowed"}
                    }
                (root / "benchmarks/m7_workloads.json").write_text(json.dumps(mut), encoding="utf-8")
                with self.assertRaises(ValueError):
                    m7_benchmark.load_contract(root)

    def test_ratio_verdict_honesty_never_hides_regression(self):
        ok = m7_benchmark.evaluate_ratio_verdict(100.0, 100.0, 50.0, 100.0, correctness_verified=True, converged=True)
        self.assertEqual(ok["verdict"], "pass")

        time_fail = m7_benchmark.evaluate_ratio_verdict(100.1, 100.0, 10.0, 100.0, correctness_verified=True, converged=True)
        self.assertEqual(time_fail["verdict"], "fail")
        self.assertFalse(time_fail["time_meets_target"])

        rss_fail = m7_benchmark.evaluate_ratio_verdict(10.0, 100.0, 100.1, 100.0, correctness_verified=True, converged=True)
        self.assertEqual(rss_fail["verdict"], "fail")
        self.assertFalse(rss_fail["rss_meets_target"])

        wrong = m7_benchmark.evaluate_ratio_verdict(10.0, 100.0, 10.0, 100.0, correctness_verified=False, converged=True)
        self.assertEqual(wrong["verdict"], "fail")

        inconc = m7_benchmark.evaluate_ratio_verdict(50.0, 100.0, 50.0, 100.0, correctness_verified=True, converged=False)
        self.assertEqual(inconc["verdict"], "inconclusive")

    def test_wait4_capture_raw_bytes_and_mismatch_rejection(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cap = m7_benchmark.capture_wait4(
                [sys.executable, "-c", "import os; os.write(1, b'raw \\r\\n\\x00')"],
                root,
                {},
                root / "ok",
            )
            out, err = m7_benchmark.verify_capture_raw(cap, expected_exit=0, expected_stdout=b"raw \r\n\x00", expected_stderr=b"")
            self.assertEqual(out, b"raw \r\n\x00")
            self.assertEqual(err, b"")
            self.assertGreater(cap["peak_rss_bytes"], 0)

            with self.assertRaisesRegex(ValueError, "raw stdout bytes differ"):
                m7_benchmark.verify_capture_raw(cap, expected_exit=0, expected_stdout=b"raw \r\n\x00 ")

            tcap = m7_benchmark.capture_wait4(
                [sys.executable, "-c", "import time; time.sleep(5)"],
                root,
                {},
                root / "timeout",
                timeout=0.05,
            )
            with self.assertRaisesRegex(ValueError, "unexecuted/timeout"):
                m7_benchmark.verify_capture_raw(tcap)

    def test_parse_probe_series_rejects_false_convergence(self):
        sampling = {
            "samples": 5,
            "convergence_windows": 4,
            "minimum_warmup_seconds": 10.0,
            "sample_seconds": 0.25,
            "maximum_relative_spread": 0.2,
        }
        rows = [
            {"phase": "initialize", "elapsed_ns": 1000},
            {"phase": "first-request", "elapsed_ns": 2000},
            {"phase": "warmup", "index": 0, "elapsed_ns": 250_000_000, "iterations": 100, "ns_per_op": 2_500_000.0},
            {"phase": "warmup", "index": 1, "elapsed_ns": 250_000_000, "iterations": 100, "ns_per_op": 2_500_000.0},
            {"phase": "warmup", "index": 2, "elapsed_ns": 250_000_000, "iterations": 100, "ns_per_op": 2_500_000.0},
            {"phase": "warmup", "index": 3, "elapsed_ns": 250_000_000, "iterations": 100, "ns_per_op": 2_500_000.0},
            {"phase": "warmup-result", "elapsed_ns": 10_000_000_000, "windows": 4, "converged": True},
        ] + [
            {"phase": "measurement", "index": i, "elapsed_ns": 250_000_000, "iterations": 100, "ns_per_op": 2_500_000.0}
            for i in range(5)
        ]

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            stdout = b"\n".join(json.dumps(r).encode() for r in rows)
            out_path = root / "stdout.bin"
            err_path = root / "stderr.bin"
            out_path.write_bytes(stdout)
            err_path.write_bytes(b"")
            cap = {
                "status": "executed",
                "exit_code": 0,
                "elapsed_ns": 15_000_000_000,
                "peak_rss_bytes": 4_000_000,
                "stdout": {"path": str(out_path), "sha256": m7_benchmark.digest(stdout), "bytes": len(stdout)},
                "stderr": {"path": str(err_path), "sha256": m7_benchmark.digest(b""), "bytes": 0},
            }
            parsed = m7_benchmark.parse_probe_series(cap, sampling, "persistent", "test")
            self.assertEqual(parsed["warmup"], "converged")

            bad_rows = copy.deepcopy(rows)
            bad_rows[2]["iterations"] = 25
            bad_rows[2]["ns_per_op"] = 10_000_000.0
            bad_stdout = b"\n".join(json.dumps(r).encode() for r in bad_rows)
            out_path.write_bytes(bad_stdout)
            cap["stdout"] = {"path": str(out_path), "sha256": m7_benchmark.digest(bad_stdout), "bytes": len(bad_stdout)}
            with self.assertRaisesRegex(ValueError, "false warmup convergence"):
                m7_benchmark.parse_probe_series(cap, sampling, "persistent", "test")

    def test_verify_report_and_strict_tamper_detection(self):
        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp) / "rep-pass"
            _make_full_synthetic_report(m7_benchmark.ROOT, dest)
            m7_benchmark.verify_report(dest, strict=False)
            m7_benchmark.verify_report(dest, strict=True)

            # Single workload regression (> 1.00x) passes --verify-report but FAILS --verify-strict
            dest_reg = Path(tmp) / "rep-reg"
            _make_full_synthetic_report(m7_benchmark.ROOT, dest_reg, failing_id="recursive-compute:persistent")
            m7_benchmark.verify_report(dest_reg, strict=False)
            with self.assertRaisesRegex(ValueError, "recursive-compute:persistent"):
                m7_benchmark.verify_report(dest_reg, strict=True)

            # B1(a)-(d) & general tamper controls
            for scenario in (
                "sealed-file",
                "missing-snapshot",
                "missing-sampling",
                "missing-js-sampling",
                "artifact-snapshot-drift",
                "stale-wasm-binding",
                "forged-verdict",
                "forged-server-median",
                "forged-js-result",
                "forged-m0-java-baseline",
                "missing-core-m0-strict",
                "local-jdk",
                "quick-profile",
                "preexisting-bin",
                "unstable-src",
                "symlink",
                "threshold",
            ):
                with self.subTest(scenario=scenario):
                    td = Path(tmp) / f"tamper-{scenario}"
                    rep = _make_full_synthetic_report(m7_benchmark.ROOT, td)
                    if scenario == "sealed-file":
                        (td / "preflight.bin").write_bytes(b"tampered")
                        with self.assertRaisesRegex(ValueError, "evidence_files"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "missing-snapshot":
                        (td / "sources.snapshot.json").unlink()
                        rep["evidence_files"] = m7_benchmark.seal_evidence_files(td)
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "sources.snapshot.json"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "missing-sampling":
                        del rep["sampling"]
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "report sampling"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "missing-js-sampling":
                        del rep["js_sampling"]
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "report js_sampling"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "artifact-snapshot-drift":
                        rep["build"]["artifacts"]["wasm_bg"]["sha256"] = "f" * 64
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "wasm_bg sha256 disagrees"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "stale-wasm-binding":
                        rep["build"]["artifacts"]["wasm_bg"]["source_binding"] = "preexisting-binary"
                        m7_benchmark.write_json(td / "report.json", rep)
                        m7_benchmark.verify_report(td, strict=False)
                        with self.assertRaisesRegex(ValueError, "artifact wasm_bg source_binding"):
                            m7_benchmark.verify_report(td, strict=True)
                    elif scenario == "forged-verdict":
                        rep["results"][0]["vs_live_temurin_21"]["verdict"] = "inconclusive"
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "tampered ratio or verdict"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "forged-server-median":
                        srv = next(r for r in rep["results"] if r["mode"] == "steady-server")
                        srv["rust"]["fork_median_operation_ns"]["median"] = 1.0
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "steady-server fork_median_operation_ns"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "forged-js-result":
                        js_row = next(r for r in rep["results"] if r["mode"] == "wasm-browser")
                        js_p = td / js_row["rust"]["js_result_rel_path"]
                        js_d = json.loads(js_p.read_bytes())
                        js_d["preflight_raw_byte_verified"] = False
                        m7_benchmark.write_json(js_p, js_d)
                        rep["evidence_files"] = m7_benchmark.seal_evidence_files(td)
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "preflight_raw_byte_verified"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "forged-m0-java-baseline":
                        core_row = next(r for r in rep["results"] if r["vs_m0_graalvm_25_0_1"] is not None)
                        core_row["vs_m0_graalvm_25_0_1"]["java_time_ns"] = 999_999_999.0
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "reports/m0/benchmarks.json"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "missing-core-m0-strict":
                        core_row = next(r for r in rep["results"] if r["id"] == "scalar:cold-cli")
                        core_row["vs_m0_graalvm_25_0_1"] = None
                        rep["counts"]["vs_m0_graalvm_25_0_1_compared"] -= 1
                        rep["counts"]["vs_m0_graalvm_25_0_1_verdicts"]["pass"] -= 1
                        m7_benchmark.write_json(td / "report.json", rep)
                        m7_benchmark.verify_report(td, strict=False)
                        with self.assertRaisesRegex(ValueError, "missing required M0 GraalVM"):
                            m7_benchmark.verify_report(td, strict=True)
                    elif scenario == "local-jdk":
                        rep["oracle"]["exact_jdk_lock_match"] = False
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "exact_jdk_lock_match"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "quick-profile":
                        rep["profile"] = "quick"
                        rep["strict_eligible"] = False
                        rep["sampling"] = m7_benchmark.load_contract()["profiles"]["quick"]
                        rep["js_sampling"] = m7_benchmark.load_contract()["js_sampling"]["quick"]
                        # Adjust counts for quick so non-strict passes
                        for r in rep["results"]:
                            if r["mode"] == "cold-cli":
                                r["java"]["samples"] = r["java"]["samples"][:3]
                                r["rust"]["samples"] = r["rust"]["samples"][:3]
                                r["java"]["elapsed_ns"] = m7_benchmark.distribution([s["elapsed_ns"] for s in r["java"]["samples"]])
                                r["java"]["peak_rss_bytes"] = m7_benchmark.distribution([s["peak_rss_bytes"] for s in r["java"]["samples"]])
                                r["rust"]["elapsed_ns"] = m7_benchmark.distribution([s["elapsed_ns"] for s in r["rust"]["samples"]])
                                r["rust"]["peak_rss_bytes"] = m7_benchmark.distribution([s["peak_rss_bytes"] for s in r["rust"]["samples"]])
                            elif r["mode"] in ("persistent", "fresh-evaluator"):
                                r["java"]["forks"] = r["java"]["forks"][:1]
                                r["rust"]["forks"] = r["rust"]["forks"][:1]
                                r["java"]["fork_median_operation_ns"] = m7_benchmark.distribution([f["operation_ns"]["median"] for f in r["java"]["forks"]])
                                r["java"]["fork_peak_rss_bytes"] = m7_benchmark.distribution([f["peak_rss_bytes"] for f in r["java"]["forks"]])
                                r["rust"]["fork_median_operation_ns"] = m7_benchmark.distribution([f["operation_ns"]["median"] for f in r["rust"]["forks"]])
                                r["rust"]["fork_peak_rss_bytes"] = m7_benchmark.distribution([f["peak_rss_bytes"] for f in r["rust"]["forks"]])
                            elif r["mode"] == "steady-server":
                                r["java"]["runs"] = r["java"]["runs"][:1]
                                r["rust"]["runs"] = r["rust"]["runs"][:1]
                                r["java"]["fork_median_operation_ns"] = m7_benchmark.distribution([x["steady_request_ns"]["median"] for x in r["java"]["runs"]])
                                r["java"]["fork_peak_rss_bytes"] = m7_benchmark.distribution([x["peak_rss_bytes"] for x in r["java"]["runs"]])
                                r["rust"]["fork_median_operation_ns"] = m7_benchmark.distribution([x["steady_request_ns"]["median"] for x in r["rust"]["runs"]])
                                r["rust"]["fork_peak_rss_bytes"] = m7_benchmark.distribution([x["peak_rss_bytes"] for x in r["rust"]["runs"]])
                            elif r["mode"] in ("napi", "wasm-node", "wasm-browser"):
                                jp = td / r["rust"]["js_result_rel_path"]
                                jd = json.loads(jp.read_bytes())
                                jd["measurements"] = jd["measurements"][:5]
                                m7_benchmark.write_json(jp, jd)
                                r["rust"]["operation_ns"] = m7_benchmark.distribution([m["ns_per_op"] for m in jd["measurements"]])
                        rep["evidence_files"] = m7_benchmark.seal_evidence_files(td)
                        m7_benchmark.write_json(td / "report.json", rep)
                        m7_benchmark.verify_report(td, strict=False)
                        with self.assertRaisesRegex(ValueError, "not strict_eligible"):
                            m7_benchmark.verify_report(td, strict=True)
                    elif scenario == "preexisting-bin":
                        rep["build"]["source_binding"] = "preexisting-binary"
                        m7_benchmark.write_json(td / "report.json", rep)
                        m7_benchmark.verify_report(td, strict=False)
                        with self.assertRaisesRegex(ValueError, "recorded-build"):
                            m7_benchmark.verify_report(td, strict=True)
                    elif scenario == "unstable-src":
                        s = json.loads((td / "sources.snapshot.json").read_bytes())
                        s["after"] = {**s["before"], "tools/m7_benchmark.py": "0" * 64}
                        s["after_sha256"] = m7_benchmark.digest(
                            json.dumps(s["after"], sort_keys=True, separators=(",", ":")).encode("utf-8")
                        )
                        s["source_stability"] = False
                        m7_benchmark.write_json(td / "sources.snapshot.json", s)
                        rep["sources_after_sha256"] = s["after_sha256"]
                        rep["source_stability"] = False
                        rep["evidence_files"] = m7_benchmark.seal_evidence_files(td)
                        m7_benchmark.write_json(td / "report.json", rep)
                        m7_benchmark.verify_report(td, strict=False)
                        with self.assertRaisesRegex(ValueError, "source_stability"):
                            m7_benchmark.verify_report(td, strict=True)
                    elif scenario == "symlink":
                        (td / "evil_link").symlink_to(td / "preflight.bin")
                        with self.assertRaisesRegex(ValueError, "symlink"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "threshold":
                        rep["target_ratios"]["execution_time_max"] = 2.0
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "target_ratios"):
                            m7_benchmark.verify_report(td, strict=False)


if __name__ == "__main__":
    unittest.main()
