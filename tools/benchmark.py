#!/usr/bin/env python3
"""Record an initial optimizing-Java baseline; never reports Rust performance parity."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import signal
import statistics
import subprocess
import sys
import threading
import time

import oracle

ROOT = Path(__file__).resolve().parents[1]
MODES = {"cold-cli", "persistent", "fresh-evaluator"}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def load_contract(root=ROOT):
    path = root / "benchmarks/workloads.json"
    contract = json.loads(path.read_bytes())
    if contract.get("schema") != 1 or contract.get("baseline") != "0.32.0":
        raise ValueError("unknown benchmark contract/baseline")
    workloads = contract.get("workloads", [])
    ids = [item["id"] for item in workloads]
    if not workloads or len(ids) != len(set(ids)):
        raise ValueError("empty or duplicate workload inventory")
    if set(contract.get("implemented_modes", [])) != MODES:
        raise ValueError("unknown/missing benchmark mode")
    sampling = contract["sampling"]
    for key in ("cold_samples", "samples", "independent_api_forks", "convergence_windows"):
        if type(sampling[key]) is not int:
            raise ValueError("invalid benchmark sampling count")
    numeric = [sampling[key] for key in ("sample_seconds", "minimum_warmup_seconds",
               "maximum_warmup_seconds", "maximum_relative_spread")]
    if not all(isinstance(value, (int, float)) and math.isfinite(value) for value in numeric):
        raise ValueError("invalid benchmark sampling number")
    if (sampling["cold_samples"] < 5 or sampling["samples"] < 5
            or sampling["independent_api_forks"] < 2 or sampling["sample_seconds"] < 0.1
            or sampling["minimum_warmup_seconds"] < 10
            or sampling["maximum_warmup_seconds"] < sampling["minimum_warmup_seconds"]
            or sampling["convergence_windows"] < 4 or sampling["convergence_windows"] % 2
            or not 0 < sampling["maximum_relative_spread"] <= 0.5):
        raise ValueError("insufficient warmup/sampling contract")
    for item in workloads:
        module = (root / "benchmarks" / item["module"]).resolve()
        if not module.is_relative_to((root / "benchmarks/fixtures").resolve()) or not module.is_file():
            raise ValueError("missing or escaping workload module")
        if not isinstance(item.get("expected_error"), bool):
            raise ValueError("missing explicit workload outcome")
    return contract


def per_process_rss_bytes(usage, system=None):
    system = system or platform.system()
    value = usage.ru_maxrss
    if system not in {"Darwin", "Linux"} or not isinstance(value, (int, float)) or value <= 0:
        raise ValueError("missing/unsupported per-process peak RSS")
    return int(value if system == "Darwin" else value * 1024)


def capture(command, cwd, environment, destination, timeout=180):
    """Wait for this exact PID, collecting its own wait4 rusage (never cumulative)."""
    if not hasattr(os, "wait4"):
        raise ValueError("per-process wait4 peak RSS unavailable on this platform")
    destination.mkdir(parents=True, exist_ok=False)
    stdout_path, stderr_path = destination / "stdout.bin", destination / "stderr.bin"
    timed_out = threading.Event()
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        started = time.perf_counter_ns()
        process = subprocess.Popen(command, cwd=cwd, env=environment, stdin=subprocess.DEVNULL,
                                   stdout=stdout, stderr=stderr, start_new_session=True)

        def expire():
            try:
                os.killpg(process.pid, signal.SIGKILL)
                timed_out.set()
            except ProcessLookupError:
                pass

        timer = threading.Timer(timeout, expire)
        timer.daemon = True
        timer.start()
        try:
            _, status, usage = os.wait4(process.pid, 0)
            elapsed = time.perf_counter_ns() - started
            process.returncode = os.waitstatus_to_exitcode(status)
        finally:
            timer.cancel()
    result = {"command": [str(arg) for arg in command], "cwd": str(cwd),
              "status": "timeout" if timed_out.is_set() else "executed",
              "exit_code": process.returncode, "elapsed_ns": elapsed,
              "peak_rss_bytes": per_process_rss_bytes(usage),
              "user_cpu_seconds": usage.ru_utime, "system_cpu_seconds": usage.ru_stime,
              "stdout": {"path": str(stdout_path), "sha256": digest(stdout_path.read_bytes()),
                         "bytes": stdout_path.stat().st_size},
              "stderr": {"path": str(stderr_path), "sha256": digest(stderr_path.read_bytes()),
                         "bytes": stderr_path.stat().st_size}}
    write_json(destination / "result.json", result)
    return result


def raw_channel(result, channel):
    value = Path(result[channel]["path"]).read_bytes()
    if digest(value) != result[channel]["sha256"] or len(value) != result[channel]["bytes"]:
        raise ValueError(f"corrupt captured {channel}")
    return value


def verify_capture(result, expected=None, expected_error=False, helper=False):
    if result.get("status") != "executed" or result.get("peak_rss_bytes", 0) <= 0:
        raise ValueError("unexecuted/timeout or missing RSS sample")
    if not isinstance(result.get("elapsed_ns"), (int, float)) or result["elapsed_ns"] <= 0:
        raise ValueError("missing/invalid process elapsed time")
    expected_exit = 0 if helper or not expected_error else 1
    if result["exit_code"] != expected_exit:
        raise ValueError(f"unexpected Java exit: {result['exit_code']}; see {result['stderr']['path']}")
    stdout, stderr = raw_channel(result, "stdout"), raw_channel(result, "stderr")
    if helper and stderr:
        raise ValueError("unexpected harness stderr; compilation trace must run separately")
    if not helper and ((expected_error and (stdout or not stderr)) or (not expected_error and stderr)):
        raise ValueError("unexpected Java output channel/outcome")
    if expected and (stdout, stderr, result["exit_code"]) != (
            raw_channel(expected, "stdout"), raw_channel(expected, "stderr"), expected["exit_code"]):
        raise ValueError("output bytes differ from Java preflight; sample rejected")


def distribution(values):
    if not values or any(not math.isfinite(value) or value <= 0 for value in values):
        raise ValueError("missing/invalid raw timing samples")
    ordered = sorted(values)
    percentile = lambda p: ordered[max(0, math.ceil(len(ordered) * p) - 1)]
    return {"count": len(values), "min": ordered[0], "median": statistics.median(values),
            "p90": percentile(0.90), "p95": percentile(0.95), "max": ordered[-1],
            "mean": statistics.mean(values), "sample_stdev": statistics.stdev(values) if len(values) > 1 else 0,
            "uncertainty": "Empirical sample distribution only; no significance or target-ratio claim"}


def parse_series(result, sampling, mode):
    if mode not in {"persistent", "fresh-evaluator"}:
        raise ValueError("unknown API benchmark mode")
    verify_capture(result, helper=True)
    rows = [json.loads(line) for line in raw_channel(result, "stdout").splitlines()]
    def phase(name):
        return [row for row in rows if row.get("phase") == name]
    if len(phase("initialize")) != 1 or len(phase("first-request")) != 1 or len(phase("warmup-result")) != 1:
        raise ValueError("missing/duplicate API timing phase")
    warmup, measurements = phase("warmup"), phase("measurement")
    summary = phase("warmup-result")[0]
    phases = [row.get("phase") for row in rows]
    if phases != (["initialize", "first-request"] + ["warmup"] * len(warmup)
                  + ["warmup-result"] + ["measurement"] * len(measurements)):
        raise ValueError("unknown or out-of-order timing phase")
    if (len(measurements) != sampling["samples"] or len(warmup) < sampling["convergence_windows"]
            or summary["windows"] != len(warmup)
            or summary["elapsed_ns"] < sampling["minimum_warmup_seconds"] * 1e9):
        raise ValueError("insufficient warmup or missing measured samples")
    for group in (warmup, measurements):
        if [row.get("index") for row in group] != list(range(len(group))):
            raise ValueError("missing/duplicate timing window")
        for row in group:
            if (row.get("iterations", 0) <= 0 or row.get("elapsed_ns", 0) < sampling["sample_seconds"] * 1e9
                    or not math.isclose(row["ns_per_op"], row["elapsed_ns"] / row["iterations"], rel_tol=1e-9)):
                raise ValueError("invalid timing window")
    minimum_process_time = (phase("initialize")[0]["elapsed_ns"] + phase("first-request")[0]["elapsed_ns"]
                            + summary["elapsed_ns"] + sum(row["elapsed_ns"] for row in measurements))
    if result["elapsed_ns"] < minimum_process_time:
        raise ValueError("API phases exceed whole-process elapsed time")
    recent = sorted(row["ns_per_op"] for row in warmup[-sampling["convergence_windows"]:])
    spread = (recent[-1] - recent[0]) / statistics.median(recent)
    if summary["converged"] and spread > sampling["maximum_relative_spread"]:
        raise ValueError("false warmup convergence")
    return {"rows": rows, "warmup": "converged" if summary["converged"] else "inconclusive",
            "recent_relative_spread": spread,
            "operation_ns": distribution([row["ns_per_op"] for row in measurements]),
            "initialize_ns": phase("initialize")[0]["elapsed_ns"],
            "first_request_ns": phase("first-request")[0]["elapsed_ns"],
            "memory_boundary": "Whole JVM fork including initialization, correctness validation, warmup and samples",
            "peak_rss_bytes": result["peak_rss_bytes"]}


def machine():
    value = {"platform": platform.platform(), "system": platform.system(), "machine": platform.machine(),
             "python": sys.version, "processor": platform.processor(), "cpu_count": os.cpu_count(),
             "load_average": list(os.getloadavg()) if hasattr(os, "getloadavg") else None}
    if platform.system() == "Darwin":
        process = subprocess.run(["/usr/sbin/sysctl", "-n", "machdep.cpu.brand_string", "hw.memsize"],
                                 capture_output=True, check=True)
        value["hardware_raw"] = process.stdout.decode()
        for name, command in (("power_state", ["/usr/bin/pmset", "-g", "batt"]),
                              ("thermal_state", ["/usr/bin/pmset", "-g", "therm"])):
            state = subprocess.run(command, capture_output=True, check=False)
            value[name] = {"exit_code": state.returncode, "stdout": state.stdout.decode(errors="replace"),
                           "stderr": state.stderr.decode(errors="replace")}
    elif Path("/proc/meminfo").is_file():
        value["memory_raw"] = Path("/proc/meminfo").read_text()
    return value


def input_census(root):
    return [{"path": str(path.relative_to(root)), "sha256": digest(path.read_bytes()),
             "bytes": path.stat().st_size}
            for path in sorted((root / "benchmarks").rglob("*")) if path.is_file()]


def completed_compilations(raw, module):
    # Truffle prints FunctionNode identities, not Pkl function names. The pinned
    # fixture's line 1 declares fib; source URI + line identifies that target.
    location = ("Src " + module.as_uri() + ":1").encode()
    return [line.decode("utf-8", errors="replace") for line in raw.splitlines()
            if re.search(rb"\bopt\s+done\b", line) and location in line]


def check_report(directory, contract=None):
    """Revalidate raw evidence, all required initial cases, and timing contracts."""
    report = json.loads((directory / "report.json").read_bytes())
    contract = contract or load_contract()
    if (report.get("schema") != 1 or report.get("rust_performance_ratios") is not None
            or report.get("compatibility_passes") != 0
            or report.get("required_pending") != contract["required_pending"]):
        raise ValueError("unknown report schema or unsupported Rust performance claim")
    results = report.get("results", [])
    expected = {item["id"]: item for item in contract["workloads"]}
    if len(results) != len(expected) or {item["id"] for item in results} != set(expected):
        raise ValueError("missing/duplicate benchmark workload results")
    sampling = report["sampling"]
    if sampling != contract["sampling"]:
        raise ValueError("benchmark sampling contract changed")
    locked = json.loads((directory / "oracle.lock.json").read_bytes())
    metadata = report["oracle"]
    if json.loads((directory / "oracle.json").read_bytes()) != metadata:
        raise ValueError("oracle provenance differs from captured metadata")
    if (json.loads((directory / "contract.json").read_bytes()) != contract
            or json.loads((directory / "inputs/benchmarks/workloads.json").read_bytes()) != contract):
        raise ValueError("benchmark contract snapshot differs")
    if (metadata.get("profile") != "performance" or metadata["version"] != locked["version"]
            or metadata["artifact_sha256"] != locked["artifact"]["sha256"]
            or metadata["lock_sha256"] != digest((directory / "oracle.lock.json").read_bytes())
            or metadata["jdk"]["distribution"] not in locked["performance_jdk"]["distributions"].values()):
        raise ValueError("benchmark optimizing oracle identity differs from locked profile")
    if any(metadata["jdk"]["properties"].get(key) != value
           for key, value in locked["performance_jdk"]["properties"].items()):
        raise ValueError("optimizing JVM runtime properties differ")
    probe = metadata["jdk"]["optimizing_runtime_probe"]
    if (probe["stdout_hex"] != (locked["performance_jdk"]["truffle_runtime_name"] + "\n").encode().hex()
            or probe["stderr_hex"] != ""
            or probe["source_sha256"] != digest((directory / "harness/OracleRuntimeProbe.java").read_bytes())):
        raise ValueError("optimizing runtime availability probe differs")
    for row in report["input_files"]:
        path = Path(row["path"])
        if path.is_absolute() or ".." in path.parts:
            raise ValueError("unsafe input snapshot path")
        data = (directory / "inputs" / path).read_bytes()
        if digest(data) != row["sha256"] or len(data) != row["bytes"]:
            raise ValueError("benchmark input snapshot differs")
    if not report["input_files"]:
        raise ValueError("empty benchmark input snapshot")
    required_inputs = {"benchmarks/workloads.json"}
    for workload in contract["workloads"]:
        required_inputs.add("benchmarks/" + workload["module"])
        required_inputs.update("benchmarks/" + path for path in workload.get("resources", []))
    if not required_inputs.issubset({row["path"] for row in report["input_files"]}):
        raise ValueError("missing workload module/resource snapshot")
    snapshot_paths = sorted(str(path.relative_to(directory / "inputs"))
                            for path in (directory / "inputs").rglob("*") if path.is_file())
    if snapshot_paths != sorted(row["path"] for row in report["input_files"]):
        raise ValueError("omitted/duplicate benchmark input snapshot")
    for filename, hash_key in (("Benchmark.java", "java_harness_sha256"), ("benchmark.py", "python_harness_sha256")):
        if digest((directory / "harness" / filename).read_bytes()) != report["toolchain"][hash_key]:
            raise ValueError("benchmark helper snapshot differs")
    classes = [{"path": str(path.relative_to(directory / "classes")), "sha256": digest(path.read_bytes())}
               for path in sorted((directory / "classes").rglob("*.class"))]
    if not classes or classes != report["toolchain"]["compiled_classes"]:
        raise ValueError("compiled benchmark helper differs")
    compile_result = json.loads((directory / "compile/result.json").read_bytes())
    verify_capture(compile_result, helper=True)
    if raw_channel(compile_result, "stdout"):
        raise ValueError("unexpected compiler output")
    def captured_at(result, folder):
        for channel in ("stdout", "stderr"):
            if Path(result[channel]["path"]) != folder / (channel + ".bin"):
                raise ValueError("duplicate or relocated process capture identity")
        if json.loads((folder / "result.json").read_bytes()) != result:
            raise ValueError("process capture metadata differs from raw process record")

    captured_at(compile_result, directory / "compile")
    expected_compile = [report["toolchain"]["javac"], "-cp", metadata["artifact"], "-d",
                        str(directory / "classes"), str(ROOT / "tools/java/Benchmark.java")]
    if compile_result["command"] != expected_compile:
        raise ValueError("benchmark helper compile command differs")

    for item in results:
        if item["expected_error"] != expected[item["id"]]["expected_error"]:
            raise ValueError("benchmark expected outcome changed")
        baseline_dir = directory / item["id"]
        reference = json.loads((baseline_dir / "preflight-cli/result.json").read_bytes())
        captured_at(reference, baseline_dir / "preflight-cli")
        verify_capture(reference, expected_error=item["expected_error"])
        module = ROOT / item["module"]
        expected_command = [*metadata["command"], "eval", "--no-cache", "--no-project", "--settings", "pkl:settings",
                            "--color=never", str(module)]
        if reference["command"] != expected_command:
            raise ValueError("CLI preflight workload command differs")
        replay_folder = baseline_dir / "preflight-cli-replay"
        replay = json.loads((replay_folder / "result.json").read_bytes())
        captured_at(replay, replay_folder)
        verify_capture(replay, reference, item["expected_error"])
        if replay["command"] != expected_command:
            raise ValueError("CLI replay workload command differs")
        if len(item["cold_cli"]["samples"]) != sampling["cold_samples"]:
            raise ValueError("missing cold CLI sample")
        for index, row in enumerate(item["cold_cli"]["samples"]):
            captured_at(row, baseline_dir / "cold-cli" / str(index))
            if row["command"] != reference["command"]:
                raise ValueError("cold CLI sample command changed")
            verify_capture(row, reference, item["expected_error"])
        for field, capture_field in (("elapsed_ns", "elapsed_ns"), ("peak_rss_bytes", "peak_rss_bytes")):
            if item["cold_cli"][field] != distribution([row[capture_field] for row in item["cold_cli"]["samples"]]):
                raise ValueError("cold CLI summary differs from raw samples")
        api = item["expected_api"]
        api_bytes = Path(api["path"]).read_bytes()
        if digest(api_bytes) != api["sha256"] or len(api_bytes) != api["bytes"]:
            raise ValueError("corrupt API expected bytes")
        if (baseline_dir / "first-api.bin").read_bytes() != api_bytes:
            raise ValueError("API preflight bytes differ")
        if not item["expected_error"] and api_bytes != raw_channel(reference, "stdout"):
            raise ValueError("CLI/API preflight success bytes disagree")
        for name in ("preflight-api", "preflight-api-replay"):
            folder = baseline_dir / name
            capture = json.loads((folder / "result.json").read_bytes())
            captured_at(capture, folder)
            verify_capture(capture, helper=True)
            expected_api_command = harness_command(metadata, directory / "classes", "verify", module,
                Path(api["path"]), expected[item["id"]], sampling, baseline_dir / "first-api.bin",
                write_expected=name == "preflight-api")
            if capture["command"] != expected_api_command:
                raise ValueError("API preflight command mode differs")
            rows = [json.loads(line) for line in raw_channel(capture, "stdout").splitlines()]
            if ([row.get("phase") for row in rows] != ["initialize", "first-request"]
                    or rows[1]["error"] != item["expected_error"] or rows[1]["bytes"] != len(api_bytes)
                    or any(row["elapsed_ns"] <= 0 for row in rows)):
                raise ValueError("API preflight phase/outcome differs")
        if set(item["api"]) != {"persistent", "fresh-evaluator"}:
            raise ValueError("missing/unknown API mode")
        for mode, value in item["api"].items():
            if len(value["forks"]) != sampling["independent_api_forks"]:
                raise ValueError("missing API fork")
            for index, fork in enumerate(value["forks"]):
                captured_at(fork["capture"], baseline_dir / mode / str(index))
                first_file = baseline_dir / f"first-{mode}-{index}.bin"
                expected_api_command = harness_command(metadata, directory / "classes", mode, module,
                    Path(api["path"]), expected[item["id"]], sampling, first_file)
                if fork["capture"]["command"] != expected_api_command:
                    raise ValueError("API fork mode/workload command differs")
                if first_file.read_bytes() != api_bytes:
                    raise ValueError("first request raw bytes differ")
                parsed = parse_series(fork["capture"], sampling, mode)
                if any(parsed[key] != fork[key] for key in parsed):
                    raise ValueError("API summary differs from raw timing evidence")
            convergence = "converged" if all(fork["warmup"] == "converged" for fork in value["forks"]) else "inconclusive"
            if (value["warmup"] != convergence or value["fork_median_operation_ns"] != distribution(
                    [fork["operation_ns"]["median"] for fork in value["forks"]])
                    or value["fork_peak_rss_bytes"] != distribution([fork["peak_rss_bytes"] for fork in value["forks"]])):
                raise ValueError("API aggregate summary differs from raw forks")
    trace = report["compilation_evidence"]
    captured_at(trace["capture"], directory / "compilation-trace")
    process = trace["capture"]
    if process["status"] != "executed" or process["exit_code"] != 0 or process["peak_rss_bytes"] <= 0:
        raise ValueError("failed compilation trace process")
    trace_workload = expected["recursive-compute"]
    trace_command = harness_command(metadata, directory / "classes", "persistent",
        ROOT / "benchmarks" / trace_workload["module"], directory / "recursive-compute/expected-api.bin",
        trace_workload, sampling, directory / "trace-first.bin", trace=True)
    if process["command"] != trace_command:
        raise ValueError("compilation trace command differs")
    trace_bytes = raw_channel(process, "stdout") + raw_channel(process, "stderr")
    actual_lines = completed_compilations(trace_bytes, ROOT / "benchmarks" / trace_workload["module"])
    if (trace["completed_fib_compilations"] != actual_lines
            or trace["state"] != ("observed" if actual_lines else "not-observed")):
        raise ValueError("compilation proof differs from actual trace bytes")
    if (directory / "trace-first.bin").read_bytes() != (directory / "recursive-compute/expected-api.bin").read_bytes():
        raise ValueError("compilation trace first bytes differ")
    return report


def harness_command(metadata, classes, mode, module, expected, workload, sampling, first, write_expected=False, trace=False):
    prefix = metadata["command"]
    # Retain the exact pinned Java/JVM options while replacing the CLI classpath/main.
    cp = prefix.index("-cp")
    extra = ["-Dpolyglot.engine.TraceCompilation=true"] if trace else []
    return [*prefix[:cp], *extra, "-cp", os.pathsep.join((str(classes), metadata["artifact"])), "Benchmark",
            mode, str(module), str(expected), str(workload["expected_error"]).lower(),
            str(sampling["minimum_warmup_seconds"]), str(sampling["maximum_warmup_seconds"]),
            str(sampling["samples"]), str(sampling["sample_seconds"]), str(sampling["convergence_windows"]),
            str(sampling["maximum_relative_spread"]), str(first), str(write_expected).lower()]


def run(root=ROOT, output=None):
    root = root.resolve()
    contract = load_contract(root)
    sampling = contract["sampling"]
    destination = (output or root / "target/m0/benchmarks").resolve()
    if destination.exists():
        raise ValueError(f"refusing to overwrite benchmark evidence: {destination}")
    metadata = oracle.ensure_oracle(root, profile="performance")
    destination.mkdir(parents=True)
    write_json(destination / "oracle.json", metadata)
    write_json(destination / "contract.json", contract)
    before_inputs = input_census(root)
    for row in before_inputs:
        snapshot = destination / "inputs" / row["path"]
        snapshot.parent.mkdir(parents=True, exist_ok=True)
        snapshot.write_bytes((root / row["path"]).read_bytes())
    (destination / "harness").mkdir()
    for path in (root / "tools/java/Benchmark.java", root / "tools/benchmark.py", root / "tools/OracleRuntimeProbe.java"):
        (destination / "harness" / path.name).write_bytes(path.read_bytes())
    (destination / "oracle.lock.json").write_bytes((root / "oracle.lock.json").read_bytes())
    before_tools = {path: digest((root / path).read_bytes()) for path in (
        "tools/java/Benchmark.java", "tools/benchmark.py")}
    started = machine()
    source = root / "tools/java/Benchmark.java"
    classes = destination / "classes"
    classes.mkdir()
    javac = Path(metadata["java"]).with_name("javac")
    compiled = capture([str(javac), "-cp", metadata["artifact"], "-d", str(classes), str(source)],
                       root, metadata["environment"], destination / "compile")
    verify_capture(compiled, helper=True)
    if raw_channel(compiled, "stdout"):
        raise ValueError("unexpected javac output")
    results = []
    for workload in contract["workloads"]:
        name = workload["id"]
        folder = destination / name
        folder.mkdir()
        module = (root / "benchmarks" / workload["module"]).resolve()
        cli = [*metadata["command"], "eval", "--no-cache", "--no-project", "--settings", "pkl:settings",
               "--color=never", str(module)]
        expected_cli = capture(cli, root, metadata["environment"], folder / "preflight-cli")
        verify_capture(expected_cli, expected_error=workload["expected_error"])
        replay = capture(cli, root, metadata["environment"], folder / "preflight-cli-replay")
        verify_capture(replay, expected_cli, workload["expected_error"])
        expected_api = folder / "expected-api.bin"
        verify_command = harness_command(metadata, classes, "verify", module, expected_api, workload,
                                         sampling, folder / "first-api.bin", write_expected=True)
        preflight_api = capture(verify_command, root, metadata["environment"], folder / "preflight-api")
        verify_capture(preflight_api, helper=True)
        api_bytes = expected_api.read_bytes()
        if not workload["expected_error"] and api_bytes != raw_channel(expected_cli, "stdout"):
            raise ValueError(f"CLI/API success bytes disagree: {name}")
        verify_command[-1] = "false"
        preflight_replay = capture(verify_command, root, metadata["environment"], folder / "preflight-api-replay")
        verify_capture(preflight_replay, helper=True)
        cold = []
        for index in range(sampling["cold_samples"]):
            sample = capture(cli, root, metadata["environment"], folder / "cold-cli" / str(index))
            verify_capture(sample, expected_cli, workload["expected_error"])
            cold.append(sample)
        item = {"id": name, "module": str(module.relative_to(root)),
                "expected_error": workload["expected_error"], "preflight": "raw-byte replay verified",
                "expected_api": {"path": str(expected_api), "sha256": digest(api_bytes), "bytes": len(api_bytes)},
                "cold_cli": {"samples": cold, "elapsed_ns": distribution([row["elapsed_ns"] for row in cold]),
                             "peak_rss_bytes": distribution([row["peak_rss_bytes"] for row in cold])},
                "api": {}}
        for mode in ("persistent", "fresh-evaluator"):
            forks = []
            for index in range(sampling["independent_api_forks"]):
                fork_folder = folder / mode / str(index)
                first = folder / f"first-{mode}-{index}.bin"
                command = harness_command(metadata, classes, mode, module, expected_api, workload, sampling, first)
                capture_result = capture(command, root, metadata["environment"], fork_folder,
                                         timeout=sampling["maximum_warmup_seconds"] + 120)
                parsed = parse_series(capture_result, sampling, mode)
                if first.read_bytes() != api_bytes:
                    raise ValueError("first request bytes differ from preflight")
                forks.append({"capture": capture_result, **parsed})
            item["api"][mode] = {"forks": forks,
                "warmup": "converged" if all(fork["warmup"] == "converged" for fork in forks) else "inconclusive",
                "fork_median_operation_ns": distribution([fork["operation_ns"]["median"] for fork in forks]),
                "fork_peak_rss_bytes": distribution([fork["peak_rss_bytes"] for fork in forks])}
        results.append(item)
        write_json(destination / "partial.json", {"scope": contract["scope"], "completed": results})
        print(f"benchmarked {name}; " + ", ".join(mode + "=" + value["warmup"] for mode, value in item["api"].items()), flush=True)
    # Trace in a separate fork: trace output/instrumentation is never mixed with timing samples.
    workload = next(item for item in contract["workloads"] if item["id"] == "recursive-compute")
    trace_command = harness_command(metadata, classes, "persistent", root / "benchmarks" / workload["module"],
        destination / workload["id"] / "expected-api.bin", workload, sampling, destination / "trace-first.bin", trace=True)
    trace = capture(trace_command, root, metadata["environment"], destination / "compilation-trace", timeout=180)
    trace_bytes = raw_channel(trace, "stdout") + raw_channel(trace, "stderr")
    compilation_lines = completed_compilations(trace_bytes, root / "benchmarks" / workload["module"])
    trace_evidence = {"capture": trace, "workload": "recursive-compute",
                      "state": "observed" if trace["exit_code"] == 0 and compilation_lines else "not-observed",
                      "completed_fib_compilations": compilation_lines,
                      "scope": "Separate instrumented recursive workload only; runtime availability alone is not workload compilation proof"}
    if before_inputs != input_census(root):
        raise ValueError("benchmark inputs changed during measurement")
    if any(digest((root / path).read_bytes()) != value for path, value in before_tools.items()):
        raise ValueError("benchmark harness changed during measurement")
    report = {"schema": 1, "timestamp_utc": datetime.now(timezone.utc).isoformat(),
              "scope": contract["scope"], "oracle": metadata, "machine_start": started, "machine_end": machine(),
              "input_files": before_inputs, "sampling": sampling, "results": results,
              "toolchain": {"javac": str(javac), "javac_sha256": oracle.sha256(javac),
                  "java_harness_sha256": digest(source.read_bytes()),
                  "python_harness_sha256": digest((root / "tools/benchmark.py").read_bytes()),
                  "compiled_classes": [{"path": str(path.relative_to(classes)), "sha256": digest(path.read_bytes())}
                      for path in sorted(classes.rglob("*.class"))]},
              "compilation_evidence": trace_evidence, "required_pending": contract["required_pending"],
              "compatibility_passes": 0, "rust_performance_ratios": None,
              "assessment": "Initial Java baseline; inspect fork convergence and machine load; full required scope remains incomplete"}
    write_json(destination / "report.json", report)
    check_report(destination, contract)
    print(f"Java baseline recorded: {destination / 'report.json'}; compilation trace: {trace_evidence['state']}")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check-contract", action="store_true")
    parser.add_argument("--check-report", type=Path, help="validate raw evidence and accounting in an existing report")
    args = parser.parse_args()
    try:
        if args.check_report:
            report = check_report(args.check_report.resolve())
            print(f"{len(report['results'])} Java workload results and raw evidence verified; pending scope remains incomplete")
        elif args.check_contract:
            contract = load_contract()
            print(f"{len(contract['workloads'])} initial workloads; {len(contract['required_pending'])} pending requirement groups")
        else:
            run(output=args.output)
        return 0
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, oracle.OracleError) as error:
        print(f"benchmark: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
