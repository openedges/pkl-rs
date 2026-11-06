#!/usr/bin/env python3
"""Bounded Pkl 0.32 metadata public API oracle; HTTP response stub, no CLI/network claim.

Compare selected required public metadata fields and complete error class/message.
Optional fields, annotation export, resolver cache lifecycle, ZIP downloading and Host
integration are outside this observation. Each case runs in a separate JVM.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import time

import oracle
from m2_package_fixtures import load_verified_manifest

ROOT = Path(__file__).resolve().parents[1]
PACKAGES = ("birds@0.5.0", "fruit@1.0.5", "badMetadataJson@1.0.0",
            "badPackageZipUrl@1.0.0", "badImportsWithinPackage@1.0.0")
SKIP = "$skipChecksumVerification"


def checked(path):
    """Reject protected evidence paths before filesystem access, also after resolution."""
    path = Path(path).absolute()
    def reject(value):
        for part in value.parts:
            lower = part.lower()
            if ("stage86" in lower or "private8" in lower
                    or lower == "stage65-b-native-regressions-evidence.tar.gz.partial"):
                raise ValueError("protected path")
        if any(suffix in value.name.lower() for suffix in (".tar", ".zip", ".gz")):
            for part in value.parts:
                match = re.search(r"stage(\d+)", part, re.I)
                if match and int(match.group(1)) <= 136:
                    raise ValueError("historical evidence archive body is excluded")
    reject(path)
    resolved = path.resolve()
    reject(resolved)
    return resolved


def digest(path):
    return oracle.sha256(checked(path))


def save(path, value):
    checked(path).write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def cases(fixture):
    result = []
    for name in PACKAGES:
        path = checked(Path(fixture["fixture_root"]) / name / f"{name}.json")
        body = path.read_bytes()
        result.append({"id": "canonical-" + name, "body": body,
                       "package": "package://localhost:0/" + name,
                       "expected": hashlib.sha256(body).hexdigest(), "test_mode": False,
                       "origin": str(path)})
    package = "package://localhost:0/metadata-control@1.0.0"
    malformed = b'{"name":'
    valid = {"name": "metadata-control", "packageUri": package, "version": "1.0.0",
             "packageZipUrl": "https://localhost:0/control.zip",
             "packageZipChecksums": {"sha256": "abc"}, "dependencies": {}}
    valid_body = json.dumps(valid, separators=(",", ":")).encode()
    def add(name, body, expected=None, test_mode=False):
        result.append({"id": name, "body": body, "package": package,
                       "expected": expected, "test_mode": test_mode,
                       "origin": "authored-minimal-control"})
    add("malformed-wrong-checksum", malformed, "0" * 64)
    add("malformed-correct-checksum", malformed, hashlib.sha256(malformed).hexdigest())
    for mode in (False, True):
        suffix = "on" if mode else "off"
        add("skip-valid-" + suffix, valid_body, SKIP, mode)
        add("skip-malformed-" + suffix, malformed, SKIP, mode)
    add("near-skip-marker-test-mode-on", valid_body, SKIP + " ", True)
    prefix = {}
    for field, value in valid.items():
        add("missing-" + field, json.dumps(prefix, separators=(",", ":")).encode())
        prefix[field] = value
    # Raw invalid UTF-8 within a JSON string must reach Java's replacement decoder.
    add("invalid-utf8-name", valid_body.replace(b'metadata-control",', b'\xff",', 1))
    return result


def run(command, destination, env, timeout=90):
    destination = checked(destination)
    destination.mkdir()
    started = time.time()
    record = {"command": command, "cwd": str(destination), "started_unix": started}
    try:
        process = subprocess.run(command, cwd=destination, env=env,
                                 capture_output=True, timeout=timeout)
        stdout, stderr = process.stdout, process.stderr
        record.update(exit_code=process.returncode, timeout=False)
    except subprocess.TimeoutExpired as error:
        stdout, stderr = error.stdout or b"", error.stderr or b""
        record.update(exit_code=None, timeout=True)
    (destination / "stdout.bin").write_bytes(stdout)
    (destination / "stderr.bin").write_bytes(stderr)
    record.update(elapsed_seconds=time.time() - started,
                  stdout_sha256=hashlib.sha256(stdout).hexdigest(),
                  stderr_sha256=hashlib.sha256(stderr).hexdigest())
    save(destination / "process.json", record)
    return record


def result_files(directory, protocol=2):
    directory = checked(directory)
    result = {}
    if not directory.is_dir():
        raise ValueError("missing result directory")
    for path in directory.rglob("*"):
        path = checked(path)
        if path.is_file():
            result[str(path.relative_to(directory))] = path.read_bytes()
    if result.get("status.txt") not in (b"ok\n", b"error\n"):
        raise ValueError("missing/invalid result status")
    if result["status.txt"] == b"error\n":
        required = {"status.txt", "error-class.utf8", "message.utf8", "message.utf16be"}
        if protocol >= 2:
            required.add("error-class.utf16be")
        if set(result) != required:
            raise ValueError("incomplete or unexpected error channels")
    else:
        required = {"status.txt"} | {field + encoding for field in
            ("name", "version", "zip-uri", "zip-sha256") for encoding in (".utf8", ".utf16be")}
        if not required <= result.keys():
            raise ValueError("incomplete success channels")
        for path in result.keys() - required:
            if not re.fullmatch(r"dependencies/(?:empty|(?:[0-9a-f]{4})+)/(?:name|uri|sha256)\.(?:utf8|utf16be)", path):
                raise ValueError("unexpected result channel")
        groups = {str(Path(path).parent) for path in result.keys() - required}
        for group in groups:
            channels = {group + "/" + field + encoding for field in ("name", "uri", "sha256")
                        for encoding in (".utf8", ".utf16be")}
            if not channels <= result.keys():
                raise ValueError("incomplete dependency channels")
    return result


def compare(java_directory, rust_directory, oracle_protocol=2):
    java = result_files(java_directory, oracle_protocol)
    rust = result_files(rust_directory)
    return {"equal": java == rust,
            "missing": list(java.keys() - rust.keys()),
            "extra": list(rust.keys() - java.keys()),
            "different": [name for name in java.keys() & rust.keys() if java[name] != rust[name]]}


def acquire(args):
    output = checked(args.output)
    if output.exists():
        raise ValueError("fresh output directory required")
    fixture_path = checked(args.fixture_manifest)
    fixture = load_verified_manifest(fixture_path)
    selected = cases(fixture)
    java = checked(args.java)
    source = checked(ROOT / "tools/java/PackageMetadataProbe.java")
    lock = json.loads((ROOT / "oracle.lock.json").read_bytes())
    if lock["version"] != "0.32.0" or lock["source"]["revision"] != "e51fb2f91774265666f94980ec630db794a4db2d":
        raise ValueError("wrong oracle source pin")
    jar = checked(ROOT / ".cache/oracle" / lock["artifact"]["filename"])
    oracle.verify_sources(ROOT, lock)
    oracle.verify_artifact(jar, lock)
    output.mkdir(parents=True)
    env = {"PATH": os.defpath, "LC_ALL": "C", "LANG": "C", "TZ": "UTC",
           "HOME": str(output / "home"), "TMPDIR": str(output / "tmp")}
    for name in ("home", "tmp", "classes", "cases"):
        (output / name).mkdir()
    identity = oracle.verify_java(java, lock, env)
    (output / "java-identity.stdout.bin").write_bytes(bytes.fromhex(identity["version_stdout_hex"]))
    (output / "java-identity.stderr.bin").write_bytes(bytes.fromhex(identity["version_stderr_hex"]))
    (output / source.name).write_bytes(source.read_bytes())
    (output / "fixture-manifest.json").write_bytes(fixture_path.read_bytes())
    (output / "run.py").write_bytes(Path(__file__).read_bytes())
    compiler = checked(Path(identity["properties"]["java.home"]) / "bin/javac")
    record = {"schema": 1, "protocol": 2, "scope": __doc__, "host": platform.platform(),
              "oracle_lock_sha256": digest(ROOT / "oracle.lock.json"),
              "reference_revision": lock["source"]["revision"],
              "artifact_sha256": digest(jar), "jdk": identity,
              "javac_sha256": digest(compiler), "environment": env,
              "fixture_manifest": str(fixture_path), "fixture_manifest_sha256": digest(fixture_path),
              "producer_sha256": {source.name: digest(source), "run.py": digest(__file__),
                                  "oracle.py": digest(ROOT / "tools/oracle.py"),
                                  "m2_package_fixtures.py": digest(ROOT / "tools/m2_package_fixtures.py")},
              "cases": [], "complete": False}
    save(output / "report.json", record)
    compilation = run([str(compiler), "-cp", str(jar), "-d", str(output / "classes"),
                       str(output / source.name)], output / "compile", env)
    record["compile"] = compilation
    save(output / "report.json", record)
    if compilation["exit_code"] != 0:
        raise ValueError("oracle compilation failed; raw output retained")
    record["classes_sha256"] = {str(p.relative_to(output)): digest(p)
                                for p in (output / "classes").glob("*.class")}
    options = [*lock["jvm_options"], f"-Duser.home={env['HOME']}",
               f"-Djava.io.tmpdir={env['TMPDIR']}"]
    for case in selected:
        folder = output / "cases" / case["id"]
        folder.mkdir()
        body = case.pop("body")
        (folder / "input.bin").write_bytes(body)
        case["input_sha256"] = digest(folder / "input.bin")
        case["request_uri"] = case["package"].replace("package:", "https:", 1)
        save(folder / "case.json", case)
        arguments = [case["package"], case["request_uri"], str(folder / "input.bin"),
                     case["expected"] if case["expected"] is not None else "-",
                     str(case["test_mode"]).lower()]
        command = [str(java), *options, "-Dorg.pkl.testMode=" + arguments[-1], "-cp",
                   str(jar) + os.pathsep + str(output / "classes"), "PackageMetadataProbe",
                   *arguments, str(folder / "java" / "result")]
        item = {"id": case["id"], "input": case,
                "case_json_sha256": digest(folder / "case.json"),
                "java": run(command, folder / "java", env)}
        if item["java"]["exit_code"] == 0:
            values = result_files(folder / "java/result")
            item["java_result_sha256"] = {name: hashlib.sha256(value).hexdigest()
                                           for name, value in values.items()}
            item["request_trace_sha256"] = digest(folder / "java/request-trace.tsv")
        if args.rust_probe:
            rust = checked(args.rust_probe)
            record["rust_probe"] = {"path": str(rust), "sha256": digest(rust),
                                     "provenance": "caller-supplied executable"}
            item["rust"] = run([str(rust), *arguments, str(folder / "rust/result")],
                               folder / "rust", env)
            if item["java"]["exit_code"] == 0 and item["rust"]["exit_code"] == 0:
                item["comparison"] = compare(folder / "java/result", folder / "rust/result")
        record["cases"].append(item)
        save(output / "report.json", record)
    record["complete"] = len(record["cases"]) == len(selected)
    record["all_java_executed"] = all(item["java"]["exit_code"] == 0 for item in record["cases"])
    save(output / "report.json", record)
    return record


def verified_capture(directory):
    """Verify the recorded epoch; do not require current producer or fixture schema."""
    directory = checked(directory)
    report = json.loads((directory / "report.json").read_bytes())
    if not report["complete"] or not report["all_java_executed"]:
        raise ValueError("Java capture was not completed")
    protocol = report.get("protocol", 1)
    if protocol not in (1, 2):
        raise ValueError("unknown captured protocol")
    for name in ("PackageMetadataProbe.java", "run.py"):
        if digest(directory / name) != report["producer_sha256"][name]:
            raise ValueError("captured producer identity changed")
    if digest(directory / "fixture-manifest.json") != report["fixture_manifest_sha256"]:
        raise ValueError("captured fixture manifest changed")
    for name, expected in report["classes_sha256"].items():
        if digest(directory / name) != expected:
            raise ValueError("captured Java class changed")
    for channel in ("stdout", "stderr"):
        if (directory / ("java-identity." + channel + ".bin")).read_bytes() != bytes.fromhex(
                report["jdk"]["version_" + channel + "_hex"]):
            raise ValueError("captured JDK identity channel changed")
    seen = set()
    for item in report["cases"]:
        if item["id"] in seen or not re.fullmatch(r"[a-zA-Z0-9@._-]+", item["id"]):
            raise ValueError("invalid or duplicate captured case")
        seen.add(item["id"])
        folder = directory / "cases" / item["id"]
        case = json.loads((folder / "case.json").read_bytes())
        if protocol >= 2 and (case != item["input"] or
                digest(folder / "case.json") != item["case_json_sha256"]):
            raise ValueError("captured case declaration changed")
        if digest(folder / "input.bin") != case["input_sha256"]:
            raise ValueError("captured body changed")
        arguments = [case["package"], case["request_uri"], str(folder / "input.bin"),
                     case["expected"] if case["expected"] is not None else "-",
                     str(case["test_mode"]).lower(), str(folder / "java/result")]
        if item["java"]["command"][-6:] != arguments:
            raise ValueError("captured invocation differs from case")
        process = json.loads((folder / "java/process.json").read_bytes())
        if process != item["java"] or process["exit_code"] != 0 or process["timeout"]:
            raise ValueError("captured Java execution differs")
        for channel in ("stdout", "stderr"):
            if digest(folder / "java" / (channel + ".bin")) != process[channel + "_sha256"]:
                raise ValueError("captured Java process channel changed")
        observed = {name: hashlib.sha256(value).hexdigest()
                    for name, value in result_files(folder / "java/result", protocol).items()}
        if observed != item["java_result_sha256"]:
            raise ValueError("captured Java result changed")
        trace = folder / "java/request-trace.tsv"
        if digest(trace) != item["request_trace_sha256"] or trace.read_bytes() != (
                "GET\t" + case["request_uri"] + "\n").encode():
            raise ValueError("captured request trace changed")
    return report


def replay(args):
    original = checked(args.oracle_capture)
    report = verified_capture(original)
    output = checked(args.output)
    if output.exists():
        raise ValueError("fresh comparison output required")
    probe = checked(args.rust_probe)
    closure = {str(probe): digest(probe)}
    if args.rust_payload:
        payload = checked(args.rust_payload)
        closure[str(payload)] = digest(payload)
    build_evidence = {}
    if args.rust_build_evidence:
        evidence_path = checked(args.rust_build_evidence)
        evidence = json.loads(evidence_path.read_bytes())
        artifact = evidence["artifact"]
        if str(checked(artifact["path"])) not in closure or closure[str(checked(artifact["path"]))] != artifact["sha256"]:
            raise ValueError("build evidence identifies a different candidate")
        build_evidence[str(evidence_path)] = digest(evidence_path)
        source_bookend = checked(evidence["source_bookend"])
        build_evidence[str(source_bookend)] = digest(source_bookend)
        for receipt in evidence["link_receipts"]:
            pin = receipt["receipt"]
            if digest(pin["path"]) != pin["sha256"]:
                raise ValueError("candidate build receipt changed")
            build_evidence[pin["path"]] = pin["sha256"]
    # A wrapper needs its explicitly identified payload; never copy just a launcher.
    with probe.open("rb") as stream:
        is_script = stream.read(2) == b"#!"
    if is_script and not args.rust_payload:
        raise ValueError("script probe requires --rust-payload to identify runtime closure")
    output.mkdir(parents=True)
    (output / "oracle-report.json").write_bytes((original / "report.json").read_bytes())
    env = {"PATH": os.defpath, "LC_ALL": "C", "LANG": "C", "TZ": "UTC"}
    record = {"schema": 1, "scope": __doc__, "oracle_capture": str(original),
              "oracle_report_sha256": digest(original / "report.json"),
              "oracle_protocol": report.get("protocol", 1), "candidate_protocol": 2,
              "oracle_producer_sha256": report["producer_sha256"],
              "comparison_tool_sha256": digest(__file__), "runtime_closure": closure,
              "recorded_local_build_evidence": build_evidence,
              "candidate_provenance": "caller-supplied executable; build evidence remains separate",
              "environment": env, "host": platform.platform(), "cases": [], "complete": False}
    save(output / "report.json", record)
    for item in report["cases"]:
        source = original / "cases" / item["id"]
        case = json.loads((source / "case.json").read_bytes())
        folder = output / item["id"]
        folder.mkdir()
        (folder / "input.bin").write_bytes((source / "input.bin").read_bytes())
        arguments = [case["package"], case["request_uri"], str(folder / "input.bin"),
                     case["expected"] if case["expected"] is not None else "-",
                     str(case["test_mode"]).lower(), str(folder / "rust/result")]
        observed = {"id": item["id"], "input_sha256": digest(folder / "input.bin"),
                    "rust": run([str(probe), *arguments], folder / "rust", env)}
        if observed["rust"]["exit_code"] == 0:
            observed["comparison"] = compare(source / "java/result", folder / "rust/result",
                                               report.get("protocol", 1))
            observed["result_sha256"] = {name: hashlib.sha256(value).hexdigest()
                for name, value in result_files(folder / "rust/result").items()}
        record["cases"].append(observed)
        save(output / "report.json", record)
    verified_capture(original)
    if digest(original / "report.json") != record["oracle_report_sha256"] or any(
            digest(path) != expected for path, expected in {**closure, **build_evidence}.items()):
        raise ValueError("oracle or candidate changed during comparison")
    record["complete"] = True
    save(output / "report.json", record)
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture-manifest")
    parser.add_argument("--java", help="pinned JDK 21 executable; never PATH default")
    parser.add_argument("--output", required=True)
    parser.add_argument("--rust-probe")
    parser.add_argument("--oracle-capture", help="compare retained Java capture without running Java")
    parser.add_argument("--rust-build-evidence", help="recorded local artifact-closure.json")
    parser.add_argument("--rust-payload", help="actual payload if --rust-probe is a launcher")
    args = parser.parse_args()
    if args.oracle_capture:
        if not args.rust_probe:
            parser.error("--oracle-capture requires --rust-probe")
        result = replay(args)
        return 0 if result["complete"] and all(
            item.get("comparison", {}).get("equal", False) for item in result["cases"]) else 1
    if not args.fixture_manifest or not args.java:
        parser.error("acquisition requires --fixture-manifest and --java")
    result = acquire(args)
    return 0 if result["all_java_executed"] and all(
        item.get("comparison", {"equal": not args.rust_probe})["equal"] for item in result["cases"]) else 1


if __name__ == "__main__":
    sys.exit(main())
