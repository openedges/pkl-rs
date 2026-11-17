#!/usr/bin/env python3
"""Bootstrap and verify the development-only Pkl JVM oracle (Python stdlib only)."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile


class OracleError(RuntimeError):
    """The requested pinned oracle cannot be verified."""


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_properties(data):
    result = {}
    for line in data.splitlines():
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            raise OracleError("invalid key=value metadata")
        key, value = line.split("=", 1)
        if key in result:
            raise OracleError(f"duplicate metadata key: {key}")
        result[key] = value
    return result


def _run(command, **kwargs):
    try:
        return subprocess.run(command, capture_output=True, timeout=60, **kwargs)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise OracleError(f"cannot execute {command[0]}: {error}") from error


def verify_sources(root, lock):
    refs = read_properties((root / "references.lock").read_text(encoding="utf-8"))
    source = lock["source"]
    expected = {
        "baseline.pkl": lock["version"],
        "pkl.path": source["path"],
        "pkl.revision": source["revision"],
        "pkl.version": lock["version"],
        "pkl.url": source["url"],
    }
    if any(refs.get(key) != value for key, value in expected.items()):
        raise OracleError("references.lock disagrees with oracle.lock.json")
    checkout = root / source["path"]
    git_env = {"PATH": os.defpath, "LC_ALL": "C"}
    head = _run(["git", "-C", str(checkout), "rev-parse", "HEAD"], env=git_env)
    if head.returncode or head.stdout.decode().strip() != source["revision"]:
        raise OracleError("Pkl source checkout is missing or at the wrong revision")
    gitlink = _run(["git", "-C", str(root), "ls-tree", "HEAD", source["path"]], env=git_env)
    if gitlink.returncode or not gitlink.stdout.startswith(
        f"160000 commit {source['revision']}\t".encode()
    ):
        raise OracleError("Pkl gitlink disagrees with oracle source revision")
    dirty = _run(["git", "-C", str(checkout), "status", "--porcelain", "--untracked-files=all"], env=git_env)
    if dirty.returncode or dirty.stdout:
        raise OracleError("Pkl reference checkout is dirty")
    for path, expected_hash in source["metadata_sha256"].items():
        if sha256(checkout / path) != expected_hash:
            raise OracleError(f"Pkl source metadata checksum mismatch: {path}")


def verify_artifact(path, lock):
    artifact = lock["artifact"]
    if not path.is_file():
        raise OracleError(f"oracle artifact missing: {path}; run tools/oracle.py fetch")
    if sha256(path) != artifact["sha256"]:
        raise OracleError(f"oracle artifact checksum mismatch: {path}")
    try:
        with zipfile.ZipFile(path) as archive:
            release = read_properties(archive.read("org/pkl/core/Release.properties").decode("utf-8"))
            for entry in ("org/pkl/cli/Main.class", "org/pkl/core/Evaluator.class", "org/pkl/server/Server.class"):
                archive.getinfo(entry)
    except (OSError, KeyError, zipfile.BadZipFile, UnicodeError) as error:
        raise OracleError(f"invalid oracle archive: {error}") from error
    if release.get("version") != lock["version"] or release.get("commitId") != artifact["commit_id"]:
        raise OracleError("oracle embedded release metadata disagrees with lock")
    return release


def fetch_artifact(path, lock):
    # Never silently replace an existing corrupt artifact.
    if path.exists():
        verify_artifact(path, lock)
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as output:
            temporary = Path(output.name)
            with urllib.request.urlopen(lock["artifact"]["url"], timeout=60) as response:
                if not response.geturl().startswith("https://"):
                    raise OracleError("oracle download redirected outside HTTPS")
                shutil.copyfileobj(response, output)
        verify_artifact(temporary, lock)
        temporary.replace(path)
    except (OSError, ValueError) as error:
        raise OracleError(f"oracle download failed: {error}") from error
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def deterministic_environment(root):
    """Return an allowlist, never inherited secrets or Java option injection."""
    cache = root / ".cache/oracle"
    for name in ("home", "tmp", "packages"):
        (cache / name).mkdir(parents=True, exist_ok=True)
    return {
        "PATH": os.defpath,
        "HOME": str(cache / "home"),
        "TMPDIR": str(cache / "tmp"),
        "TMP": str(cache / "tmp"),
        "TEMP": str(cache / "tmp"),
        "LC_ALL": "C",
        "LANG": "C",
        "TZ": "UTC",
        "TERM": "dumb",
        "NO_COLOR": "1",
        "COLUMNS": "80",
    }


def verify_java(java, lock, environment):
    identity = _run([str(java), "-XshowSettings:properties", "-version"], env=environment)
    if identity.returncode:
        raise OracleError("Java identity command failed")
    properties = dict(re.findall(r"^\s+([\w.]+) = (.*)$", identity.stderr.decode("utf-8"), re.MULTILINE))
    allow_patch = os.environ.get("PKL_ALLOW_LOCAL_JDK") == "1"
    exact_jdk_lock_match = True
    for key, expected in lock["jdk"]["properties"].items():
        actual = properties.get(key)
        if actual != expected:
            if allow_patch and properties.get("java.vendor") == "Eclipse Adoptium" and actual and actual.startswith("21.0."):
                exact_jdk_lock_match = False
                continue
            raise OracleError(f"Java identity mismatch: {key}: expected {expected!r}, got {actual!r}")
    home = Path(properties["java.home"])
    modules_path = home / "lib/modules"
    release_path = home / "release"
    return {
        "executable": str(java),
        "executable_sha256": sha256(java) if Path(java).is_file() else None,
        "runtime_modules_sha256": sha256(modules_path) if modules_path.is_file() else None,
        "release_sha256": sha256(release_path) if release_path.is_file() else None,
        "exact_jdk_lock_match": exact_jdk_lock_match,
        "properties": {key: properties[key] for key in (
            "java.vendor", "java.version", "java.runtime.version", "java.vm.name",
            "java.vm.version", "java.home", "os.name", "os.arch", "os.version",
        )},
        "version_stdout_hex": identity.stdout.hex(),
        "version_stderr_hex": identity.stderr.hex(),
    }


def ensure_performance_jdk(root, lock, fetch):
    configuration = lock["performance_jdk"]
    target = f"{platform.system()}-{platform.machine()}"
    if target not in configuration["distributions"]:
        raise OracleError(f"performance JDK distribution is not pinned for {target}")
    distribution = configuration["distributions"][target]
    cache = root / ".cache/oracle"
    archive = cache / distribution["filename"]
    java = cache / distribution["java_path"]
    if fetch:
        cache.mkdir(parents=True, exist_ok=True)
        if not archive.exists():
            temporary = None
            try:
                with tempfile.NamedTemporaryFile(dir=cache, delete=False) as output:
                    temporary = Path(output.name)
                    with urllib.request.urlopen(distribution["url"], timeout=60) as response:
                        shutil.copyfileobj(response, output)
                if sha256(temporary) != distribution["sha256"]:
                    raise OracleError("performance JDK archive checksum mismatch")
                temporary.replace(archive)
            finally:
                if temporary:
                    temporary.unlink(missing_ok=True)
        if sha256(archive) != distribution["sha256"]:
            raise OracleError("performance JDK archive checksum mismatch")
        if not java.exists():
            with tarfile.open(archive, "r:gz") as bundle:
                for member in bundle.getmembers():
                    path = Path(member.name)
                    if path.is_absolute() or ".." in path.parts:
                        raise OracleError("unsafe performance JDK archive path")
                    if member.issym() or member.islnk():
                        link = Path(member.linkname)
                        parent = (cache / path).parent if member.issym() else cache
                        if link.is_absolute() or not (parent / link).resolve().is_relative_to(cache):
                            raise OracleError("unsafe performance JDK archive link")
                # Explicit validation above also supports Python versions without filters.
                if hasattr(tarfile, "data_filter"):
                    bundle.extractall(cache, filter="data")
                else:
                    bundle.extractall(cache)
    if not archive.is_file() or sha256(archive) != distribution["sha256"]:
        raise OracleError("performance JDK archive missing/corrupt; run fetch --profile performance")
    if not java.is_file():
        raise OracleError("performance JDK executable missing; run fetch --profile performance")
    for relative, expected_hash in distribution["runtime_sha256"].items():
        if sha256(java.parent.parent / relative) != expected_hash:
            raise OracleError(f"performance JDK runtime checksum mismatch: {relative}")
    return java.resolve(), configuration, distribution


def ensure_oracle(root, fetch=False, profile="reference"):
    """Verify source, artifact and Java identity; return JSON-compatible provenance.

    `command` is the complete prefix ending in org.pkl.cli.Main. Callers append
    CLI arguments and execute with `environment` and an explicit working directory.
    No network access occurs unless fetch=True and the artifact is absent.
    """
    root = Path(root).resolve()
    lock_path = root / "oracle.lock.json"
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    if lock.get("schema") != 1:
        raise OracleError("unsupported oracle lock schema")
    verify_sources(root, lock)
    artifact = root / ".cache/oracle" / lock["artifact"]["filename"]
    if fetch:
        fetch_artifact(artifact, lock)
    release = verify_artifact(artifact, lock)
    if profile not in ("reference", "performance"):
        raise OracleError(f"unknown oracle profile: {profile}")
    jdk_lock = lock
    distribution = None
    if profile == "performance":
        executable, configuration, distribution = ensure_performance_jdk(root, lock, fetch)
        jdk_lock = {"jdk": configuration}
    else:
        configured = os.environ.get("PKL_ORACLE_JAVA")
        executable = configured or shutil.which("java")
    if not executable:
        raise OracleError("Java unavailable; set PKL_ORACLE_JAVA to the pinned JDK executable")
    java = Path(executable).expanduser().resolve()
    environment = deterministic_environment(root)
    identity = verify_java(java, jdk_lock, environment)
    if distribution:
        identity["distribution"] = distribution
    options = lock["jvm_options"] + (configuration.get("jvm_options", []) if profile == "performance" else []) + [
        f"-Duser.home={environment['HOME']}",
        f"-Djava.io.tmpdir={environment['TMPDIR']}",
    ]
    command = [str(java), *options, "-cp", str(artifact), "org.pkl.cli.Main"]
    version = _run([*command, "--version"], env=environment, cwd=root)
    if version.returncode or version.stderr or not version.stdout.startswith(f"Pkl {lock['version']} (".encode()):
        raise OracleError("oracle runtime version check failed")
    if profile == "performance":
        probe_command = [str(java), *options, "-cp", str(artifact), str(root / "tools/OracleRuntimeProbe.java")]
        probe = _run(probe_command, env=environment, cwd=root)
        if probe.returncode or probe.stderr or probe.stdout != (configuration["truffle_runtime_name"] + "\n").encode():
            raise OracleError("optimizing Truffle runtime probe failed")
        identity["optimizing_runtime_probe"] = {"command": probe_command,
            "stdout_hex": probe.stdout.hex(), "stderr_hex": probe.stderr.hex(),
            "source_sha256": sha256(root / "tools/OracleRuntimeProbe.java")}
    return {
        "schema": 1,
        "profile": profile,
        "version": lock["version"],
        "lock_sha256": sha256(lock_path),
        "source": lock["source"],
        "artifact": str(artifact),
        "artifact_sha256": lock["artifact"]["sha256"],
        "artifact_url": lock["artifact"]["url"],
        "release": release,
        "java": str(java),
        "jdk": identity,
        "exact_jdk_lock_match": identity.get("exact_jdk_lock_match", True),
        "command": command,
        "environment": environment,
        "platform": platform.platform(),
        "version_stdout_hex": version.stdout.hex(),
        "version_stderr_hex": version.stderr.hex(),
    }


def smoke(root, metadata):
    """Record raw successful/error executions and exact replay, not Rust compatibility."""
    destination = root / "target/m0/oracle"
    if metadata.get("profile") == "performance":
        destination /= "performance"
    work = destination / "work"
    work.mkdir(parents=True, exist_ok=True)
    inputs = {
        "success": b'answer = 42\nmessage = "oracle"\n',
        "error": b'answer = missingOracleSmokeName\n',
    }
    results = []
    for name, source in inputs.items():
        module = work / f"{name}.pkl"
        module.write_bytes(source)
        command = [*metadata["command"], "eval", "--no-project", "--color=never", module.name]
        first = _run(command, cwd=work, env=metadata["environment"])
        second = _run(command, cwd=work, env=metadata["environment"])
        for suffix, value in (("stdout.bin", first.stdout), ("stderr.bin", first.stderr)):
            (destination / f"{name}.{suffix}").write_bytes(value)
        replay_equal = (first.returncode, first.stdout, first.stderr) == (second.returncode, second.stdout, second.stderr)
        if name == "success":
            valid = first.returncode == 0 and first.stdout == inputs["success"] and first.stderr == b""
        else:
            valid = first.returncode == 1 and first.stdout == b"" and bool(first.stderr)
        results.append({"case_id": f"oracle-smoke-{name}", "command": command,
                        "cwd": str(work), "input_sha256": sha256(module),
                        "exit_code": first.returncode, "stdout_sha256": hashlib.sha256(first.stdout).hexdigest(),
                        "stderr_sha256": hashlib.sha256(first.stderr).hexdigest(),
                        "replay_equal": replay_equal, "passed": valid and replay_equal})
    report = {"oracle": metadata, "scope": "JVM oracle bootstrap and exact replay only", "results": results}
    (destination / "smoke.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if not all(result["passed"] for result in results):
        raise OracleError(f"oracle smoke failed; see {destination / 'smoke.json'}")
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("fetch", "verify", "smoke"))
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--profile", choices=("reference", "performance"), default="reference")
    args = parser.parse_args()
    try:
        metadata = ensure_oracle(args.root, fetch=args.action == "fetch", profile=args.profile)
        destination = args.root.resolve() / "target/m0/oracle"
        destination.mkdir(parents=True, exist_ok=True)
        filename = "provenance.json" if args.profile == "reference" else "performance-provenance.json"
        (destination / filename).write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
        if args.action == "smoke":
            smoke(args.root.resolve(), metadata)
        print(f"Pkl {metadata['version']} JVM oracle verified; provenance: {destination}")
        return 0
    except (OracleError, OSError, ValueError, KeyError) as error:
        print(f"oracle: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
