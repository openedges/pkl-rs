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


def _make_full_synthetic_report(
    root: Path,
    dest: Path,
    *,
    failing_id: str | None = None,
    failing_native_id: str | None = None,
    baseline_mode: str = "both",
) -> dict:
    """Create a complete sealed synthetic M7 benchmark report for verifier tamper tests."""
    dest.mkdir(parents=True, exist_ok=True)
    contract = m7_benchmark.load_contract(root)
    native_lock = m7_benchmark.validate_native_image_lock(root)
    ni_target = sorted(native_lock["distributions"].keys())[0]
    ni_dist = native_lock["distributions"][ni_target]
    sampling = contract["profiles"]["full"]
    js_sampling = contract["js_sampling"]["full"]
    all_ids, core_wid_modes, native_ids = m7_benchmark.expected_workload_sets(contract)
    all_ids_sorted = sorted(all_ids)
    core_ids = {w["id"] for w in contract["core_workloads"]}
    use_jvm = baseline_mode in ("both", "jvm")
    use_native = baseline_mode in ("both", "native-image")

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
    native_verdicts: dict[str, int] = {}
    native_compared = 0

    for wid in all_ids_sorted:
        base_id, mode = wid.split(":", 1)
        if baseline_mode == "native-image" and mode not in m7_benchmark.NATIVE_IMAGE_ELIGIBLE_MODES:
            continue
        is_fail = wid == failing_id
        is_native_fail = wid == failing_native_id
        rust_t = 105_000.0 if is_fail else 30_000.0
        java_t = 100_000.0
        ni_t = 25_000.0 if is_native_fail else 40_000.0
        rust_rss = 900_000_000.0 if mode == "wasm-browser" else 15_000_000.0
        java_rss = 250_000_000.0
        ni_rss = 25_000_000.0

        vs_t = None
        if use_jvm:
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

        vs_ni = None
        if use_native and mode in m7_benchmark.NATIVE_IMAGE_ELIGIBLE_MODES:
            vs_ni = m7_benchmark.evaluate_ratio_verdict(
                rust_t,
                ni_t,
                rust_rss,
                ni_rss,
                correctness_verified=True,
                converged=True,
                baseline_key_prefix="native_image",
            )
            native_compared += 1
            native_verdicts[vs_ni["verdict"]] = native_verdicts.get(vs_ni["verdict"], 0) + 1

        row: dict = {
            "id": wid,
            "workload_id": base_id,
            "family": "core-6" if base_id in core_ids else (mode if mode in ("napi", "wasm-node", "wasm-browser") else "other"),
            "mode": mode,
            "op_mode": "typed-conversion" if "typed" in base_id else "eval-expression",
            "correctness_verified": True,
            "java": None,
            "native_image": None,
            "vs_official_native_image": vs_ni,
            "vs_live_temurin_21": vs_t,
            "vs_m0_graalvm_25_0_1": None,
        }

        if mode == "cold-cli":
            r_samps = [{"elapsed_ns": rust_t, "peak_rss_bytes": rust_rss} for _ in range(sampling["cold_samples"])]
            if use_jvm:
                j_samps = [{"elapsed_ns": java_t, "peak_rss_bytes": java_rss} for _ in range(sampling["cold_samples"])]
                row["java"] = {
                    "elapsed_ns": m7_benchmark.distribution([s["elapsed_ns"] for s in j_samps]),
                    "peak_rss_bytes": m7_benchmark.distribution([s["peak_rss_bytes"] for s in j_samps]),
                    "samples": j_samps,
                }
            if use_native:
                n_samps = [{"elapsed_ns": ni_t, "peak_rss_bytes": ni_rss} for _ in range(sampling["cold_samples"])]
                row["native_image"] = {
                    "elapsed_ns": m7_benchmark.distribution([s["elapsed_ns"] for s in n_samps]),
                    "peak_rss_bytes": m7_benchmark.distribution([s["peak_rss_bytes"] for s in n_samps]),
                    "samples": n_samps,
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
            row["native_image_not_applicable_reason"] = m7_benchmark.NATIVE_IMAGE_API_NA_REASON
            row["rust"] = {
                "warmup": "converged",
                "fork_median_operation_ns": m7_benchmark.distribution([f["operation_ns"]["median"] for f in r_forks]),
                "fork_peak_rss_bytes": m7_benchmark.distribution([f["peak_rss_bytes"] for f in r_forks]),
                "forks": r_forks,
            }
        elif mode == "steady-server":
            r_runs = [
                {"steady_request_ns": m7_benchmark.distribution([rust_t]), "peak_rss_bytes": rust_rss}
                for _ in range(sampling["independent_api_forks"])
            ]
            if use_jvm:
                j_runs = [
                    {"steady_request_ns": m7_benchmark.distribution([java_t]), "peak_rss_bytes": java_rss}
                    for _ in range(sampling["independent_api_forks"])
                ]
                row["java"] = {
                    "fork_median_operation_ns": m7_benchmark.distribution([r["steady_request_ns"]["median"] for r in j_runs]),
                    "fork_peak_rss_bytes": m7_benchmark.distribution([r["peak_rss_bytes"] for r in j_runs]),
                    "runs": j_runs,
                }
            if use_native:
                n_runs = [
                    {"steady_request_ns": m7_benchmark.distribution([ni_t]), "peak_rss_bytes": ni_rss}
                    for _ in range(sampling["independent_api_forks"])
                ]
                row["native_image"] = {
                    "fork_median_operation_ns": m7_benchmark.distribution([r["steady_request_ns"]["median"] for r in n_runs]),
                    "fork_peak_rss_bytes": m7_benchmark.distribution([r["peak_rss_bytes"] for r in n_runs]),
                    "runs": n_runs,
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
            row["native_image_not_applicable_reason"] = m7_benchmark.NATIVE_IMAGE_API_NA_REASON
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

        if wid in core_wid_modes:
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
    temurin_compared = 0
    for r in results:
        if r["vs_live_temurin_21"] is not None:
            temurin_compared += 1
            v = r["vs_live_temurin_21"]["verdict"]
            verdict_counts[v] = verdict_counts.get(v, 0) + 1

    ni_oracle_doc = (
        {
            "version": "0.32.0",
            "target": ni_target,
            "exact_native_image_lock_match": True,
            "pkl": {"sha256": ni_dist["pkl"]["sha256"], "bytes": ni_dist["pkl"]["bytes"]},
            "pkldoc": {"sha256": ni_dist["pkldoc"]["sha256"], "bytes": ni_dist["pkldoc"]["bytes"]},
        }
        if use_native
        else None
    )

    report = {
        "schema_version": 1,
        "baseline": "0.32.0",
        "baseline_mode": baseline_mode,
        "profile": "full",
        "strict_eligible": baseline_mode == "both",
        "complete_catalog": baseline_mode == "both",
        "selected_families": sorted(m7_benchmark.ALL_FAMILIES),
        "target_ratios": contract["target_ratios"],
        "native_image_targets": contract["native_image_targets"],
        "browser_wasm_targets": contract["browser_wasm_targets"],
        "sampling": sampling,
        "js_sampling": js_sampling,
        "contract_sha256": m7_benchmark.file_digest(root / m7_benchmark.WORKLOADS_PATH),
        "oracle": {"version": "0.32.0", "exact_jdk_lock_match": True} if use_jvm else None,
        "native_image_oracle": ni_oracle_doc,
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
            "vs_official_native_image_compared": native_compared,
            "vs_official_native_image_verdicts": dict(sorted(native_verdicts.items())),
            "vs_live_temurin_21_compared": temurin_compared,
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
        all_ids, _, native_ids = m7_benchmark.expected_workload_sets(contract)
        self.assertEqual(len(all_ids), 34)
        self.assertEqual(len(native_ids), 15)

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for rel in ("benchmarks/m7_workloads.json", "reports/m0/benchmarks.json", "oracle.lock.json"):
                dst = root / rel
                dst.parent.mkdir(parents=True, exist_ok=True)
                dst.write_bytes((m7_benchmark.ROOT / rel).read_bytes())
            orig_lock = (m7_benchmark.ROOT / "oracle.lock.json").read_bytes()
            for bad_case in (
                "relax-time",
                "relax-rss",
                "relax-native-time",
                "relax-native-rss",
                "disable-native-required",
                "drop-native-mode",
                "bad-native-count",
                "missing-native-lock",
                "bad-native-lock-sha",
                "quick-strict",
                "relax-browser-time",
                "relax-browser-linear-mem",
                "relax-browser-js-heap",
                "relax-browser-gzip",
                "non-firefox-exception",
            ):
                with self.subTest(bad_case=bad_case):
                    (root / "oracle.lock.json").write_bytes(orig_lock)
                    mut = copy.deepcopy(contract)
                    if bad_case == "relax-time":
                        mut["target_ratios"]["execution_time_max"] = 1.05
                    elif bad_case == "relax-rss":
                        mut["target_ratios"]["peak_rss_max"] = 1.1
                    elif bad_case == "relax-native-time":
                        mut["native_image_targets"]["execution_time_max"] = 1.05
                    elif bad_case == "relax-native-rss":
                        mut["native_image_targets"]["peak_rss_max"] = 1.1
                    elif bad_case == "disable-native-required":
                        mut["native_image_targets"]["required_for_cli_server_pkldoc"] = False
                    elif bad_case == "drop-native-mode":
                        mut["native_image_targets"]["eligible_modes"] = ["cold-cli"]
                    elif bad_case == "bad-native-count":
                        mut["native_image_targets"]["expected_eligible_workload_count"] = 14
                    elif bad_case == "missing-native-lock":
                        bad_lock = json.loads(orig_lock)
                        del bad_lock["native_image"]
                        (root / "oracle.lock.json").write_text(json.dumps(bad_lock), encoding="utf-8")
                    elif bad_case == "bad-native-lock-sha":
                        bad_lock = json.loads(orig_lock)
                        bad_lock["native_image"]["distributions"]["Darwin-arm64"]["pkl"]["sha256"] = "not-a-sha"
                        (root / "oracle.lock.json").write_text(json.dumps(bad_lock), encoding="utf-8")
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

    def test_repl_jline_stderr_classifier_and_strict_rust_stderr(self):
        self.assertTrue(m7_benchmark.is_benign_jline_repl_stderr(b""))
        jline_info = (
            b"Nov 22, 2026 11:59:00 PM org.jline.utils.Log log\n"
            b"INFO: History file contained errors, rewriting with valid entries\n"
        )
        self.assertTrue(m7_benchmark.is_benign_jline_repl_stderr(jline_info))
        jline_warn = (
            b"Nov 22, 2026 3:34:34 PM org.jline.utils.Log log\n"
            b"WARNING: Failed to load history\n"
            b"java.nio.file.FileSystemException: /Users/test/.pkl/repl-history: Operation not permitted\n"
            b"\tat org.jline.reader.impl.history.DefaultHistory.load(DefaultHistory.java:158)\n"
        )
        self.assertTrue(m7_benchmark.is_benign_jline_repl_stderr(jline_warn))
        self.assertFalse(m7_benchmark.is_benign_jline_repl_stderr(b"\xe2\x80\x93\xe2\x80\x93 Pkl Error \xe2\x80\x93\xe2\x80\x93\nboom\n"))
        self.assertFalse(m7_benchmark.is_benign_jline_repl_stderr(b"unexpected stderr line\n"))
        self.assertFalse(m7_benchmark.is_benign_jline_repl_stderr(jline_info + b"extra error\n"))

        # End-to-end check on _run_cold_cli_pair(allow_native_jline_stderr=True):
        # Official native-image JLine stderr is recorded honestly while Rust stderr == b"" stays strictly enforced!
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            env_meta = {"environment": {"PATH": "/usr/bin:/bin"}}
            java_fn = lambda st: [sys.executable, "-c", "import sys; sys.stdout.write('42\\n')"]
            ni_fn = lambda st: [
                sys.executable,
                "-c",
                "import sys; sys.stdout.write('42\\n'); sys.stderr.write('Nov 22, 2026 11:59:00 PM org.jline.utils.Log log\\nINFO: History file contained errors, rewriting with valid entries\\n')",
            ]
            rust_ok_fn = lambda st: [sys.executable, "-c", "import sys; sys.stdout.write('42\\n')"]
            rust_err_fn = lambda st: [
                sys.executable,
                "-c",
                "import sys; sys.stdout.write('42\\n'); sys.stderr.write('INFO: History file contained errors\\n')",
            ]

            row = m7_benchmark._run_cold_cli_pair(
                "repl-session",
                "repl",
                root / "ok",
                java_fn,
                rust_ok_fn,
                env_meta,
                root,
                cold_samples=1,
                native_oracle_meta=env_meta,
                native_cmd_fn=ni_fn,
                allow_native_jline_stderr=True,
            )
            self.assertEqual(row["native_image"]["stderr_policy"], "jline-history-log-recorded")
            self.assertFalse(row["native_image"]["raw_stderr_bytes_match"])
            self.assertGreater(row["native_image"]["preflight_stderr_bytes"], 0)
            self.assertEqual(row["expected_stderr_sha256"], m7_benchmark.digest(b""))

            # Rust emitting ANY stderr MUST still be rejected!
            with self.assertRaisesRegex(ValueError, "raw stderr bytes differ"):
                m7_benchmark._run_cold_cli_pair(
                    "repl-session",
                    "repl",
                    root / "rust-bad",
                    java_fn,
                    rust_err_fn,
                    env_meta,
                    root,
                    cold_samples=1,
                    native_oracle_meta=env_meta,
                    native_cmd_fn=ni_fn,
                    allow_native_jline_stderr=True,
                )

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

            # Single workload regression (> 1.00x) vs JVM passes --verify-report but FAILS --verify-strict
            dest_reg = Path(tmp) / "rep-reg"
            _make_full_synthetic_report(m7_benchmark.ROOT, dest_reg, failing_id="recursive-compute:persistent")
            m7_benchmark.verify_report(dest_reg, strict=False)
            with self.assertRaisesRegex(ValueError, "recursive-compute:persistent"):
                m7_benchmark.verify_report(dest_reg, strict=True)

            # Single CLI/server/pkldoc workload regression (> 1.00x) vs Official Native Image
            # passes --verify-report (even while beating Temurin 21 JVM!) but MUST FAIL --verify-strict!
            for ni_reg_id in ("scalar:cold-cli", "server-session:steady-server", "pkldoc-single-package:cold-cli"):
                dest_ni_reg = Path(tmp) / f"rep-ni-reg-{ni_reg_id.replace(':', '-')}"
                _make_full_synthetic_report(m7_benchmark.ROOT, dest_ni_reg, failing_native_id=ni_reg_id)
                m7_benchmark.verify_report(dest_ni_reg, strict=False)
                with self.assertRaisesRegex(ValueError, f"{ni_reg_id}: vs_official_native_image=fail"):
                    m7_benchmark.verify_report(dest_ni_reg, strict=True)

            # Partial baseline modes ("jvm" or "native-image") pass --verify-report & --summary,
            # but MUST FAIL --verify-strict!
            for partial_mode in ("jvm", "native-image"):
                dest_partial = Path(tmp) / f"rep-partial-{partial_mode}"
                rep_partial = _make_full_synthetic_report(m7_benchmark.ROOT, dest_partial, baseline_mode=partial_mode)
                m7_benchmark.verify_report(dest_partial, strict=False)
                summary_md = m7_benchmark.format_markdown_summary(rep_partial)
                self.assertIn(f"baseline={partial_mode}", summary_md)
                with self.assertRaisesRegex(ValueError, "baseline_mode="):
                    m7_benchmark.verify_report(dest_partial, strict=True)

            # B1(a)-(d) & native-image & general tamper controls
            for scenario in (
                "sealed-file",
                "missing-snapshot",
                "missing-sampling",
                "missing-js-sampling",
                "artifact-snapshot-drift",
                "stale-wasm-binding",
                "forged-verdict",
                "forged-native-verdict",
                "forged-native-median",
                "native-lock-false",
                "native-sha-mismatch",
                "native-pkldoc-bytes-mismatch",
                "native-threshold-relaxed",
                "missing-native-on-cli",
                "fake-native-on-api",
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
                    elif scenario == "forged-native-verdict":
                        cli_row = next(r for r in rep["results"] if r["id"] == "scalar:cold-cli")
                        cli_row["vs_official_native_image"]["verdict"] = "inconclusive"
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "tampered ratio or verdict in vs_official_native_image"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "forged-native-median":
                        cli_row = next(r for r in rep["results"] if r["id"] == "scalar:cold-cli")
                        cli_row["native_image"]["elapsed_ns"]["median"] = 999_999.0
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "native_image cold-cli elapsed_ns"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "native-lock-false":
                        rep["native_image_oracle"]["exact_native_image_lock_match"] = False
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "exact_native_image_lock_match"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "native-sha-mismatch":
                        rep["native_image_oracle"]["pkl"]["sha256"] = "0" * 64
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "native_image_oracle.pkl SHA-256"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "native-pkldoc-bytes-mismatch":
                        rep["native_image_oracle"]["pkldoc"]["bytes"] = 123
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "native_image_oracle.pkldoc SHA-256"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "native-threshold-relaxed":
                        rep["native_image_targets"]["execution_time_max"] = 1.25
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "native_image_targets"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "missing-native-on-cli":
                        cli_row = next(r for r in rep["results"] if r["id"] == "scalar:cold-cli")
                        cli_row["vs_official_native_image"] = None
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "missing required vs_official_native_image"):
                            m7_benchmark.verify_report(td, strict=False)
                    elif scenario == "fake-native-on-api":
                        api_row = next(r for r in rep["results"] if r["id"] == "scalar:persistent")
                        api_row["vs_official_native_image"] = {"verdict": "pass"}
                        m7_benchmark.write_json(td / "report.json", rep)
                        with self.assertRaisesRegex(ValueError, "must not carry vs_official_native_image"):
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
                                for side in ("java", "native_image", "rust"):
                                    r[side]["samples"] = r[side]["samples"][:3]
                                    r[side]["elapsed_ns"] = m7_benchmark.distribution([s["elapsed_ns"] for s in r[side]["samples"]])
                                    r[side]["peak_rss_bytes"] = m7_benchmark.distribution([s["peak_rss_bytes"] for s in r[side]["samples"]])
                            elif r["mode"] in ("persistent", "fresh-evaluator"):
                                for side in ("java", "rust"):
                                    r[side]["forks"] = r[side]["forks"][:1]
                                    r[side]["fork_median_operation_ns"] = m7_benchmark.distribution([f["operation_ns"]["median"] for f in r[side]["forks"]])
                                    r[side]["fork_peak_rss_bytes"] = m7_benchmark.distribution([f["peak_rss_bytes"] for f in r[side]["forks"]])
                            elif r["mode"] == "steady-server":
                                for side in ("java", "native_image", "rust"):
                                    r[side]["runs"] = r[side]["runs"][:1]
                                    r[side]["fork_median_operation_ns"] = m7_benchmark.distribution([x["steady_request_ns"]["median"] for x in r[side]["runs"]])
                                    r[side]["fork_peak_rss_bytes"] = m7_benchmark.distribution([x["peak_rss_bytes"] for x in r[side]["runs"]])
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
