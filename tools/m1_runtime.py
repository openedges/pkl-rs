#!/usr/bin/env python3
"""Record the bounded M1 VM experiment, preserving diagnostic mismatches explicitly."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import statistics

import benchmark
import oracle

ROOT = Path(__file__).resolve().parents[1]
SUCCESS_CASES = ("amendment", "captured-receiver", "lexical-let", "object-cycle", "lazy-unused", "mixed-numeric")
ERROR_CASES = ("first-error", "forcing-cycle", "overflow-first")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def raw(result, channel):
    entry = result[channel]
    data = Path(entry["path"]).read_bytes()
    if len(data) != entry["bytes"] or digest(data) != entry["sha256"]:
        raise ValueError("raw capture identity mismatch")
    return data


def equivalent(java, rust):
    """Whole channel bytes and process outcomes; no normalizing of diagnostics."""
    return (java["status"] == rust["status"] == "executed"
            and java["exit_code"] == rust["exit_code"]
            and raw(java, "stdout") == raw(rust, "stdout")
            and raw(java, "stderr") == raw(rust, "stderr"))


def checked(result):
    if result["status"] != "executed" or result["exit_code"] != 0:
        raise ValueError(f"command failed; preserved at {result['stderr']['path']}")
    return result


def candidate_decision(measurements, specialized):
    if not measurements:
        return "functional evidence only; no performance decision"
    if not all(fork[language]["warmup_converged"] for fork in measurements for language in ("rust", "java")):
        return "inconclusive warmup; execution-strategy comparison requires another experiment"
    ratios = [fork["rust_java_median_time_ratio"] for fork in measurements]
    if specialized and all(ratio < 1 for ratio in ratios):
        return ("adopt scalar-specialized bytecode as the bounded M2 baseline with generic fallback; "
                "revisit fully optimized JVM and object workloads before broader performance conclusions")
    if any(ratio > 1 for ratio in ratios):
        return "reject this dispatch candidate as sufficient for the measured workload; investigate specialization"
    return "retain bounded candidate; broader workload evidence remains required"


def series(result, samples, java):
    checked(result)
    # Compilation trace is a separate stderr channel; stdout remains JSON lines.
    raw(result, "stderr")
    rows = [json.loads(line) for line in raw(result, "stdout").splitlines()]
    preflight = [row for row in rows if row.get("phase") == "preflight"]
    warmup = [row for row in rows if row.get("phase") == "warmup"]
    measured = [row for row in rows if row.get("phase") == "measurement"]
    convergence = [row for row in rows if row.get("phase") == "warmup-result"]
    phases = (["preflight"] + ["warmup"] * len(warmup)
              + (["warmup-result"] if java else []) + ["measurement"] * samples)
    if ([row.get("phase") for row in rows] != phases or len(preflight) != 1
            or preflight[0].get("seeds") != 256 or len(measured) != samples
            or len(convergence) != int(java) or not warmup):
        raise ValueError("missing or unexpected benchmark phases")
    for group, minimum in ((warmup, 100_000_000), (measured, 250_000_000)):
        if [row.get("index") for row in group] != list(range(len(group))):
            raise ValueError("missing or duplicate timing windows")
        for row in group:
            if (row.get("iterations", 0) < 256 or row.get("elapsed_ns", 0) < minimum
                    or not math.isclose(row["ns_per_op"], row["elapsed_ns"] / row["iterations"], rel_tol=1e-9)):
                raise ValueError("invalid timing window")
    if sum(row["elapsed_ns"] for row in warmup) < 10_000_000_000:
        raise ValueError("insufficient measured warmup time")
    recent = [row["ns_per_op"] for row in warmup[-8:]]
    spread = (max(recent) - min(recent)) / statistics.median(recent)
    converged = len(recent) == 8 and spread <= 0.15
    if java and bool(convergence[0].get("converged")) != converged:
        raise ValueError("false Java warmup convergence")
    return {"raw": result, "operation_ns": benchmark.distribution([row["ns_per_op"] for row in measured]),
            "peak_rss_bytes": result["peak_rss_bytes"], "warmup_converged": converged,
            "recent_relative_spread": spread, "warmup_windows": len(warmup),
            "warmup_measured_ns": sum(row["elapsed_ns"] for row in warmup)}


def run(output, measure=False, forks=2, samples=9, specialized=False):
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    metadata = oracle.ensure_oracle(ROOT, profile="performance" if measure else "reference")
    benchmark.write_json(output / "oracle.json", metadata)
    inputs = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "rust-toolchain.toml",
              ROOT / "references.lock", ROOT / "oracle.lock.json",
              ROOT / "crates/pkl-runtime/Cargo.toml", ROOT / "tools/java/M1RuntimeProbe.java",
              Path(__file__), ROOT / "tools/tests/test_m1_runtime.py",
              *sorted((ROOT / "crates/pkl-runtime/src").rglob("*.rs")),
              *sorted((ROOT / "crates/pkl-runtime/src").rglob("*.pkl")),
              *sorted((ROOT / "crates/pkl-runtime/examples").rglob("*.rs")),
              *sorted((ROOT / "tests/fixtures/m1/runtime").glob("*.pkl"))]
    input_identities = []
    for path in inputs:
        data = path.read_bytes()
        snapshot = output / "sources" / path.relative_to(ROOT)
        snapshot.parent.mkdir(parents=True, exist_ok=True)
        snapshot.write_bytes(data)
        input_identities.append({"path": str(path), "snapshot": str(snapshot),
                                 "sha256": digest(data), "bytes": len(data)})
    rustc = checked(benchmark.capture(["rustc", "-vV"], ROOT, dict(os.environ), output / "rustc"))
    classes = output / "classes"
    classes.mkdir()
    source = ROOT / "tools/java/M1RuntimeProbe.java"
    javac = str(Path(metadata["java"]).with_name("javac"))
    checked(benchmark.capture([javac, "-cp", metadata["artifact"], "-d", str(classes), str(source)],
                             ROOT, metadata["environment"], output / "javac"))
    build = benchmark.capture(["cargo", "build", "-p", "pkl-runtime", "--locked", "--release",
                               "--example", "runtime_probe", "--example", "runtime_bench"],
                              ROOT, dict(os.environ), output / "cargo", timeout=180)
    checked(build)
    if any(oracle.sha256(Path(row["path"])) != row["sha256"] for row in input_identities):
        raise ValueError("source files changed during compilation; rerun in a fresh evidence directory")
    binaries = output / "binaries"
    binaries.mkdir()
    probe, rust_bench = binaries / "runtime_probe", binaries / "runtime_bench"
    shutil.copy2(ROOT / "target/release/examples/runtime_probe", probe)
    shutil.copy2(ROOT / "target/release/examples/runtime_bench", rust_bench)
    cp = metadata["command"].index("-cp")
    prefix = [*metadata["command"][:cp], "-cp", os.pathsep.join((str(classes), metadata["artifact"])), "M1RuntimeProbe"]
    cases = []
    for name in SUCCESS_CASES + ERROR_CASES:
        module = ROOT / f"tests/fixtures/m1/runtime/{name}.pkl"
        java = benchmark.capture([*prefix, "compare", str(module), "result"], ROOT, metadata["environment"], output / name / "java")
        rust = benchmark.capture([str(probe), str(module), "result"], ROOT, metadata["environment"], output / name / "rust")
        equality = equivalent(java, rust)
        expected_success = name in SUCCESS_CASES
        if (expected_success and (not equality or java["exit_code"] != 0)
                or not expected_success and (java["exit_code"] != 1 or rust["exit_code"] != 1)):
            raise ValueError(f"unexpected experiment outcome: {name}")
        cases.append({"id": name, "input": str(module), "input_sha256": oracle.sha256(module),
                      "java": java, "rust": rust, "raw_equal": equality,
                      "status": ("bounded-value-bytes-match" if expected_success else "diagnostic-bytes-match")
                      if equality else "diagnostic-byte-mismatch"})
    measured = []
    if measure:
        if forks < 2 or samples < 5:
            raise ValueError("at least two independent forks and five samples required")
        workload = ROOT / "tests/fixtures/m1/runtime/arithmetic-workload.pkl"
        # The ordering alternates across independent forks; runs are sequential.
        for index in range(forks):
            group = {}
            for language in (("rust", "java") if index % 2 == 0 else ("java", "rust")):
                if language == "rust":
                    command = [str(rust_bench), "10", str(samples), "specialized" if specialized else "generic"]
                else:
                    command = [*metadata["command"][:cp], "-Dpolyglot.engine.TraceCompilation=true",
                               "-cp", os.pathsep.join((str(classes), metadata["artifact"])), "M1RuntimeProbe",
                               "benchmark", str(workload), "10", str(samples)]
                result = benchmark.capture(command, ROOT, metadata["environment"], output / f"fork-{index}" / language, timeout=120)
                group[language] = series(result, samples, language == "java")
            group["rust_java_median_time_ratio"] = group["rust"]["operation_ns"]["median"] / group["java"]["operation_ns"]["median"]
            group["rust_java_peak_rss_ratio"] = group["rust"]["peak_rss_bytes"] / group["java"]["peak_rss_bytes"]
            compilations = benchmark.completed_compilations(raw(group["java"]["raw"], "stderr"), workload)
            group["java_workload_completed_compilations"] = compilations
            group["interpretation"] = "bounded-API-boundary-only; Java parsing per request; not dispatch parity"
            measured.append(group)
    identities = input_identities + [
        {"path": str(path), "sha256": oracle.sha256(path), "bytes": path.stat().st_size}
        for path in (probe, rust_bench)]
    if any(oracle.sha256(Path(row["path"])) != row["sha256"] for row in input_identities):
        raise ValueError("source files changed during the experiment; preserve this run and rerun")
    report = {"schema": 1, "timestamp_utc": datetime.now(timezone.utc).isoformat(),
              "scope": "bounded M1 runtime architectural experiment; incomplete language/performance gates",
              "oracle": metadata, "machine": benchmark.machine(), "identities": identities, "rustc": rustc,
              "cases": cases, "measurements": measured, "compatibility_complete": False,
              "known_gaps": ["Java diagnostic bytes and logical call stacks", "super missing-member empty Dynamic behavior",
                             "full required workloads, cold startup, product boundaries and long sessions"],
              "candidate_decision": candidate_decision(measured, specialized),
              "measurement_contract": {"workload": "128 checked integer additions, seed cycles through 0..255",
                  "correctness": "all 256 inputs validated before timing; every returned decimal byte string checked outside timing",
                  "rust_boundary": "reused immutable Program; start fresh evaluation state + poll + decimal conversion",
                  "java_boundary": "persistent optimizing evaluator; evaluateExpressionString parses run(seed) on every call + decimal conversion",
                  "rss_boundary": "whole independent process including initialization, preflight, warmup and samples",
                  "compiled_code": "Rust Program compiled once; Java function warmup and compilation trace preserved",
                  "dispatch_variant": "specialized" if specialized else "generic",
                  "instruction_count": 130 if specialized else 258, "forks": forks if measure else 0, "samples": samples if measure else 0}}
    benchmark.write_json(output / "report.json", report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--benchmark", action="store_true")
    parser.add_argument("--specialized", action="store_true")
    parser.add_argument("--report", type=Path)
    arguments = parser.parse_args()
    report = run(arguments.output, measure=arguments.benchmark, specialized=arguments.specialized)
    if arguments.report:
        benchmark.write_json(arguments.report, report)
    print(json.dumps({"bounded_value_matches": sum(case["raw_equal"] for case in report["cases"]
                                                  if case["id"] in SUCCESS_CASES),
                      "diagnostic_matches": sum(case["raw_equal"] for case in report["cases"]
                                                 if case["id"] in ERROR_CASES),
                      "diagnostic_mismatches": sum(not case["raw_equal"] for case in report["cases"]),
                      "measured_forks": len(report["measurements"]), "report": str(arguments.output / "report.json")}))


if __name__ == "__main__":
    main()
