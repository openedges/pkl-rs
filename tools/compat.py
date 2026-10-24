#!/usr/bin/env python3
"""Capture Java oracle evidence and compare observable bytes without normalization."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
MARKER = ".pkl-rs-compat-workspace.json"
PRIVATE = {MARKER, "generated", ".state"}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encoded(value) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode()


def read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(encoded(value))


def relative_path(root: Path, relative: str) -> Path:
    path = Path(relative)
    if path.is_absolute() or ".." in path.parts:
        raise ValueError(f"Invalid relative path: {relative}")
    return root / path


def contained(root: Path, relative: str) -> Path:
    path = relative_path(root, relative).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError(f"Path escapes workspace: {relative}")
    return path


def no_symlinks(path: Path) -> None:
    for parent in (path, *path.parents):
        if parent.is_symlink():
            raise ValueError(f"Managed path contains a symlink: {parent}")


def overlap(first: Path, second: Path) -> bool:
    return first.is_relative_to(second) or second.is_relative_to(first)


def validate_paths(root: Path, workspace: Path, destination: Path | None = None,
                   protected: tuple[Path, ...] = ()) -> None:
    root = root.resolve()
    target = root / "target"
    for path in (workspace, *((destination,) if destination else ())):
        absolute = path.absolute()
        no_symlinks(absolute)
        resolved = absolute.resolve()
        if resolved == target or not resolved.is_relative_to(target):
            raise ValueError(f"Managed workspace/evidence must be below {target}: {path}")
    workspace = workspace.resolve()
    if destination and overlap(workspace, destination.resolve()):
        raise ValueError("Workspace and evidence directory must not overlap")
    for path in protected:
        if overlap(workspace, path.resolve()):
            raise ValueError(f"Workspace overlaps protected input/evidence: {path}")


def load_manifest(path: Path) -> dict:
    manifest = read_json(path)
    cases = manifest.get("cases", [])
    if manifest.get("schema_version") != 1 or not cases:
        raise ValueError("Expected schema_version=1 and a nonempty case inventory")
    ids = [case["id"] for case in cases]
    if not all(isinstance(value, str) and value for value in ids) or len(ids) != len(set(ids)):
        raise ValueError("Invalid or duplicate case IDs")
    for case in cases:
        if case.get("execution", {}).get("status") == "ready":
            if not case.get("command") or not isinstance(case["command"], list) or not all(isinstance(x, str) for x in case["command"]):
                raise ValueError(f"Missing/invalid command: {case['id']}")
            if not case.get("source", {}).get("sha256"):
                raise ValueError(f"Missing input provenance: {case['id']}")
        if case.get("execution", {}).get("status") not in {"ready", "blocked"}:
            raise ValueError(f"Unknown execution state: {case['id']}")
        contained(ROOT, case.get("cwd", "."))
        environment = case.get("environment", {})
        if not isinstance(environment, dict) or not all(isinstance(k, str) and isinstance(v, str) for k, v in environment.items()):
            raise ValueError(f"Invalid environment: {case['id']}")
        forbidden = {"HOME", "TMP", "TEMP", "TMPDIR", "JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS", "_JAVA_OPTIONS", "CLASSPATH"}
        if set(environment) & forbidden:
            raise ValueError(f"Case environment overrides isolation/Java options: {case['id']}")
    return manifest


def tracked_paths(repository: Path) -> list[str]:
    result = subprocess.run(["git", "-C", str(repository), "ls-files", "-z"],
                            check=True, stdout=subprocess.PIPE)
    return [p for p in result.stdout.decode("utf-8").split("\0") if p]


def resource_bytes(path: Path) -> bytes:
    return os.readlink(path).encode("utf-8") if path.is_symlink() else path.read_bytes()


def validate_inputs(root: Path, manifest: dict, staged: Path | None = None) -> dict:
    """Check manifest hashes against both the checkout and the actual staged bytes."""
    staged = staged or root
    repositories = {"pkl-rs": {"path": "."}}
    verified = {}
    references = {}
    if manifest.get("sources"):
        from oracle import read_properties
        references = read_properties((root / "references.lock").read_text(encoding="utf-8"))
        if "baseline" in manifest and manifest["baseline"] != references["baseline.pkl"]:
            raise ValueError("Manifest baseline disagrees with references.lock")
    for source in manifest.get("sources", []):
        name = source["repository"]
        if name in repositories:
            raise ValueError(f"Duplicate source repository: {name}")
        repositories[name] = source
        if any(references.get(f"{name}.{field}") != source[field] for field in ("path", "revision", "url")):
            raise ValueError(f"Source provenance disagrees with references.lock: {name}")
        checkout = contained(root, source["path"])
        revision = subprocess.check_output(["git", "-C", str(checkout), "rev-parse", "HEAD"]).decode().strip()
        if revision != source["revision"]:
            raise ValueError(f"Source revision mismatch: {name}")
        gitlink = subprocess.check_output(["git", "-C", str(root), "ls-tree", "HEAD", source["path"]])
        if not gitlink.startswith(f"160000 commit {source['revision']}\t".encode()):
            raise ValueError(f"Source gitlink mismatch: {name}")
        resources = source["resources"]
        if digest(encoded(resources)) != source["resource_set_sha256"]:
            raise ValueError(f"Resource inventory checksum mismatch: {name}")
        declared = [entry["path"] for entry in resources]
        actual = [f"{source['path']}/{p}" for p in tracked_paths(checkout)]
        if len(declared) != len(set(declared)) or set(declared) != set(actual):
            raise ValueError(f"Tracked source inventory mismatch: {name}")
        for resource in resources:
            validate_resource(root, staged, resource, verified)
    for case in manifest["cases"]:
        if case["execution"]["status"] != "ready":
            continue
        source = case["source"]
        repository = repositories.get(source["repository"])
        if repository is None:
            raise ValueError(f"Missing repository provenance: {case['id']}")
        if source["repository"] != "pkl-rs" and source.get("revision") != repository["revision"]:
            raise ValueError(f"Case revision mismatch: {case['id']}")
        path = (Path(repository["path"]) / source["path"]).as_posix()
        validate_resource(root, staged, {"path": path, "sha256": source["sha256"]}, verified)
        for resource in case.get("resources", []):
            validate_resource(root, staged, resource, verified)
    return {"files": len(verified), "sha256": digest(encoded(verified))}


def validate_resource(root: Path, staged: Path, resource: dict, verified: dict) -> None:
    relative = resource["path"]
    for base in dict.fromkeys((root, staged)):
        path = relative_path(base, relative)
        # Hash symlink text faithfully, but never permit a staged link to escape.
        contained(base, relative)
        data = resource_bytes(path)
        if digest(data) != resource["sha256"]:
            raise ValueError(f"Input/resource checksum mismatch: {relative}")
        if "bytes" in resource and len(data) != resource["bytes"]:
            raise ValueError(f"Input/resource size mismatch: {relative}")
        if "kind" in resource and resource["kind"] != ("symlink" if path.is_symlink() else "file"):
            raise ValueError(f"Input/resource kind mismatch: {relative}")
    if relative in verified and verified[relative] != resource["sha256"]:
        raise ValueError(f"Conflicting resource provenance: {relative}")
    verified[relative] = resource["sha256"]


def prepare_workspace(root: Path, workspace: Path, *, include_project_fixtures: bool = True,
                      workspace_profile: dict | None = None) -> None:
    """Recreate only an empty or explicitly owned generated workspace below target."""
    validate_paths(root, workspace)
    workspace = workspace.resolve()
    identity = {"schema_version": 1, "repository": str(root.resolve()), "workspace": str(workspace)}
    if workspace_profile is not None:
        identity["workspace_profile"] = workspace_profile
    if workspace.exists() and any(workspace.iterdir()):
        if not (workspace / MARKER).is_file() or read_json(workspace / MARKER) != identity:
            raise ValueError("Refusing to replace an unowned nonempty workspace; choose a new path")
        shutil.rmtree(workspace)
    workspace.mkdir(parents=True, exist_ok=True)
    write_json(workspace / MARKER, identity)
    for relative in ("external/pkl", "external/pkl-pantry"):
        source = root / relative
        destination = workspace / relative
        for name in tracked_paths(source):
            src = relative_path(source, name)
            dst = relative_path(destination, name)
            dst.parent.mkdir(parents=True, exist_ok=True)
            if src.is_symlink():
                contained(source, name)
                dst.symlink_to(os.readlink(src), target_is_directory=src.is_dir())
            else:
                shutil.copy2(src, dst)
    fixtures = root / "tests/fixtures"
    if include_project_fixtures and fixtures.exists():
        for path in fixtures.rglob("*"):
            if path.is_symlink():
                contained(root, path.relative_to(root).as_posix())
        shutil.copytree(fixtures, workspace / "tests/fixtures", symlinks=True)


def output_files(directory: Path) -> dict[str, bytes]:
    result = {}
    if directory.is_symlink():
        raise ValueError(f"Unexpected output symlink: {directory}")
    if directory.exists():
        for path in sorted(directory.rglob("*")):
            if path.is_symlink():
                raise ValueError(f"Unexpected output symlink: {path}")
            if path.is_file():
                result[path.relative_to(directory).as_posix()] = path.read_bytes()
    return result


class CaptureTimeout(RuntimeError):
    def __init__(self, timeout: float, result: dict):
        super().__init__(f"Runner timed out after {timeout}s; process group terminated")
        self.result = result


def capture(command: list[str], cwd: Path, env: dict[str, str], timeout: float,
            generated: Path) -> dict:
    if os.name != "posix":
        raise RuntimeError("Process-tree cleanup is currently supported only on POSIX; Windows runner unavailable")
    before = time.perf_counter_ns()
    try:
        process = subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, start_new_session=True)
    except OSError as error:
        raise RuntimeError(f"Runner unavailable: {error}") from error
    timed_out = False
    try:
        try:
            stdout, stderr = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            try:
                stdout, stderr = process.communicate(timeout=5)
            except subprocess.TimeoutExpired as error:
                stdout, stderr = error.output or b"", error.stderr or b""
                process.stdout.close()
                process.stderr.close()
                process.wait(timeout=5)
    finally:
        if not timed_out:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
    result = {"stdout": stdout, "stderr": stderr, "exit_code": process.returncode,
              "files": output_files(generated), "timed_out": timed_out,
              "elapsed_ns": time.perf_counter_ns() - before}
    if timed_out:
        raise CaptureTimeout(timeout, result)
    return result


def store_capture(directory: Path, result: dict) -> dict:
    directory.mkdir(parents=True, exist_ok=True)
    hashes = {}
    for channel in ("stdout", "stderr"):
        (directory / f"{channel}.bin").write_bytes(result[channel])
        hashes[channel] = digest(result[channel])
    for name, data in result["files"].items():
        path = contained(directory / "files", name)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    metadata = {"exit_code": result["exit_code"], "elapsed_ns": result["elapsed_ns"],
                "sha256": hashes, "timed_out": result.get("timed_out", False),
                "workspace_changes": result.get("workspace_changes", {}),
                "files": {name: digest(data) for name, data in result["files"].items()}}
    write_json(directory / "result.json", metadata)
    return metadata


def load_capture(directory: Path) -> dict:
    metadata = read_json(directory / "result.json")
    result = {channel: (directory / f"{channel}.bin").read_bytes()
              for channel in ("stdout", "stderr")}
    for channel, data in result.items():
        if digest(data) != metadata["sha256"][channel]:
            raise ValueError(f"Corrupt expected {channel}: {directory}")
    files = output_files(directory / "files")
    if {name: digest(data) for name, data in files.items()} != metadata["files"]:
        raise ValueError(f"Missing, extra, or corrupt expected files: {directory}")
    return {**result, "files": files, "exit_code": metadata["exit_code"],
            "timed_out": metadata.get("timed_out", False),
            "workspace_changes": metadata.get("workspace_changes", {})}


def differences(expected: dict, actual: dict) -> list[str]:
    changed = []
    for channel in ("stdout", "stderr", "exit_code"):
        if expected[channel] != actual[channel]:
            changed.append(channel)
    for channel in ("workspace_changes", "timed_out"):
        if expected.get(channel, {} if channel == "workspace_changes" else False) != actual.get(channel, {} if channel == "workspace_changes" else False):
            changed.append(channel)
    for name in sorted(set(expected["files"]) | set(actual["files"])):
        if expected["files"].get(name) != actual["files"].get(name):
            changed.append(f"files/{name}")
    return changed


def case_directory(case_id: str) -> str:
    return hashlib.sha256(case_id.encode("utf-8")).hexdigest()[:24]


def snapshot(workspace: Path) -> dict:
    result = {}
    for current, directories, files in os.walk(workspace, followlinks=False):
        if Path(current) == workspace:
            directories[:] = [name for name in directories if name not in PRIVATE]
            files = [name for name in files if name not in PRIVATE]
        for name in sorted(directories + files):
            path = Path(current) / name
            relative = path.relative_to(workspace).as_posix()
            kind = "symlink" if path.is_symlink() else "directory" if path.is_dir() else "file"
            result[relative] = {"kind": kind, "sha256": digest(resource_bytes(path)) if kind != "directory" else None}
    return result


def restore_changes(root: Path, workspace: Path, before: dict, after: dict) -> tuple[dict, dict]:
    changes = {name: {"before": before.get(name), "after": after.get(name)}
               for name in sorted(set(before) | set(after)) if before.get(name) != after.get(name)}
    captured = {}
    for name, transition in changes.items():
        path = relative_path(workspace, name)
        if transition["after"] is not None and transition["after"]["kind"] != "directory":
            captured[f"workspace/{name}"] = resource_bytes(path)
    for name in sorted(changes, key=lambda value: len(Path(value).parts), reverse=True):
        path = relative_path(workspace, name)
        # A changed ancestor link must be removed before restoring descendants;
        # never follow it while deleting the old logical child paths.
        if any(parent.is_symlink() for parent in path.parents if parent != workspace and parent.is_relative_to(workspace)):
            continue
        if path.is_symlink() or path.is_file():
            path.unlink()
        elif path.exists():
            path.rmdir()
    for name, transition in changes.items():
        if transition["before"] is not None:
            path = relative_path(workspace, name)
            source = relative_path(root, name)
            # A preexisting arbitrary untracked input cannot be silently restored.
            if not source.exists() and not source.is_symlink():
                raise ValueError(f"Cannot restore mutated input: {name}")
            path.parent.mkdir(parents=True, exist_ok=True)
            if source.is_symlink():
                path.symlink_to(os.readlink(source), target_is_directory=source.is_dir())
            elif source.is_dir():
                path.mkdir(exist_ok=True)
            else:
                shutil.copy2(source, path)
    return changes, captured


def case_execution(case: dict, prefix: list[str], env: dict[str, str], workspace: Path) -> tuple:
    directory = case_directory(case["id"])
    state = workspace / ".state" / directory
    if state.exists():
        shutil.rmtree(state)
    home, temporary = state / "home", state / "tmp"
    home.mkdir(parents=True)
    temporary.mkdir()
    generated = workspace / "generated" / directory
    if generated.exists():
        shutil.rmtree(generated)
    generated.mkdir(parents=True)
    environment = {**env, **case.get("environment", {}), "HOME": str(home), "TMPDIR": str(temporary),
                   "TMP": str(temporary), "TEMP": str(temporary)}
    command = []
    for arg in prefix:
        if arg.startswith("-Duser.home="):
            arg = f"-Duser.home={home}"
        elif arg.startswith("-Djava.io.tmpdir="):
            arg = f"-Djava.io.tmpdir={temporary}"
        command.append(arg)
    arguments = [arg.replace("{output_dir}", str(generated)) for arg in case["command"]]
    command.extend(arguments)
    # Only explicit case overrides and deterministic controls are safe to log.
    recorded_environment = {key: environment[key] for key in set(case.get("environment", {})) | {"HOME", "TMPDIR", "TMP", "TEMP", "LANG", "LC_ALL", "TZ", "TERM", "NO_COLOR"} if key in environment}
    return command, environment, generated, recorded_environment


def run_cases(manifest: dict, prefix: list[str], env: dict[str, str], workspace: Path,
              destination: Path, timeout: float, jobs: int,
              expected_root: Path | None = None) -> list[dict]:
    def run(case: dict, mutable: bool) -> dict:
        directory = case_directory(case["id"])
        record = {"id": case["id"], "directory": directory}
        if case["execution"]["status"] != "ready":
            return {**record, "status": "blocked", "reason": case["execution"].get("reason", "")}
        try:
            command, environment, generated, recorded_environment = case_execution(case, prefix, env, workspace)
            cwd = contained(workspace, case.get("cwd", "."))
            record.update({"command": command, "cwd": str(cwd), "environment": recorded_environment})
            before = snapshot(workspace) if mutable else None
            if mutable and before != baseline:
                raise ValueError("Workspace inputs differ from the verified initial snapshot")
            timed_out = None
            try:
                result = capture(command, cwd, environment, timeout, generated)
            except CaptureTimeout as error:
                result, timed_out = error.result, str(error)
            except (OSError, ValueError, RuntimeError, subprocess.SubprocessError):
                if mutable:
                    restore_changes(ROOT, workspace, before, snapshot(workspace))
                raise
            result["files"] = {f"generated/{name}": data for name, data in result["files"].items()}
            if mutable:
                changes, secondary = restore_changes(ROOT, workspace, before, snapshot(workspace))
                if snapshot(workspace) != before:
                    raise ValueError("Unable to restore staged inputs after writable case")
                result["workspace_changes"] = changes
                result["files"].update(secondary)
            metadata = store_capture(destination / directory, result)
            record["result"] = metadata
            if timed_out:
                return {**record, "status": "timeout", "reason": timed_out}
            if result["exit_code"] not in case.get("expected_exit", [0]):
                return {**record, "status": "unexpected_oracle_exit" if expected_root is None else "failed",
                        "reason": f"Exit {result['exit_code']}; expected {case.get('expected_exit', [0])}"}
            if expected_root is None:
                return {**record, "status": "recorded"}
            expected = load_capture(expected_root / directory)
            if expected["timed_out"]:
                raise ValueError("Timed-out expected capture cannot be a baseline")
            changed = differences(expected, result)
            return {**record, "status": "failed" if changed else "passed", "differences": changed}
        except (OSError, ValueError, RuntimeError, KeyError, subprocess.SubprocessError) as error:
            return {**record, "status": "environment_error", "reason": str(error)}

    def read_only(case):
        arguments = case.get("command") or []
        return arguments and arguments[0] == "eval" and not any(
            arg in {"-o", "--output-path", "-m", "--multiple-file-output-path"} or
            arg.startswith(("--output-path=", "--multiple-file-output-path=")) for arg in arguments)

    read_cases = [case for case in manifest["cases"] if read_only(case)]
    other_cases = [case for case in manifest["cases"] if not read_only(case)]
    baseline = snapshot(workspace)
    progress_lock = threading.Lock()
    destination.mkdir(parents=True, exist_ok=True)

    def run_and_record(case, mutable):
        record = run(case, mutable)
        with progress_lock:
            with (destination / "progress.jsonl").open("a", encoding="utf-8") as stream:
                stream.write(json.dumps(record, ensure_ascii=False) + "\n")
        return record

    with ThreadPoolExecutor(max_workers=jobs) as pool:
        records = list(pool.map(lambda case: run_and_record(case, False), read_cases))
    after = snapshot(workspace)
    if after != baseline:
        changes, _ = restore_changes(ROOT, workspace, baseline, after)
        for record in records:
            record.update({"status": "environment_error", "reason": "Parallel eval mutated staged inputs",
                           "unexpected_workspace_changes": changes})
    records.extend(run_and_record(case, True) for case in other_cases)
    by_id = {record["id"]: record for record in records}
    return [by_id[case["id"]] for case in manifest["cases"]]


def validate_coverage(manifest: dict, report: dict, accepted: set[str]) -> list[str]:
    expected = {case["id"] for case in manifest["cases"]}
    ids = [result["id"] for result in report.get("results", [])]
    failures = []
    if not expected:
        failures.append("Empty corpus")
    if len(ids) != len(set(ids)) or set(ids) != expected:
        failures.append("Case inventory differs from execution results")
    for result in report.get("results", []):
        if result.get("status") not in accepted:
            failures.append(f"{result['id']}: {result.get('status')}")
    return failures


def check_report(manifest: dict, report: dict, directory: Path, manifest_hash: str) -> list[str]:
    if report.get("schema_version") != 1 or report.get("mode") not in {"oracle-recording", "candidate-comparison"}:
        raise ValueError("Unknown report schema/mode")
    if report["manifest_sha256"] != manifest_hash:
        raise ValueError("Manifest changed since execution")
    accepted = {"recorded"} if report["mode"] == "oracle-recording" else {"passed"}
    failures = validate_coverage(manifest, report, accepted)
    if digest(encoded(read_json(directory / "inputs.json"))) != report["inputs"]["snapshot_sha256"]:
        raise ValueError("Staged input evidence missing or corrupt")
    for record in report.get("results", []):
        if record.get("status") not in accepted and "result" not in record:
            continue
        expected_directory = case_directory(record["id"])
        if record.get("directory") != expected_directory:
            failures.append(f"Invalid capture directory: {record['id']}")
            continue
        try:
            capture_data = load_capture(directory / expected_directory)
            metadata = read_json(directory / expected_directory / "result.json")
            if metadata != record.get("result") or (capture_data["timed_out"] and record.get("status") != "timeout"):
                raise ValueError("Capture metadata changed or timed-out execution was accepted")
        except (OSError, ValueError, KeyError) as error:
            failures.append(f"{record['id']}: {error}")
    return failures


def oracle_identity(metadata: dict) -> dict:
    return {key: metadata[key] for key in ("profile", "lock_sha256", "artifact_sha256", "source", "jdk", "command")}


def runner_provenance(command: list[str], environment: dict, runner_file: Path | None = None) -> dict:
    if not command or not isinstance(command, list) or not all(isinstance(arg, str) for arg in command):
        raise ValueError("Runner command must be a nonempty string array")
    resolved = shutil.which(command[0], path=environment.get("PATH", os.defpath))
    if not resolved:
        raise ValueError(f"Candidate runner executable unavailable: {command[0]}")
    executable = Path(resolved).resolve()
    files = {str(executable): digest(executable.read_bytes())}
    for argument in command[1:]:
        path = Path(argument)
        if path.is_absolute() and path.is_file():
            files[str(path.resolve())] = digest(path.read_bytes())
    return {"command": command, "executable": str(executable), "files_sha256": files,
            "runner_file_sha256": digest(runner_file.read_bytes()) if runner_file else None}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("record", "compare", "check"))
    parser.add_argument("--manifest", type=Path, default=ROOT / "compatibility/corpus.json")
    parser.add_argument("--output", type=Path, default=ROOT / "target/m0/baseline")
    parser.add_argument("--expected", type=Path)
    parser.add_argument("--runner", type=Path, help="JSON file containing a command array for the candidate")
    parser.add_argument("--workspace", type=Path, default=ROOT / "target/m0/workspace")
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--timeout", type=float, default=30)
    args = parser.parse_args()
    try:
        manifest = load_manifest(args.manifest)
        manifest_hash = digest(args.manifest.read_bytes())
        if args.action == "check":
            report = read_json(args.output / "report.json")
            validate_inputs(ROOT, manifest)
            from oracle import ensure_oracle
            if oracle_identity(report["oracle"]) != oracle_identity(ensure_oracle(ROOT)):
                raise ValueError("Recorded oracle identity differs from the current pinned oracle")
            failures = check_report(manifest, report, args.output, manifest_hash)
        else:
            if args.jobs < 1 or not 0 < args.timeout < float("inf"):
                raise ValueError("jobs and timeout must be finite and positive")
            protected = tuple(path for path in (args.manifest, args.runner, args.expected) if path)
            validate_paths(ROOT, args.workspace, args.output, protected)
            if args.expected and overlap(args.output.resolve(), args.expected.resolve()):
                raise ValueError("Actual and expected evidence directories must not overlap")
            if args.output.exists() and any(args.output.iterdir()):
                raise ValueError("Output directory must be empty; choose a new evidence directory")
            validate_inputs(ROOT, manifest)
            from oracle import ensure_oracle
            oracle = ensure_oracle(ROOT)
            prefix = oracle["command"]
            env = oracle["environment"]
            if args.action == "compare":
                if args.expected is None or args.runner is None:
                    raise ValueError("compare requires --expected and --runner")
                previous = read_json(args.expected / "report.json")
                if previous.get("mode") != "oracle-recording" or oracle_identity(previous["oracle"]) != oracle_identity(oracle):
                    raise ValueError("Expected oracle identity differs from the current pinned oracle")
                if previous["workspace"] != str(args.workspace.resolve()):
                    raise ValueError("Comparison requires the same workspace path as the Java oracle")
                failures = check_report(manifest, previous, args.expected, manifest_hash)
                if failures:
                    raise ValueError("Incomplete Java baseline: " + "; ".join(failures[:5]))
                prefix = read_json(args.runner)["command"]
            runner = runner_provenance(prefix, env, args.runner)
            prepare_workspace(ROOT, args.workspace)
            inputs = validate_inputs(ROOT, manifest, args.workspace.resolve())
            staged_snapshot = snapshot(args.workspace.resolve())
            write_json(args.output / "inputs.json", staged_snapshot)
            if args.action == "compare" and read_json(args.expected / "inputs.json") != staged_snapshot:
                raise ValueError("Staged input snapshot differs from Java baseline")
            results = run_cases(manifest, prefix, env, args.workspace.resolve(), args.output,
                                args.timeout, args.jobs, args.expected)
            counts = {status: sum(r["status"] == status for r in results)
                      for status in sorted({r["status"] for r in results})}
            report = {"schema_version": 1, "mode": "oracle-recording" if args.action == "record" else "candidate-comparison",
                      "manifest_sha256": manifest_hash, "workspace": str(args.workspace.resolve()),
                      "inputs": {**inputs, "snapshot_sha256": digest(encoded(staged_snapshot))},
                      "oracle": {k: v for k, v in oracle.items() if k != "environment"},
                      "runner": runner, "execution": {"parallel_read_only_eval_jobs": args.jobs,
                          "writable_cases": "serialized, secondary writes captured and restored",
                          "process_cleanup": "POSIX process groups; Windows unavailable"},
                      "counts": counts, "results": results}
            write_json(args.output / "report.json", report)
            failures = check_report(manifest, report, args.output, manifest_hash)
        print(json.dumps(report.get("counts", {}), sort_keys=True))
        if failures:
            print(f"Incomplete or failing corpus: {len(failures)} issue(s). See {args.output / 'report.json'}", file=sys.stderr)
            return 1
        print("Java evidence recorded; this is not Rust compatibility." if args.action == "record" else "All required cases accounted for.")
        return 0
    except (OSError, ValueError, RuntimeError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(f"Compatibility tooling failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
