#!/usr/bin/env python3
"""Observe the pinned Java evaluator lifecycle; this is not Rust/API parity evidence."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

import oracle

ROOT = Path(__file__).resolve().parents[1]
MODES = ("repeat", "delayed", "reentry", "timeout", "close")
CLOSED_CLASSES = {"java.lang.IllegalStateException", "org.graalvm.polyglot.PolyglotException"}


def events(data):
    result = {}
    for line in data.decode("ascii").splitlines():
        key, value = line.split("\t")
        if key in result:
            raise ValueError(f"duplicate Host event: {key}")
        result[key] = value
    return result


def observe(mode, directory):
    if mode not in MODES:
        raise ValueError("unknown Host observation mode")
    observed = events((directory / "stdout.bin").read_bytes())
    def value(name, expected):
        return observed.get(name) == "ok" and (directory / f"{name}.result.bin").read_bytes() == expected
    checks = {"warmup": value("warmup", b"1"),
              "close-idempotent": value("close-once", b"closed") and value("close-twice", b"closed"),
              "after-close-rejected": observed.get("after-final-close") in ( {"java.util.concurrent.RejectedExecutionException"} if mode == "timeout" else CLOSED_CLASSES )
                 and bool((directory / "after-final-close.error.bin").read_bytes())}
    if mode == "repeat":
        checks.update({"same-module-cached": value("first", b"payload-1") and value("repeat", b"payload-1"),
                       "resource-cache-across-modules": value("second-module", b"payload-1") and observed.get("reader-calls") == "1",
                       "fresh-evaluator-new-read": value("fresh", b"payload-2") and observed.get("reader-calls-after-fresh") == "2"})
    elif mode == "reentry":
        checks["same-thread-reentry-succeeded"] = value("reentrant", b"41") and value("outer", b"41") and observed.get("reader-calls") == "1"
    else:
        checks["real-reader-callback-entered"] = observed.get("pending-before-release") == "true" and observed.get("reader-calls") == "1"
        if mode == "delayed":
            checks["delayed-result-and-cache"] = value("evaluation", b"payload-1") and value("repeat", b"payload-1")
        elif mode == "timeout":
            checks["evaluation-timed-out"] = observed.get("evaluation") == "org.pkl.core.PklException" and (directory / "evaluation.error.bin").read_bytes() == b"Evaluation timed out after 2 second(s)."
            checks["timeout-permanently-closed"] = observed.get("after-timeout") == "java.util.concurrent.RejectedExecutionException" and bool((directory / "after-timeout.error.bin").read_bytes())
        elif mode == "close":
            checks["close-completed"] = value("close", b"closed")
            checks["active-evaluation-cancelled"] = observed.get("evaluation") in {"org.pkl.core.PklException", "org.pkl.core.PklBugException", "org.graalvm.polyglot.PolyglotException"} and bool((directory / "evaluation.error.bin").read_bytes())
            checks["closed-evaluator-rejected"] = observed.get("after-close") in CLOSED_CLASSES and bool((directory / "after-close.error.bin").read_bytes())
    return observed, checks


def check_report(directory):
    report = json.loads((directory / "report.json").read_text())
    if not report["source_stability"] or not report["source_sha256"]:
        raise ValueError("Host helper provenance is incomplete or changed")
    for relative, expected in report["source_sha256"].items():
        if oracle.sha256(directory / "sources" / relative) != expected:
            raise ValueError(f"Host helper source snapshot changed: {relative}")
    if report["modes"] != list(MODES) or [row["mode"] for row in report["results"]] != list(MODES):
        raise ValueError("missing or duplicated Host observations")
    actual = {path.relative_to(directory).as_posix() for path in directory.rglob("*")
              if path.is_file() and path != directory / "report.json"}
    if actual != set(report["evidence_sha256"]):
        raise ValueError("incomplete Host evidence inventory")
    for relative, expected in report["evidence_sha256"].items():
        if oracle.sha256(directory / relative) != expected:
            raise ValueError(f"Host evidence changed: {relative}")
    for row in report["results"]:
        case = directory / row["mode"]
        observed, checks = observe(row["mode"], case)
        if (row["exit_code"] != 0 or (case / "stderr.bin").read_bytes() != b""
                or observed != row["observations"] or checks != row["checks"] or not all(checks.values())):
            raise ValueError(f"Host observation failed: {row['mode']}")
    return report


def execute(output):
    if output.exists():
        raise ValueError("evidence output exists; use a new directory")
    metadata = oracle.ensure_oracle(ROOT, fetch=False)
    output.mkdir(parents=True)
    sources = ["tools/java/M1HostProbe.java", "tools/m1_host.py"]
    source_hashes = {path: oracle.sha256(ROOT / path) for path in sources}
    for relative in sources:
        destination = output / "sources" / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes((ROOT / relative).read_bytes())
    rows = []
    for mode in MODES:
        directory = output / mode
        directory.mkdir()
        command = [*metadata["command"][:-1], str(ROOT / "tools/java/M1HostProbe.java"), mode, str(directory)]
        result = subprocess.run(command, cwd=ROOT, env=metadata["environment"], capture_output=True, timeout=45)
        (directory / "stdout.bin").write_bytes(result.stdout)
        (directory / "stderr.bin").write_bytes(result.stderr)
        try:
            observed, checks = observe(mode, directory)
            error = None
        except (OSError, ValueError) as failure:
            observed, checks, error = {}, {"complete-observations": False}, str(failure)
        row = {"mode": mode, "command": command, "cwd": str(ROOT), "exit_code": result.returncode,
               "observations": observed, "checks": checks, "acquisition_error": error}
        rows.append(row)
        print(f"{mode}: {'observed' if result.returncode == 0 and all(checks.values()) else 'FAILED'}", flush=True)
    report = {"schema_version": 1,
              "scope": "Java-only evaluator ResourceReader, caching, same-thread reentry, timeout and close observations; not Rust byte/API equivalence or async protocol parity",
              "modes": list(MODES), "oracle": metadata, "source_sha256": source_hashes,
              "source_stability": all(oracle.sha256(ROOT / path) == digest for path, digest in source_hashes.items()),
              "results": rows}
    report["evidence_sha256"] = {path.relative_to(output).as_posix(): oracle.sha256(path)
                                 for path in sorted(output.rglob("*")) if path.is_file()}
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    if not report["source_stability"]:
        raise ValueError("Host helper sources changed during acquisition")
    check_report(output)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "target/m1/host-java")
    parser.add_argument("--check-report", type=Path)
    args = parser.parse_args()
    try:
        if args.check_report:
            check_report(args.check_report.resolve())
            print("Java Host observations verified")
        else:
            execute(args.output.resolve())
        return 0
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, oracle.OracleError) as error:
        print(f"m1 host: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
