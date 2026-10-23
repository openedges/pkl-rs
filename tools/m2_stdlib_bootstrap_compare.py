"""Raw supplied-artifact bootstrap/ModuleInfo comparison; every boundary remains explicit."""
import argparse
import collections
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=True) + "\n")


def verify_oracle(root):
    closure = json.loads((root / "closure.json").read_bytes())
    assert closure["closed"] and not closure["liveHandles"]
    assert sha(root / "report.json") == closure["reportSha256"]
    assert sha(root / "inventory.json") == closure["inventorySha256"]
    rows = json.loads((root / "inventory.json").read_bytes())["files"]
    for row in rows:
        path = root / row["path"]
        assert not path.is_symlink()
        assert path.stat().st_size == row["bytes"] and sha(path) == row["sha256"]
    report = json.loads((root / "report.json").read_bytes())
    assert not report["inputChanges"]
    return report, {"root": str(root), "reportSha256": sha(root / "report.json"),
                    "closureSha256": sha(root / "closure.json"), "verifiedPayloadFiles": len(rows)}


def compare_call(java_root, expected, actual_dir, actual_prefix=None, followup_boundary=False):
    """All four byte/unit channels and status; no message normalization or partial match."""
    prefix = actual_prefix or expected["prefix"]
    original = java_root / expected["rawPrefix"]
    actual = actual_dir / prefix
    constructor = actual_dir / "constructor.status.bin"
    constructor_only = constructor.exists()
    status_path = actual.with_name(actual.name + ".status.bin")
    if constructor_only:
        observed_status = constructor.read_bytes()
    elif status_path.exists():
        observed_status = status_path.read_bytes()
    else:
        return {"status": "unexecuted", "reason": "native completion is absent", "rawMatch": False}
    differences = []
    for channel in ["stdout.bin", "stderr.bin", "stdout.code-units.bin", "stderr.code-units.bin"]:
        old = original.with_name(original.name + "." + channel).read_bytes()
        if constructor_only:
            path = actual_dir / ("constructor." + channel)
            new = path.read_bytes() if path.exists() else b""
        else:
            path = actual.with_name(actual.name + "." + channel)
            new = path.read_bytes() if path.exists() else None
        if old != new:
            differences.append(channel)
    if observed_status != expected["status"].encode():
        differences.append("status.bin")
    disposition_path = actual.with_name(actual.name + ".disposition.bin")
    disposition = disposition_path.read_text() if disposition_path.exists() else None
    result = {"rawMatch": not differences, "differentChannels": differences,
              "actualStatusHex": observed_status.hex(), "actualDisposition": disposition,
              "expectedJavaErrorClass": expected["errorClass"]}
    if followup_boundary:
        result["status"] = "isolated-followup-control"
        result["sameEvaluatorApiParityVerified"] = False
    elif expected["errorClass"] == "org.pkl.core.PklBugException":
        result["status"] = "java-internal-error-boundary"
        result["nativeExplicitUnsupported"] = disposition == "unsupported"
    elif constructor_only:
        # A single native constructor cannot establish Java's repeated API timing contract.
        result["status"] = "constructor-diagnostic-boundary"
        result["constructorTextMatch"] = all(c == "status.bin" for c in differences)
    elif disposition == "unsupported":
        result["status"] = "unsupported"
    elif observed_status not in [b"success\n", b"error\n"]:
        result["status"] = "protocol-failure"
    else:
        disposition_match = expected["status"] != "error\n" or disposition == "language-error"
        result["ordinaryErrorDispositionMatch"] = disposition_match
        result["status"] = "pass" if not differences and disposition_match else "mismatch"
    return result


def module_name(source):
    return next((line[len("module "):] for line in source.splitlines() if line.startswith("module ")), "main")


def verify_sources(root, mapping):
    for name, digest in mapping.items():
        path = root / name
        raw = os.readlink(path).encode() if path.is_symlink() else path.read_bytes()
        assert hashlib.sha256(raw).hexdigest() == digest, name


def main():
    parser = argparse.ArgumentParser()
    for name in ["bootstrap-oracle", "moduleinfo-oracle", "candidate", "source-map", "source-root", "build-report", "output"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    started = time.time_ns()
    write(output / "exclusions.json", {"declaredBeforeTraversal": True, "cargoCachePaths": [],
          "scope": "No source/cache walks: exact supplied immutable source map only; oracle payloads use sealed inventories."})
    bootstrap_root, module_root = args.bootstrap_oracle.resolve(), args.moduleinfo_oracle.resolve()
    bootstrap, bootstrap_identity = verify_oracle(bootstrap_root)
    moduleinfo, module_identity = verify_oracle(module_root)
    mapping = json.loads(args.source_map.read_bytes())
    verify_sources(args.source_root.resolve(), mapping)
    assert mapping["crates/pkl-runtime/examples/stage45_annotation_probe.rs"] == sha(
        Path(__file__).resolve().parents[1] / "crates/pkl-runtime/examples/stage45_annotation_probe.rs")
    for source, name in [(args.candidate, "stage45_annotation_probe"), (args.source_map, "source-map.json"), (args.build_report, "build-report.json")]:
        shutil.copy2(source, output / name)
    candidate = output / "stage45_annotation_probe"
    identity = {"candidateOriginalPath": str(args.candidate.resolve()), "candidateSha256": sha(candidate),
                "sourceMapSha256": sha(args.source_map), "sourceRoot": str(args.source_root.resolve()),
                "verifiedMappedSources": len(mapping), "buildReportSha256": sha(args.build_report),
                "consumerSha256": sha(Path(__file__)), "bootstrap": bootstrap_identity, "moduleInfo": module_identity}
    write(output / "identity.json", identity)
    records, comparisons = [], []

    def run(label, source, uri, expressions, formats, cwd, environment):
        directory = output / "processes" / label
        directory.mkdir(parents=True)
        command = [str(candidate), str(source), uri, module_name(source.read_text()), str(expressions), str(directory / "calls"), *formats]
        then = time.time_ns()
        try:
            process = subprocess.run(command, cwd=cwd, env=environment, capture_output=True, timeout=90)
            stdout, stderr, code, timed_out = process.stdout, process.stderr, process.returncode, False
        except subprocess.TimeoutExpired as error:
            stdout, stderr, code, timed_out = error.stdout or b"", error.stderr or b"", None, True
        (directory / "stdout.bin").write_bytes(stdout)
        (directory / "stderr.bin").write_bytes(stderr)
        record = {"label": label, "command": command, "cwd": str(cwd), "environment": environment,
                  "startedUnixNs": then, "finishedUnixNs": time.time_ns(), "exitCode": code, "timedOut": timed_out,
                  "sourceSha256": sha(source), "sourceUri": uri,
                  "stdoutSha256": sha(directory / "stdout.bin"), "stderrSha256": sha(directory / "stderr.bin")}
        write(directory / "process.json", record)
        records.append(record)
        return directory / "calls"

    for case in json.loads((bootstrap_root / "cases.json").read_bytes()):
        workspace = Path(case["workspace"])
        assert sha(workspace / "main.pkl") == case["sourceSha256"]
        for repeat in [0, 1]:
            original = next(r for r in bootstrap["records"] if r["case"] == case["id"] and r["repeat"] == repeat)
            actual = run("bootstrap/" + case["id"] + "/" + str(repeat), workspace / "main.pkl", case["moduleUri"],
                         workspace / "expressions.txt", case["formats"], Path(original["cwd"]), original["environment"])
            observation = next(o for o in bootstrap["observations"] if o["case"] == case["id"] and o["repeat"] == repeat)
            for expected in observation["calls"]:
                folder = actual / ("session-" + str(expected["session"]))
                result = compare_call(bootstrap_root, expected, folder)
                result.update(suite="bootstrap", case=case["id"], repeat=repeat, session=expected["session"],
                              format=expected["format"], prefix=expected["prefix"], expression=expected["expression"],
                              javaRawPrefix=expected["rawPrefix"], actualDirectory=str(folder.relative_to(output)))
                if case["reader"] == "counted":
                    before, after = folder / (expected["prefix"] + ".reads-before.bin"), folder / (expected["prefix"] + ".reads-after.bin")
                    result["callerReadCountsMatch"] = before.exists() and after.exists() and before.read_bytes() == str(expected["readsBefore"]).encode() and after.read_bytes() == str(expected["readsAfter"]).encode()
                    if result["status"] == "pass" and not result["callerReadCountsMatch"]:
                        result["status"] = "mismatch"
                else:
                    result["callerReadCountScope"] = "Java builtin reader has no callback instrumentation; counted profile is compared separately."
                slots = folder / "terminal-slots.bin"
                result["terminalHeapReleased"] = slots.exists() and slots.read_bytes() == b"0"
                if result["status"] == "pass" and not result["terminalHeapReleased"]:
                    result["status"] = "mismatch"
                comparisons.append(result)
    control = module_root / "workspaces/control.pkl"
    control_expressions = output / "control-expressions.txt"
    control_expressions.write_bytes(b"stable\n")
    for case in json.loads((module_root / "cases.json").read_bytes()):
        workspace = Path(case["workspace"])
        assert sha(workspace / "main.pkl") == case["sourceSha256"]
        for repeat in [0, 1]:
            original = next(r for r in moduleinfo["records"] if r["repeat"] == repeat)
            label = "moduleinfo/" + case["id"] + "/" + str(repeat)
            actual = run(label, workspace / "main.pkl", case["moduleUri"], workspace / "expressions.txt", ["absent"], Path(original["cwd"]), original["environment"]) / "session-0"
            followup = run(label + "-isolated-followup", control, control.as_uri(), control_expressions, ["absent"], Path(original["cwd"]), original["environment"]) / "session-0"
            observation = next(o for o in moduleinfo["observations"] if o["repeat"] == repeat)
            for expected in [c for c in observation["calls"] if c["case"] == case["id"]]:
                boundary = expected["prefix"].startswith("followup-")
                folder = followup if boundary else actual
                result = compare_call(module_root, expected, folder, "outer-0" if boundary else None, boundary)
                result.update(suite="moduleinfo", case=case["id"], repeat=repeat, prefix=expected["prefix"], expression=expected["expression"],
                              javaRawPrefix=expected["rawPrefix"], actualDirectory=str(folder.relative_to(output)))
                slots = folder / "terminal-slots.bin"
                result["terminalHeapReleased"] = slots.exists() and slots.read_bytes() == b"0"
                if result["status"] == "pass" and not result["terminalHeapReleased"]:
                    result["status"] = "mismatch"
                comparisons.append(result)
    assert len(comparisons) == bootstrap["outerCallCount"] + moduleinfo["apiCallCount"] == 824
    verify_sources(args.source_root.resolve(), mapping)
    assert sha(args.candidate) == sha(candidate) == identity["candidateSha256"]
    report = {"schema": 1, "scope": "All sealed corrected bootstrap and ModuleInfo calls accounted as full raw ordinary comparisons or explicit constructor/internal/isolated-followup boundaries. Not full M2 or Java product API parity.",
              "moduleInfoSessionBoundary": "Each native case has one SourceProbe. Java used one evaluator across28 root sources; native separate control sources are isolated and never counted same-evaluator followup compatibility.",
              "bootstrapSessionBoundary": "Three native SourceProbes per process correspond to three Java evaluators per fresh JVM. JVM-global pkl:test successful hidden output caches remain observable mismatches if absent in Rust.",
              "identity": identity, "records": records, "comparisons": comparisons,
              "counts": dict(collections.Counter(c["status"] for c in comparisons)),
              "rawChannelDifferenceCounts": dict(collections.Counter(channel for c in comparisons for channel in c.get("differentChannels", []))),
              "sourceAndArtifactStable": True, "startedUnixNs": started, "finishedUnixNs": time.time_ns(),
              "strictAllCallsPassed": all(c["status"] == "pass" for c in comparisons)}
    write(output / "report.json", report)
    print(json.dumps({k: report[k] for k in ["counts", "rawChannelDifferenceCounts", "sourceAndArtifactStable", "strictAllCallsPassed"]}, indent=2))
    return 0 if report["strictAllCallsPassed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
