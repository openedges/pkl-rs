"""Measurement contract regressions; these are not Pkl performance results."""
import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace
import sys
import tempfile
import unittest

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import benchmark


class BenchmarkTests(unittest.TestCase):
    def recorded(self, root, stdout=b"42\n", stderr=b"", code=0):
        root.mkdir(parents=True)
        result = {"status": "executed", "exit_code": code, "peak_rss_bytes": 1024, "elapsed_ns": 60_000_000_000}
        for name, data in (("stdout", stdout), ("stderr", stderr)):
            path = root / (name + ".bin")
            path.write_bytes(data)
            result[name] = {"path": str(path), "sha256": benchmark.digest(data), "bytes": len(data)}
        return result

    def series(self, warmup_ns=10_000_000_000, converged=True):
        rows = [{"phase": "initialize", "elapsed_ns": 500},
                {"phase": "first-request", "elapsed_ns": 1000}]
        for phase, count in (("warmup", 8), ("measurement", 5)):
            if phase == "measurement":
                rows.append({"phase": "warmup-result", "elapsed_ns": warmup_ns, "windows": 8, "converged": converged})
            rows.extend({"phase": phase, "index": index, "elapsed_ns": 250_000_000,
                         "iterations": 100, "ns_per_op": 2_500_000} for index in range(count))
        sampling = {"samples": 5, "convergence_windows": 8, "minimum_warmup_seconds": 10,
                    "sample_seconds": 0.25, "maximum_relative_spread": 0.2}
        return rows, sampling

    def report_fixture(self, root):
        """Synthetic evidence for checker corruption tests, never benchmark results."""
        contract = benchmark.load_contract()
        contract["workloads"] = [item for item in contract["workloads"] if item["id"] in {"scalar", "recursive-compute"}]
        contract["sampling"]["samples"] = 5
        sampling = contract["sampling"]
        lock_bytes = (benchmark.ROOT / "oracle.lock.json").read_bytes()
        locked = json.loads(lock_bytes)
        (root / "oracle.lock.json").write_bytes(lock_bytes)
        metadata = {"profile": "performance", "version": locked["version"],
                    "artifact_sha256": locked["artifact"]["sha256"], "artifact": "/fixture/pkl.jar",
                    "lock_sha256": benchmark.digest(lock_bytes), "java": "/fixture/java",
                    "command": ["/fixture/java", "-cp", "/fixture/pkl.jar", "org.pkl.cli.Main"],
                    "jdk": {"distribution": next(iter(locked["performance_jdk"]["distributions"].values())),
                            "properties": locked["performance_jdk"]["properties"],
                            "optimizing_runtime_probe": {"stdout_hex": (locked["performance_jdk"]["truffle_runtime_name"] + "\n").encode().hex(),
                                "stderr_hex": "", "source_sha256": benchmark.digest((benchmark.ROOT / "tools/OracleRuntimeProbe.java").read_bytes())}}}
        benchmark.write_json(root / "oracle.json", metadata)
        benchmark.write_json(root / "contract.json", contract)
        def record(folder, stdout=b"", command=None):
            result = self.recorded(folder, stdout)
            result["command"] = command or []
            result["cwd"] = str(benchmark.ROOT)
            benchmark.write_json(folder / "result.json", result)
            return result
        (root / "inputs/benchmarks").mkdir(parents=True)
        input_bytes = json.dumps(contract).encode()
        (root / "inputs/benchmarks/workloads.json").write_bytes(input_bytes)
        input_rows = [{"path": "benchmarks/workloads.json", "sha256": benchmark.digest(input_bytes), "bytes": len(input_bytes)}]
        for workload in contract["workloads"]:
            relative = "benchmarks/" + workload["module"]
            data = (benchmark.ROOT / relative).read_bytes()
            path = root / "inputs" / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            input_rows.append({"path": relative, "sha256": benchmark.digest(data), "bytes": len(data)})
        (root / "harness").mkdir()
        (root / "harness/OracleRuntimeProbe.java").write_bytes((benchmark.ROOT / "tools/OracleRuntimeProbe.java").read_bytes())
        toolchain = {"javac": "/fixture/javac", "javac_sha256": "fixture"}
        for filename, key in (("Benchmark.java", "java_harness_sha256"), ("benchmark.py", "python_harness_sha256")):
            data = (benchmark.ROOT / ("tools/java/" if filename.endswith(".java") else "tools/") / filename).read_bytes()
            (root / "harness" / filename).write_bytes(data)
            toolchain[key] = benchmark.digest(data)
        (root / "classes").mkdir()
        (root / "classes/Benchmark.class").write_bytes(b"synthetic compiler artifact")
        toolchain["compiled_classes"] = [{"path": "Benchmark.class", "sha256": benchmark.digest(b"synthetic compiler artifact")}]
        record(root / "compile", command=["/fixture/javac", "-cp", "/fixture/pkl.jar", "-d", str(root / "classes"),
                                           str(benchmark.ROOT / "tools/java/Benchmark.java")])
        results = []
        for workload in contract["workloads"]:
            folder = root / workload["id"]
            folder.mkdir()
            module = benchmark.ROOT / "benchmarks" / workload["module"]
            cli = [*metadata["command"], "eval", "--no-cache", "--no-project", "--settings", "pkl:settings", "--color=never", str(module)]
            record(folder / "preflight-cli", b"42\n", cli)
            record(folder / "preflight-cli-replay", b"42\n", cli)
            expected = folder / "expected-api.bin"
            expected.write_bytes(b"42\n")
            (folder / "first-api.bin").write_bytes(b"42\n")
            preflight = [{"phase": "initialize", "elapsed_ns": 1},
                         {"phase": "first-request", "elapsed_ns": 1, "error": False, "bytes": 3}]
            for name in ("preflight-api", "preflight-api-replay"):
                command = benchmark.harness_command(metadata, root / "classes", "verify", module, expected,
                    workload, sampling, folder / "first-api.bin", write_expected=name == "preflight-api")
                record(folder / name, b"\n".join(json.dumps(row).encode() for row in preflight), command)
            cold = [record(folder / "cold-cli" / str(index), b"42\n", cli) for index in range(5)]
            item = {"id": workload["id"], "module": str(module.relative_to(benchmark.ROOT)), "expected_error": False,
                    "expected_api": {"path": str(expected), "sha256": benchmark.digest(b"42\n"), "bytes": 3},
                    "cold_cli": {"samples": cold, "elapsed_ns": benchmark.distribution([60_000_000_000] * 5),
                                 "peak_rss_bytes": benchmark.distribution([1024] * 5)}, "api": {}}
            for mode in ("persistent", "fresh-evaluator"):
                forks = []
                for index in range(2):
                    rows, _ = self.series()
                    first = folder / f"first-{mode}-{index}.bin"
                    first.write_bytes(b"42\n")
                    command = benchmark.harness_command(metadata, root / "classes", mode, module, expected, workload, sampling, first)
                    result = record(folder / mode / str(index), b"\n".join(json.dumps(row).encode() for row in rows), command)
                    forks.append({"capture": result, **benchmark.parse_series(result, sampling, mode)})
                item["api"][mode] = {"forks": forks, "warmup": "converged",
                    "fork_median_operation_ns": benchmark.distribution([2_500_000] * 2),
                    "fork_peak_rss_bytes": benchmark.distribution([1024] * 2)}
            results.append(item)
        trace_workload = next(item for item in contract["workloads"] if item["id"] == "recursive-compute")
        command = benchmark.harness_command(metadata, root / "classes", "persistent",
            benchmark.ROOT / "benchmarks" / trace_workload["module"], root / "recursive-compute/expected-api.bin",
            trace_workload, sampling, root / "trace-first.bin", trace=True)
        trace_line = "[engine] opt done FunctionNode |Src " + (benchmark.ROOT / "benchmarks" / trace_workload["module"]).as_uri() + ":1 0x123"
        trace = record(root / "compilation-trace", (trace_line + "\n").encode(), command)
        (root / "trace-first.bin").write_bytes(b"42\n")
        report = {"schema": 1, "compatibility_passes": 0, "rust_performance_ratios": None,
                  "required_pending": contract["required_pending"], "sampling": sampling, "oracle": metadata,
                  "input_files": input_rows,
                  "toolchain": toolchain, "results": results,
                  "compilation_evidence": {"capture": trace, "completed_fib_compilations": [trace_line], "state": "observed"}}
        benchmark.write_json(root / "report.json", report)
        return report, contract

    def test_per_pid_rss_units_and_missing_rss_fail(self):
        self.assertEqual(benchmark.per_process_rss_bytes(SimpleNamespace(ru_maxrss=20), "Darwin"), 20)
        self.assertEqual(benchmark.per_process_rss_bytes(SimpleNamespace(ru_maxrss=20), "Linux"), 20480)
        for number, system in ((0, "Darwin"), (-1, "Linux"), (20, "Unknown")):
            with self.assertRaises(ValueError):
                benchmark.per_process_rss_bytes(SimpleNamespace(ru_maxrss=number), system)

    def test_corrupt_bytes_and_missing_memory_never_accepted(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            expected = self.recorded(root / "expected")
            changed = self.recorded(root / "changed", stdout=b"42 \n")
            with self.assertRaisesRegex(ValueError, "output bytes differ"):
                benchmark.verify_capture(changed, expected)
            changed["peak_rss_bytes"] = 0
            with self.assertRaisesRegex(ValueError, "missing RSS"):
                benchmark.verify_capture(changed)
            Path(expected["stdout"]["path"]).write_bytes(b"43\n")
            with self.assertRaisesRegex(ValueError, "corrupt captured"):
                benchmark.verify_capture(expected)

    def test_unexpected_error_does_not_become_a_performance_sample(self):
        with tempfile.TemporaryDirectory() as temp:
            result = self.recorded(Path(temp) / "process", stdout=b"", stderr=b"error", code=1)
            with self.assertRaisesRegex(ValueError, "unexpected Java exit"):
                benchmark.verify_capture(result)
            benchmark.verify_capture(result, expected_error=True)

    def test_insufficient_warmup_unknown_mode_and_missing_sample(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for index, scenario in enumerate(("short", "unknown", "missing")):
                rows, sampling = self.series(warmup_ns=1 if scenario == "short" else 10_000_000_000)
                if scenario == "missing":
                    rows.pop()
                capture = self.recorded(root / str(index), stdout=b"\n".join(json.dumps(row).encode() for row in rows))
                with self.assertRaises(ValueError):
                    benchmark.parse_series(capture, sampling, "unknown" if scenario == "unknown" else "persistent")

    def test_nonconvergence_is_explicit_inconclusive(self):
        with tempfile.TemporaryDirectory() as temp:
            rows, sampling = self.series(converged=False)
            capture = self.recorded(Path(temp) / "process", stdout=b"\n".join(json.dumps(row).encode() for row in rows))
            result = benchmark.parse_series(capture, sampling, "persistent")
            self.assertEqual(result["warmup"], "inconclusive")
            self.assertEqual(result["operation_ns"]["count"], 5)

    def test_false_convergence_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            rows, sampling = self.series()
            row = next(row for row in rows if row["phase"] == "warmup")
            row["ns_per_op"] *= 4
            row["iterations"] //= 4
            capture = self.recorded(Path(temp) / "process", stdout=b"\n".join(json.dumps(row).encode() for row in rows))
            with self.assertRaisesRegex(ValueError, "false warmup convergence"):
                benchmark.parse_series(capture, sampling, "fresh-evaluator")

    def test_wait4_capture_raw_bytes_and_timeout(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            result = benchmark.capture([sys.executable, "-c", "import os; os.write(1,b' x \\r\\n\\x00')"],
                                       root, {}, root / "process")
            benchmark.verify_capture(result)
            self.assertEqual(benchmark.raw_channel(result, "stdout"), b" x \r\n\x00")
            self.assertGreater(result["peak_rss_bytes"], 0)
            result = benchmark.capture([sys.executable, "-c", "import time; time.sleep(5)"],
                                       root, {}, root / "timeout", timeout=0.05)
            with self.assertRaisesRegex(ValueError, "unexecuted/timeout"):
                benchmark.verify_capture(result)

    def test_contract_rejects_empty_and_unknown_modes_and_short_warmup(self):
        original = benchmark.load_contract()
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for item in original["workloads"]:
                module = root / "benchmarks" / item["module"]
                module.parent.mkdir(parents=True, exist_ok=True)
                module.write_text("fixture")
            path = root / "benchmarks/workloads.json"
            for change in ("empty", "mode", "warmup"):
                contract = json.loads(json.dumps(original))
                if change == "empty":
                    contract["workloads"] = []
                elif change == "mode":
                    contract["implemented_modes"].append("unknown")
                else:
                    contract["sampling"]["minimum_warmup_seconds"] = 1
                path.write_text(json.dumps(contract))
                with self.assertRaises(ValueError):
                    benchmark.load_contract(root)

    def test_report_rejects_repeated_captures_missing_first_bytes_and_forged_summary(self):
        for scenario in ("repeat-cold", "repeat-fork", "wrong-mode", "missing-first", "forged-summary", "missing-trace", "input-drift"):
            with self.subTest(scenario=scenario), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                report, contract = self.report_fixture(root)
                benchmark.check_report(root, contract)
                item = report["results"][0]
                if scenario == "repeat-cold":
                    item["cold_cli"]["samples"] = [item["cold_cli"]["samples"][0]] * 5
                elif scenario == "repeat-fork":
                    item["api"]["persistent"]["forks"][1] = item["api"]["persistent"]["forks"][0]
                elif scenario == "wrong-mode":
                    item["api"]["fresh-evaluator"] = item["api"]["persistent"]
                elif scenario == "missing-first":
                    (root / item["id"] / "first-persistent-0.bin").unlink()
                elif scenario == "forged-summary":
                    item["cold_cli"]["elapsed_ns"]["median"] = 1
                elif scenario == "missing-trace":
                    Path(report["compilation_evidence"]["capture"]["stdout"]["path"]).unlink()
                else:
                    (root / "inputs/benchmarks/workloads.json").write_bytes(b"drift")
                benchmark.write_json(root / "report.json", report)
                with self.assertRaises((ValueError, OSError)):
                    benchmark.check_report(root, contract)


if __name__ == "__main__":
    unittest.main()
