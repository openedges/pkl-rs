#!/usr/bin/env python3
"""Compare bounded Java/Rust lifecycle protocol bytes; retain full diagnostic gaps."""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import benchmark
import oracle

ROOT = Path(__file__).resolve().parents[1]
MODES = ("retry", "reentry")
INPUTS = {
    "retry": b'first = read("life:first")\nsecond = read("life:second")\nresult = first + second\n',
    "reentry": b'seed = read("life:shared")\nshared = seed + "!"\nresult = read("life:outer") + shared\n',
}
PROTOCOLS = {
    "retry": (b"attempt=1 outcome=error first_calls=1 second_calls=0\n"
              b"attempt=2 outcome=error first_calls=2 second_calls=1\n"
              b"attempt=3 outcome=ok value=AB first_calls=2 second_calls=2\n"
              b"attempt=4 outcome=ok value=AB first_calls=2 second_calls=2\n"
              b"close outcome=ok\nafter-close outcome=error\n"),
    "reentry": (b"warmup value=S reader_calls=1\ninner value=S! reader_calls=2\nouter value=S!S! reader_calls=2\n"
                b"shared value=S! reader_calls=2\nrepeat value=S!S! reader_calls=2\n"
                b"close outcome=ok\nafter-close outcome=error\n"),
}
DIAGNOSTICS = {"retry": ("attempt-1", "attempt-2", "after-close"),
               "reentry": ("after-close",)}
NEGATIVE_MODE = "nested-new-read"
NEGATIVE_INPUT = b'shared = read("life:shared")\nresult = read("life:outer") + shared\n'
NEGATIVE_PROTOCOL = (b"inner value=S reader_calls=2\nouter outcome=error\nreader_calls=2\n"
                     b"close outcome=ok\nafter-close outcome=error\n")


def captured(result, directory, channel):
    data = (directory / f"{channel}.bin").read_bytes()
    identity = result[channel]
    if benchmark.digest(data) != identity["sha256"] or len(data) != identity["bytes"]:
        raise ValueError(f"capture identity mismatch: {directory}/{channel}")
    return data


def observe(mode, directory, java, rust):
    if mode not in MODES:
        raise ValueError("unknown lifecycle mode")
    protocol = []
    for language, result in (("java", java), ("rust", rust)):
        case = directory / language
        if result["status"] != "executed" or result["exit_code"] != 0:
            raise ValueError(f"unexecuted or failed lifecycle probe: {mode}/{language}")
        stdout = captured(result, case, "stdout")
        if captured(result, case, "stderr") != b"":
            raise ValueError(f"unexpected lifecycle stderr: {mode}/{language}")
        if stdout != PROTOCOLS[mode]:
            raise ValueError(f"incomplete or inconsistent lifecycle protocol: {mode}/{language}")
        protocol.append(stdout)
    diagnostics = []
    for name in DIAGNOSTICS[mode]:
        contents = []
        for language in ("java", "rust"):
            data = (directory / language / f"{name}.error.bin").read_bytes()
            classification = (directory / language / f"{name}.error-class.bin").read_bytes()
            if not data or not classification:
                raise ValueError(f"empty full diagnostic: {mode}/{language}/{name}")
            contents.append(data)
        diagnostics.append({"id": name, "raw_equal": contents[0] == contents[1],
                            "status": "full-diagnostic-bytes-match" if contents[0] == contents[1]
                            else "full-diagnostic-byte-mismatch"})
    return {"protocol_raw_equal": protocol[0] == protocol[1], "diagnostics": diagnostics}


def inventory(directory):
    return {path.relative_to(directory).as_posix(): oracle.sha256(path)
            for path in sorted(directory.rglob("*"))
            if path.is_file() and path != directory / "report.json"
            and not path.is_relative_to(directory / "build")}


def observe_negative(directory, result):
    if (result["status"] != "executed" or result["exit_code"] != 0
            or captured(result, directory, "stdout") != NEGATIVE_PROTOCOL
            or captured(result, directory, "stderr") != b""):
        raise ValueError("incomplete Java nested-new-read negative observation")
    if (directory / "outer.error-class.bin").read_bytes() != b"org.pkl.core.PklBugException":
        raise ValueError("unexpected Java nested-new-read error class")
    for name in ("outer.error.bin", "outer.trace.bin", "after-close.error.bin", "after-close.error-class.bin"):
        if not (directory / name).read_bytes():
            raise ValueError("missing full Java negative diagnostic")
    # This identifies the observed upstream defect, not an error-byte compatibility comparison.
    if b"java.util.ConcurrentModificationException" not in (directory / "outer.error.bin").read_bytes():
        raise ValueError("Java nested-new-read no longer exhibits the recorded cache mutation defect")
    return {"status": "observed-upstream-failure", "error_class": "org.pkl.core.PklBugException",
            "cause": "java.util.ConcurrentModificationException", "rust_comparison": False,
            "compatibility_pass": False}


def check_report(directory):
    report = json.loads((directory / "report.json").read_bytes())
    if report.get("schema") != 1 or report.get("acquisition_error"):
        raise ValueError("failed or unknown lifecycle evidence report")
    if (report.get("modes") != list(MODES)
            or [row["id"] for row in report["cases"]] != list(MODES)):
        raise ValueError("missing, duplicate or inconsistent lifecycle cases")
    if not report["source_stability_before_execution"] or not report["source_stability_after_execution"]:
        raise ValueError("lifecycle sources changed during acquisition")
    if not report["sources"] or not report["artifacts"]:
        raise ValueError("missing lifecycle source/artifact provenance")
    for row in report["sources"]:
        if oracle.sha256(directory / row["snapshot"]) != row["sha256"]:
            raise ValueError("lifecycle source snapshot changed")
    for row in report["artifacts"]:
        if oracle.sha256(directory / row["path"]) != row["sha256"]:
            raise ValueError("lifecycle artifact changed")
    if inventory(directory) != report["evidence_sha256"]:
        raise ValueError("lifecycle evidence inventory changed")
    for row in report["cases"]:
        mode = row["id"]
        if (directory / "inputs" / f"{mode}.pkl").read_bytes() != INPUTS[mode]:
            raise ValueError("lifecycle input changed")
        observed = observe(mode, directory / mode, row["java"], row["rust"])
        if observed != row["observations"] or not observed["protocol_raw_equal"]:
            raise ValueError("lifecycle observations do not match raw evidence")
    if (directory / "inputs" / f"{NEGATIVE_MODE}.pkl").read_bytes() != NEGATIVE_INPUT:
        raise ValueError("Java negative observation input changed")
    negative = report["java_negative_observation"]
    if observe_negative(directory / NEGATIVE_MODE / "java", negative["java"]) != negative["observations"]:
        raise ValueError("Java negative observation does not match raw evidence")
    return report


def checked(result):
    if result["status"] != "executed" or result["exit_code"] != 0:
        raise ValueError(f"lifecycle acquisition command failed: {result['command']}")
    return result


def sources():
    fixed = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "references.lock",
             "oracle.lock.json", "tools/m1_lifecycle.py", "tools/oracle.py", "tools/benchmark.py",
             "tools/java/M1LifecycleProbe.java", "tools/tests/test_m1_lifecycle.py"]
    paths = [ROOT / relative for relative in fixed]
    paths.extend(path for path in sorted((ROOT / "crates").rglob("*"))
                 if path.is_file() and (path.suffix == ".rs" or path.name == "Cargo.toml"))
    return paths


def execute(output):
    output.mkdir(parents=True, exist_ok=False)
    report = {"schema": 1, "timestamp_utc": datetime.now(timezone.utc).isoformat(),
              "scope": "bounded lifecycle protocol: failed member retry, successful member memoization, first-error order, same-evaluator callback reentry, shared object/cache, outer resumption, idempotent close and post-close rejection",
              "limitations": ["protocol bytes are not full Java diagnostics or public API compatibility",
                              "Java parses the Pkl fixtures; Rust uses explicit bytecode translations checked against exact fixture bytes, not frontend read(...) lowering",
                              "reentry uses a prewarmed resource: a new nested Java resource read inside an outer read can raise ConcurrentModificationException; that unsupported upstream edge is outside this successful protocol",
                              "native bounded VM/Host only; not browser/Node lifecycle parity",
                              "no performance or long-session memory claim"],
              "compatibility_complete": False, "modes": list(MODES), "cases": [],
              "sources": [], "artifacts": [], "acquisition_error": None}
    try:
        # Snapshot every candidate workspace Rust source before compiling in a fresh target directory.
        for path in sources():
            data = path.read_bytes()
            relative = path.relative_to(ROOT)
            snapshot = Path("sources") / relative
            destination = output / snapshot
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
            report["sources"].append({"path": relative.as_posix(), "snapshot": snapshot.as_posix(),
                                      "sha256": benchmark.digest(data), "bytes": len(data)})
        inputs = output / "inputs"
        inputs.mkdir()
        for mode, data in INPUTS.items():
            (inputs / f"{mode}.pkl").write_bytes(data)
        (inputs / f"{NEGATIVE_MODE}.pkl").write_bytes(NEGATIVE_INPUT)
        metadata = oracle.ensure_oracle(ROOT, fetch=False)
        report["oracle"] = metadata
        benchmark.write_json(output / "oracle.json", metadata)
        report["rustc"] = checked(benchmark.capture(["rustc", "-vV"], ROOT, dict(os.environ), output / "rustc"))
        classes = output / "classes"
        classes.mkdir()
        checked(benchmark.capture([str(Path(metadata["java"]).with_name("javac")), "-cp", metadata["artifact"],
                                   "-d", str(classes), str(output / "sources/tools/java/M1LifecycleProbe.java")],
                                  ROOT, metadata["environment"], output / "javac"))
        environment = dict(os.environ, CARGO_TARGET_DIR=str(output / "build"))
        build = checked(benchmark.capture(["cargo", "build", "-p", "pkl-runtime", "--locked", "--release",
                                           "--example", "lifecycle_probe", "--message-format=json"],
                                          ROOT, environment, output / "cargo", timeout=240))
        report["build"] = build
        messages = [json.loads(line) for line in benchmark.raw_channel(build, "stdout").splitlines()]
        executables = [Path(row["executable"]) for row in messages
                       if row.get("reason") == "compiler-artifact" and row.get("executable")
                       and row.get("target", {}).get("name") == "lifecycle_probe"
                       and row.get("target", {}).get("kind") == ["example"]]
        if len(executables) != 1 or not executables[0].is_relative_to(output / "build"):
            raise ValueError("missing or inconsistent freshly built candidate artifact")
        binary = output / "binaries/lifecycle_probe"
        binary.parent.mkdir()
        shutil.copy2(executables[0], binary)
        report["artifacts"] = [{"path": path.relative_to(output).as_posix(), "sha256": oracle.sha256(path),
                                 "bytes": path.stat().st_size}
                                for path in [binary, *sorted(classes.glob("*.class"))]]
        stable = lambda: all(oracle.sha256(ROOT / row["path"]) == row["sha256"] for row in report["sources"])
        report["source_stability_before_execution"] = stable()
        if not report["source_stability_before_execution"]:
            raise ValueError("sources changed during build; preserve evidence and rerun in a fresh directory")
        cp = metadata["command"].index("-cp")
        prefix = [*metadata["command"][:cp], "-cp", os.pathsep.join((str(classes), metadata["artifact"])),
                  "M1LifecycleProbe"]
        for mode in MODES:
            row = {"id": mode}
            for language, command in (("java", prefix), ("rust", [str(binary)])):
                case = output / mode / language
                row[language] = benchmark.capture([*command, mode, str(inputs / f"{mode}.pkl"), str(case)],
                                                   ROOT, metadata["environment"], case, timeout=45)
            # Retain the captures even when contract validation fails.
            report["cases"].append(row)
            row["observations"] = observe(mode, output / mode, row["java"], row["rust"])
            print(f"{mode}: protocol bytes match; full diagnostics retained separately", flush=True)
        case = output / NEGATIVE_MODE / "java"
        negative = {"java": benchmark.capture([*prefix, NEGATIVE_MODE,
                    str(inputs / f"{NEGATIVE_MODE}.pkl"), str(case)], ROOT, metadata["environment"], case, timeout=45)}
        report["java_negative_observation"] = negative
        negative["observations"] = observe_negative(case, negative["java"])
        print("nested-new-read: upstream Java failure observed; no Rust compatibility pass", flush=True)
        report["source_stability_after_execution"] = stable()
        if not report["source_stability_after_execution"]:
            raise ValueError("sources changed during execution; preserve evidence and rerun")
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, oracle.OracleError) as error:
        report["acquisition_error"] = str(error)
        raise
    finally:
        report["evidence_sha256"] = inventory(output)
        benchmark.write_json(output / "report.json", report)
    check_report(output)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--output", type=Path)
    group.add_argument("--check-report", type=Path)
    args = parser.parse_args()
    try:
        report = check_report(args.check_report.resolve()) if args.check_report else execute(args.output.resolve())
        print(json.dumps({"protocol_matches": len(report["cases"]),
                          "full_diagnostic_mismatches": sum(not error["raw_equal"] for row in report["cases"]
                                                            for error in row["observations"]["diagnostics"]),
                          "compatibility_complete": False}))
        return 0
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, oracle.OracleError) as error:
        print(f"m1 lifecycle: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
