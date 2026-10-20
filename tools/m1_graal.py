#!/usr/bin/env python3
"""Reproduce bounded Graal compilation diagnostics without changing the Java baseline."""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
from urllib.parse import unquote, urlparse

import benchmark
import oracle

ROOT = Path(__file__).resolve().parents[1]
TRACE_OPTIONS = ("-Dpolyglot.engine.TraceCompilation=true",
                 "-Dpolyglot.engine.CompilationFailureAction=Print")


def compilation_events(raw, source_uri, source_line=None):
    """Select Truffle events for the exact guest source, never unrelated JVM code."""
    source = re.escape(source_uri.encode())
    line_number = str(source_line).encode() if source_line is not None else rb"\d+"
    location = re.compile(rb"\|Src " + source + rb":(" + line_number + rb")(?:\s|$)")
    event = re.compile(rb"^\[engine\] opt (done|failed|invalidated)\s+engine=(\d+)\s+id=(\d+)\s+([^|]+)\|Tier\s+(\d+)\|")
    result = []
    for line in raw.splitlines():
        match = event.search(line)
        source_match = location.search(line)
        if match and source_match:
            result.append({"event": match[1].decode("ascii"), "engine": int(match[2]),
                           "source_line": int(source_match[1]),
                           "target_id": int(match[3]), "target": match[4].decode("utf-8").rstrip(),
                           "tier": int(match[5]), "raw_line_hex": line.hex()})
    return result


def inventory(directory):
    return {path.relative_to(directory).as_posix(): oracle.sha256(path)
            for path in sorted(directory.rglob("*"))
            if path.is_file() and path not in {directory / "report.json", directory / "EVIDENCE.json"}}


def contained(directory, relative):
    path = Path(relative)
    if path.is_absolute() or ".." in path.parts:
        raise ValueError("unsafe evidence path")
    resolved = directory / path
    if not resolved.resolve().is_relative_to(directory.resolve()):
        raise ValueError("escaping evidence path")
    return resolved


def file_identity(directory, relative):
    path = contained(directory, relative)
    return {"path": relative, "sha256": oracle.sha256(path), "bytes": path.stat().st_size}


def verify_files(directory, rows, prefix):
    if not rows or len({row["path"] for row in rows}) != len(rows):
        raise ValueError(f"empty/duplicate {prefix} inventory")
    for row in rows:
        if file_identity(directory, row["path"]) != row:
            raise ValueError(f"{prefix} identity changed: {row['path']}")
    actual = {path.relative_to(directory).as_posix() for path in (directory / prefix).rglob("*") if path.is_file()}
    if actual != {row["path"] for row in rows}:
        raise ValueError(f"missing/unlisted {prefix} file")


def captured(directory, relative, expected_command=None):
    folder = contained(directory, relative)
    result = json.loads((folder / "result.json").read_bytes())
    if result.get("status") != "executed" or result.get("exit_code") != 0:
        raise ValueError(f"unexecuted/failed diagnostic command: {relative}")
    if expected_command is not None and result["command"] != expected_command:
        raise ValueError(f"diagnostic command differs: {relative}")
    values = []
    for channel in ("stdout", "stderr"):
        data = (folder / f"{channel}.bin").read_bytes()
        identity = result[channel]
        if benchmark.digest(data) != identity["sha256"] or len(data) != identity["bytes"]:
            raise ValueError(f"raw capture identity differs: {relative}/{channel}")
        values.append(data)
    return result, *values


def checked_capture(directory, report, relative, command):
    result, stdout, stderr = captured(directory, relative, command)
    if result["cwd"] != report["acquisition_workspace"]:
        raise ValueError("capture workspace differs from recorded execution environment")
    origin = Path(report["acquisition_directory"]) / relative
    for channel in ("stdout", "stderr"):
        if result[channel]["path"] != str(origin / f"{channel}.bin"):
            raise ValueError("capture belongs to another execution case")
    return result, stdout, stderr


def checked(result):
    if result.get("status") != "executed" or result.get("exit_code") != 0:
        raise ValueError(f"diagnostic acquisition command failed: {result['command']}")
    return result


def verify_oracle(directory, report):
    """Recheck recorded performance provenance against the snapshotted immutable pins."""
    metadata = report["oracle"]
    locked_path = directory / "sources/oracle.lock.json"
    locked = json.loads(locked_path.read_bytes())
    refs = oracle.read_properties((directory / "sources/references.lock").read_text())
    if json.loads((directory / "oracle.json").read_bytes()) != metadata:
        raise ValueError("oracle provenance differs from raw metadata")
    if (metadata.get("profile") != "performance" or metadata["version"] != "0.32.0"
            or locked["version"] != "0.32.0"
            or metadata["lock_sha256"] != oracle.sha256(locked_path)
            or metadata["artifact_sha256"] != locked["artifact"]["sha256"]
            or metadata["artifact_url"] != locked["artifact"]["url"]
            or metadata["source"] != locked["source"]
            or refs["pkl.revision"] != locked["source"]["revision"]
            or refs["baseline.pkl"] != locked["version"]):
        raise ValueError("oracle baseline provenance differs from pins")
    environment = metadata["environment"]
    if (set(environment) != {"PATH", "HOME", "TMPDIR", "TMP", "TEMP", "LC_ALL", "LANG", "TZ", "TERM", "NO_COLOR", "COLUMNS"}
            or environment["LC_ALL"] != "C" or environment["LANG"] != "C" or environment["TZ"] != "UTC"):
        raise ValueError("oracle execution environment differs from controlled allowlist")
    options = [*locked["jvm_options"], *locked["performance_jdk"]["jvm_options"],
               "-Duser.home=" + environment["HOME"], "-Djava.io.tmpdir=" + environment["TMPDIR"]]
    if metadata["command"] != [metadata["java"], *options, "-cp", metadata["artifact"], "org.pkl.cli.Main"]:
        raise ValueError("baseline JVM options/classpath differ from immutable oracle profile")
    jdk = metadata["jdk"]
    distributions = locked["performance_jdk"]["distributions"].values()
    if jdk["distribution"] not in distributions:
        raise ValueError("performance JDK distribution differs from pins")
    for key, value in locked["performance_jdk"]["properties"].items():
        if jdk["properties"].get(key) != value:
            raise ValueError("performance JDK runtime identity differs from pins")
    hashes = jdk["distribution"]["runtime_sha256"]
    if (jdk["executable_sha256"] != hashes["bin/java"]
            or jdk["release_sha256"] != hashes["release"]
            or jdk["runtime_modules_sha256"] != hashes["lib/modules"]):
        raise ValueError("performance JDK runtime hashes differ from pins")
    probe = jdk["optimizing_runtime_probe"]
    if (probe["stdout_hex"] != (locked["performance_jdk"]["truffle_runtime_name"] + "\n").encode().hex()
            or probe["stderr_hex"] != ""
            or probe["source_sha256"] != oracle.sha256(directory / "sources/tools/OracleRuntimeProbe.java")):
        raise ValueError("optimizing runtime probe differs")
    baseline = directory / "artifacts" / locked["artifact"]["filename"]
    oracle.verify_artifact(baseline, locked)
    return locked


def verify_probe_output(stdout, seeds):
    rows = [json.loads(line) for line in stdout.splitlines()]
    if (len(rows) != 1 or set(rows[0]) != {"phase", "verified_calls", "seeds"}
            or rows[0]["phase"] != "compilation-probe" or rows[0]["seeds"] != seeds
            or type(rows[0]["verified_calls"]) is not int or rows[0]["verified_calls"] < seeds):
        raise ValueError("missing/incomplete compilation correctness probe")
    return rows[0]


def verify_target_events(stderr, source_uri, source_line, required_tiers=(), require_bailout=False):
    events = compilation_events(stderr, source_uri, source_line)
    completed = {}
    for row in events:
        if row["event"] == "done" and row["target"].startswith("FunctionNode@"):
            identity = (row["engine"], row["target_id"], row["target"])
            completed.setdefault(identity, set()).add(row["tier"])
    if required_tiers and not any(set(required_tiers).issubset(tiers) for tiers in completed.values()):
        raise ValueError("missing required guest target/tier compilation success")
    if require_bailout:
        if not any(row["event"] == "failed" and b"FrameWithoutBoxing" in bytes.fromhex(row["raw_line_hex"])
                   for row in events):
            raise ValueError("missing expected guest target compilation failure")
        if b"FrameWithoutBoxing" not in stderr or b"EnsureVirtualizedNode.ensureVirtualFailure" not in stderr:
            raise ValueError("missing complete frame materialization bailout diagnostic")
    return events


VM_UTILS = "external/pkl/pkl-core/src/main/java/org/pkl/core/runtime/VmUtils.java"
ORIGINAL_NODE = "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/frame/GetEnclosingFrameNode.java"
ARITHMETIC = "tests/fixtures/m1/runtime/arithmetic-workload.pkl"
RECEIVER = "tests/fixtures/m1/runtime/arithmetic-receiver.pkl"
FIB = "benchmarks/fixtures/recursive.pkl"
FIB_RECEIVER = "tests/fixtures/m1/runtime/recursive-receiver.pkl"
BYPASS = "    if (levelsUp == 0) {\n      return frame;\n    }\n    return VmUtils.getFrame(frame, levelsUp);"
TRACE_OPTIONS = (*TRACE_OPTIONS, "-Djdk.graal.TrackNodeSourcePosition=true")


def cases_contract():
    return [
        {"id": "arithmetic-baseline", "source": ARITHMETIC, "workload": "arithmetic", "overlay": None,
         "line": 1, "required_tiers": [], "require_bailout": True},
        {"id": "arithmetic-recompiled-control", "source": ARITHMETIC, "workload": "arithmetic", "overlay": "original",
         "line": 1, "required_tiers": [], "require_bailout": True},
        {"id": "arithmetic-frame-bypass", "source": ARITHMETIC, "workload": "arithmetic", "overlay": "bypass",
         "line": 1, "required_tiers": [1, 2], "require_bailout": False},
        {"id": "arithmetic-receiver-baseline", "source": RECEIVER, "workload": "arithmetic", "overlay": None,
         "line": 3, "required_tiers": [1, 2], "require_bailout": False},
        {"id": "fib-baseline", "source": FIB, "workload": "fib", "overlay": None,
         "line": 1, "required_tiers": [], "require_bailout": True},
        {"id": "fib-frame-bypass", "source": FIB, "workload": "fib", "overlay": "bypass",
         "line": 1, "required_tiers": [], "require_bailout": True},
        {"id": "fib-receiver-baseline", "source": FIB_RECEIVER, "workload": "fib", "overlay": None,
         "line": 3, "required_tiers": [1, 2], "require_bailout": False},
        {"id": "fib-scope-recompiled-control", "source": FIB, "workload": "fib", "overlay": "scope-original",
         "line": 1, "required_tiers": [], "require_bailout": True},
        {"id": "fib-scope-loop-bypass", "source": FIB, "workload": "fib", "overlay": "scope-bypass",
         "line": 1, "required_tiers": [1, 2], "require_bailout": False},
    ]


def source_paths():
    paths = ["references.lock", "oracle.lock.json", "tools/m1_graal.py", "tools/tests/test_m1_graal.py",
             "tools/oracle.py", "tools/benchmark.py", "tools/OracleRuntimeProbe.java", "tools/java/M1GraalProbe.java",
             ARITHMETIC, RECEIVER, FIB, FIB_RECEIVER, ORIGINAL_NODE,
             VM_UTILS,
             "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/member/AbstractInvokeMethodLexicalNode.java",
             "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/frame/ReadFrameSlotNode.java"]
    lock = json.loads((ROOT / "oracle.lock.json").read_bytes())
    paths.extend("external/pkl/" + path for path in lock["source"]["metadata_sha256"])
    return sorted(set(paths))


def java_command(directory, report, overlay=None, traced=False):
    command = report["oracle"]["command"]
    cp_index = command.index("-cp")
    cp = [str(directory / "artifacts/helper")]
    if overlay is not None:
        cp.insert(0, str(directory / "artifacts" / overlay))
    cp.append(str(directory / "artifacts" / Path(report["oracle"]["artifact"]).name))
    return [*command[:cp_index], *(TRACE_OPTIONS if traced else ()), "-cp", os.pathsep.join(cp), "M1GraalProbe"]


def case_command(origin, report, spec):
    command = java_command(origin, report, spec["overlay"], True)
    command.insert(1, "-Xlog:class+load=info:file=" + str(origin / "cases" / spec["id"] / "class-loading.log"))
    return [*command, "exercise", str(origin / "sources" / spec["source"]), spec["workload"],
            str(origin / "expected" / spec["workload"]), str(report["seconds"])]


def verify_class_origin(directory, report, spec):
    data = (directory / "cases" / spec["id"] / "class-loading.log").read_bytes()
    origin = Path(report["acquisition_directory"]) / "artifacts"
    jar = Path(report["oracle"]["artifact"]).name
    result = {}
    for name, overrides in (("org.pkl.core.ast.frame.GetEnclosingFrameNode", {"original", "bypass"}),
                            ("org.pkl.core.runtime.VmUtils", {"scope-original", "scope-bypass"})):
        locations = re.findall(re.escape(name.encode()) + rb" source: (\S+)", data)
        expected = origin / (spec["overlay"] if spec["overlay"] in overrides else jar)
        if len(locations) != 1:
            raise ValueError("missing/duplicate actual diagnostic class origin")
        location = urlparse(locations[0].decode("utf-8"))
        if location.scheme != "file" or Path(unquote(location.path)) != expected:
            raise ValueError("actual loaded class differs from declared diagnostic classpath")
        result[name] = locations[0].decode("utf-8")
    return result


def override_sources(directory):
    original = (directory / "sources" / ORIGINAL_NODE).read_text()
    needle = "    return VmUtils.getFrame(frame, levelsUp);"
    if original.count(needle) != 1:
        raise ValueError("unexpected original frame lookup source")
    vm_utils = (directory / "sources" / VM_UTILS).read_text()
    import_from = "import org.organicdesign.fp.collections.ImMap;"
    import_to = "import org.pkl.thirdparty.paguro.collections.ImMap;"
    if vm_utils.count(import_from) != 1:
        raise ValueError("unexpected upstream shaded dependency import")
    adapted = vm_utils.replace(import_from, import_to)
    loop = ("    var owner = getOwner(frame);\n"
            "    while (owner.isParseTimeInvisibleScope()) {\n"
            "      frame = owner.getEnclosingFrame();\n"
            "      owner = getOwner(frame);\n"
            "    }\n"
            "    return frame;")
    if adapted.count(loop) != 1:
        raise ValueError("unexpected upstream invisible-scope loop")
    return {"original": ("GetEnclosingFrameNode.java", original),
            "bypass": ("GetEnclosingFrameNode.java", original.replace(needle, BYPASS)),
            "scope-original": ("VmUtils.java", adapted),
            "scope-bypass": ("VmUtils.java", adapted.replace(loop, "    return frame;"))}


def observe_case(directory, report, spec):
    seeds = 256 if spec["workload"] == "arithmetic" else 2
    origin = Path(report["acquisition_directory"])
    source = origin / "sources" / spec["source"]
    command = case_command(origin, report, spec)
    _, stdout, stderr = checked_capture(directory, report, "cases/" + spec["id"], command)
    probe = verify_probe_output(stdout, seeds)
    events = verify_target_events(stderr, source.as_uri(), spec["line"], spec["required_tiers"], spec["require_bailout"])
    if spec["require_bailout"]:
        # The source-position flag records the actual escaping-frame path, not only a generic bailout message.
        for frame in (b"VmUtils.skipInvisibleScopes", b"VmUtils.getFrame",
                      b"AbstractInvokeMethodLexicalNode.executeGeneric" if spec["id"] == "fib-frame-bypass"
                      else b"GetEnclosingFrameNode.executeGeneric"):
            if frame not in stderr:
                raise ValueError("missing source-position frame escape causal stack")
    class_origin = verify_class_origin(directory, report, spec)
    return {"probe": probe, "target_events": events, "frame_class_origin": class_origin,
            "completed_tiers": sorted({row["tier"] for row in events if row["event"] == "done"
                                       and row["target"].startswith("FunctionNode@")}),
            "target_failed_events": sum(row["event"] == "failed" for row in events),
            "all_guest_failure_events": [row for row in compilation_events(stderr, source.as_uri())
                                         if row["event"] == "failed"],
            "diagnostic_only": True, "performance_evidence": False}


def required_artifact_paths(filename):
    paths = {"artifacts/" + filename, "artifacts/helper/M1GraalProbe.class"}
    paths.update("artifacts/" + name + "/org/pkl/core/ast/frame/GetEnclosingFrameNode.class"
                 for name in ("original", "bypass"))
    paths.update("artifacts/" + name + "/org/pkl/core/runtime/" + class_name
                 for name in ("scope-original", "scope-bypass")
                 for class_name in ("VmUtils.class", "VmUtils$1.class"))
    return paths


def check_report(directory):
    report = json.loads((directory / "report.json").read_bytes())
    if (report.get("schema") != 1 or report.get("acquisition_error")
            or report.get("performance_evidence") is not False or report.get("compatibility_complete") is not False
            or type(report.get("seconds")) is not int or not 10 <= report["seconds"] <= 600):
        raise ValueError("failed/unknown Graal diagnostic report or unsupported performance claim")
    contract = cases_contract()
    if (report.get("contract") != contract or [row.get("id") for row in report.get("cases", [])]
            != [row["id"] for row in contract]):
        raise ValueError("missing/duplicate/substituted required Graal case")
    if not report.get("source_stability_before_execution") or not report.get("source_stability_after_execution"):
        raise ValueError("sources changed during Graal acquisition")
    for key in ("sources", "artifacts", "expected"):
        verify_files(directory, report[key], key)
    if {row["path"] for row in report["sources"]} != {"sources/" + path for path in source_paths()}:
        raise ValueError("missing/substituted executable input source inventory")
    locked = verify_oracle(directory, report)
    if {row["path"] for row in report["artifacts"]} != required_artifact_paths(locked["artifact"]["filename"]):
        raise ValueError("missing/substituted compiled diagnostic artifact inventory")
    for relative, expected in locked["source"]["metadata_sha256"].items():
        if oracle.sha256(directory / "sources/external/pkl" / relative) != expected:
            raise ValueError("upstream source metadata differs from pins")
    for name, (filename, text) in override_sources(directory).items():
        if (directory / "generated" / name / filename).read_text() != text:
            raise ValueError("diagnostic override source differs from declared exact transformation")
    if inventory(directory) != report["evidence_sha256"]:
        raise ValueError("Graal evidence inventory changed")
    origin = Path(report["acquisition_directory"])
    if not origin.is_absolute():
        raise ValueError("missing original acquisition directory identity")
    javac = str(Path(report["oracle"]["java"]).with_name("javac"))
    jar = str(origin / "artifacts" / locked["artifact"]["filename"])
    compile_commands = {"helper": [javac, "-proc:none", "-cp", jar, "-d", str(origin / "artifacts/helper"),
                                      str(origin / "sources/tools/java/M1GraalProbe.java")]}
    for name, (filename, _) in override_sources(directory).items():
        compile_commands[name] = [javac, "-proc:none", "-cp", jar, "-d", str(origin / "artifacts" / name),
                                  str(origin / "generated" / name / filename)]
    for name, command in compile_commands.items():
        _, stdout, stderr = checked_capture(directory, report, "compile/" + name, command)
        if stdout or stderr:
            raise ValueError("unexpected diagnostic compiler output")
    expected_paths = set()
    for workload, source, seeds in (("arithmetic", ARITHMETIC, 256), ("fib", FIB, 2)):
        command = [*java_command(origin, report), "preflight", str(origin / "sources" / source), workload,
                   str(origin / "expected" / workload)]
        _, stdout, stderr = checked_capture(directory, report, "preflight/" + workload, command)
        if stderr or stdout != (json.dumps({"phase": "preflight", "seeds": seeds}, separators=(",", ":")) + "\n").encode():
            raise ValueError("missing/failed raw baseline correctness preflight")
        for seed in range(seeds):
            relative = f"expected/{workload}/seed-{seed:03d}.bin"
            if not (directory / relative).read_bytes():
                raise ValueError("empty raw expected result")
            expected_paths.add(relative)
    if {row["path"] for row in report["expected"]} != expected_paths:
        raise ValueError("missing/substituted raw expected output inventory")
    for row, spec in zip(report["cases"], contract):
        if row["observations"] != observe_case(directory, report, spec):
            raise ValueError("Graal observation differs from raw target/tier logs")
    return report


def execute(directory, seconds):
    directory.mkdir(parents=True, exist_ok=False)
    report = {"schema": 1, "timestamp_utc": datetime.now(timezone.utc).isoformat(), "seconds": seconds,
              "acquisition_directory": str(directory), "acquisition_workspace": str(ROOT),
              "scope": "bounded original failures, recompiled negative control, semantic bypass diagnostic and genuine unchanged-baseline receiver arithmetic compilation",
              "performance_evidence": False, "compatibility_complete": False, "contract": cases_contract(),
              "limitations": ["the zero-level frame bypass intentionally omits invisible-scope semantics and is not a production fix",
                              "compilation events do not establish optimized performance or Rust/Java ratios",
                              "other guest targets can still fail compilation; complete raw diagnostics are retained"],
              "sources": [], "artifacts": [], "expected": [], "cases": [], "acquisition_error": None}
    try:
        for relative in source_paths():
            destination = directory / "sources" / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((ROOT / relative).read_bytes())
            report["sources"].append(file_identity(directory, "sources/" + relative))
        stable = lambda: all(oracle.sha256(ROOT / row["path"].removeprefix("sources/")) == row["sha256"]
                             for row in report["sources"])
        metadata = oracle.ensure_oracle(ROOT, fetch=False, profile="performance")
        report["oracle"] = metadata
        benchmark.write_json(directory / "oracle.json", metadata)
        artifacts = directory / "artifacts"
        artifacts.mkdir()
        shutil.copy2(metadata["artifact"], artifacts / Path(metadata["artifact"]).name)
        jar = str(artifacts / Path(metadata["artifact"]).name)
        javac = str(Path(metadata["java"]).with_name("javac"))
        checked(benchmark.capture([javac, "-proc:none", "-cp", jar, "-d", str(artifacts / "helper"),
                str(directory / "sources/tools/java/M1GraalProbe.java")], ROOT, metadata["environment"], directory / "compile/helper"))
        for name, (filename, text) in override_sources(directory).items():
            source = directory / "generated" / name / filename
            source.parent.mkdir(parents=True)
            source.write_text(text)
            checked(benchmark.capture([javac, "-proc:none", "-cp", jar, "-d", str(artifacts / name), str(source)],
                                      ROOT, metadata["environment"], directory / "compile" / name))
        report["artifacts"] = [file_identity(directory, path.relative_to(directory).as_posix())
                               for path in sorted(artifacts.rglob("*")) if path.is_file()]
        report["source_stability_before_execution"] = stable()
        if not report["source_stability_before_execution"]:
            raise ValueError("sources changed during build; rerun in a fresh evidence directory")
        (directory / "expected").mkdir()
        for workload, source in (("arithmetic", ARITHMETIC), ("fib", FIB)):
            checked(benchmark.capture([*java_command(directory, report), "preflight", str(directory / "sources" / source),
                                      workload, str(directory / "expected" / workload)], ROOT, metadata["environment"],
                                      directory / "preflight" / workload))
        report["expected"] = [file_identity(directory, path.relative_to(directory).as_posix())
                              for path in sorted((directory / "expected").rglob("*")) if path.is_file()]
        for spec in report["contract"]:
            command = case_command(directory, report, spec)
            benchmark.capture(command, ROOT, metadata["environment"], directory / "cases" / spec["id"], timeout=seconds + 60)
            row = {"id": spec["id"]}
            report["cases"].append(row)
            row["observations"] = observe_case(directory, report, spec)
            print(f"{spec['id']}: checked results; completed tiers={row['observations']['completed_tiers']}", flush=True)
        report["source_stability_after_execution"] = stable()
        if not report["source_stability_after_execution"]:
            raise ValueError("sources changed during execution; preserve evidence and rerun")
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, oracle.OracleError) as error:
        report["acquisition_error"] = str(error)
        raise
    finally:
        report["evidence_sha256"] = inventory(directory)
        benchmark.write_json(directory / "report.json", report)
    return check_report(directory)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--output", type=Path)
    group.add_argument("--check-report", type=Path)
    parser.add_argument("--seconds", type=int, default=15, help="diagnostic exercise duration per fork; no performance samples")
    args = parser.parse_args()
    try:
        if not 10 <= args.seconds <= 600:
            raise ValueError("diagnostic duration must be 10..600 seconds")
        report = check_report(args.check_report.resolve()) if args.check_report else execute(args.output.resolve(), args.seconds)
        print(json.dumps({"verified_diagnostic_cases": len(report["cases"]), "performance_evidence": False}))
        return 0
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, oracle.OracleError) as error:
        print(f"m1 Graal: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
