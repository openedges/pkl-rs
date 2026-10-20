#!/usr/bin/env python3
"""Record Java Pantry package/API tests and generated-package evaluation (stdlib only)."""
from __future__ import annotations
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import signal
import subprocess
import sys
from urllib.parse import urlsplit
import zipfile

import discover
import oracle

ROOT = Path(__file__).resolve().parents[1]


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, data) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(discover.encoded(data))


def capture(command, cwd, environment, destination: Path, timeout=180):
    destination.mkdir(parents=True, exist_ok=True)
    process = subprocess.Popen(command, cwd=cwd, env=environment, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
        code, status = process.returncode, "executed"
    except subprocess.TimeoutExpired:
        if os.name == "posix":
            os.killpg(process.pid, signal.SIGKILL)
        else:
            process.kill()
        stdout, stderr = process.communicate()
        code, status = None, "timeout"
    (destination / "stdout.bin").write_bytes(stdout)
    (destination / "stderr.bin").write_bytes(stderr)
    result = {"command": [str(x) for x in command], "cwd": str(cwd), "exit_code": code, "status": status,
              "stdout": {"path": str(destination / "stdout.bin"), "sha256": sha(destination / "stdout.bin"), "bytes": len(stdout)},
              "stderr": {"path": str(destination / "stderr.bin"), "sha256": sha(destination / "stderr.bin"), "bytes": len(stderr)}}
    write_json(destination / "result.json", result)
    return result


def census(directory):
    return [{"path": str(p.relative_to(directory)), "sha256": sha(p), "bytes": p.stat().st_size}
            for p in sorted(directory.rglob("*")) if p.is_file()]


def seed_package(metadata_path: Path, cache: Path):
    """Verify generated artifacts before copying into upstream's package-2 cache."""
    metadata = json.loads(metadata_path.read_bytes())
    archive = metadata_path.with_name(metadata_path.name + ".zip")
    expected_metadata = metadata_path.with_name(metadata_path.name + ".sha256").read_text()
    expected_archive = archive.with_name(archive.name + ".sha256").read_text()
    # Upstream files contain exactly the digest; never trim content to validate.
    if sha(metadata_path) != expected_metadata or sha(archive) != expected_archive:
        raise ValueError(f"generated package checksum mismatch: {metadata_path}")
    if sha(archive) != metadata["packageZipChecksums"]["sha256"]:
        raise ValueError(f"metadata ZIP checksum mismatch: {metadata_path}")
    uri = metadata["packageUri"]
    parsed = urlsplit(uri)
    if parsed.scheme != "package" or parsed.netloc != "pkg.pkl-lang.org" or not parsed.path.startswith("/pkl-pantry/"):
        raise ValueError(f"unexpected Pantry package URI: {uri}")
    if any(part in (".", "..") for part in parsed.path.split("/")):
        raise ValueError("unsafe package cache path")
    target = cache / "package-2" / parsed.netloc / parsed.path.lstrip("/")
    target.mkdir(parents=True, exist_ok=True)
    name = parsed.path.rsplit("/", 1)[1]
    for source, destination in ((metadata_path, target / (name + ".json")), (archive, target / (name + ".zip"))):
        if destination.exists():
            if sha(destination) != sha(source):
                raise ValueError(f"generated packages disagree: {destination}")
            continue
        shutil.copyfile(source, destination)
    with zipfile.ZipFile(archive) as zipped:
        modules = sorted(n for n in zipped.namelist() if n.endswith(".pkl"))
    return {"uri": uri, "metadata_sha256": sha(metadata_path), "archive_sha256": sha(archive),
            "modules": [uri + "#/" + path for path in modules]}


def invalid_observation_processes(report):
    records = report.get("packaged_evaluations", []) + report.get("packaged_multi_file_evaluations", [])
    return [record for record in records if record.get("status") != "executed" or record.get("exit_code") not in (0, 1)]


def required_package_cases(manifest):
    sources = [source for source in manifest.get("sources", []) if source.get("repository") == "pkl-pantry"]
    if len(sources) != 1:
        raise ValueError("Pantry source census is missing or ambiguous")
    prefix = sources[0]["path"] + "/"
    expected = {r["path"][len(prefix):] for r in sources[0]["resources"]
                if r["path"].startswith(prefix + "packages/")
                and len(r["path"][len(prefix):].split("/")) == 3
                and r["path"].endswith("/PklProject")}
    cases = [case for case in manifest.get("cases", []) if case.get("suite") == "package-api-tests"]
    actual = {case["source"]["path"] for case in cases}
    if not expected or actual != expected or len(cases) != len(actual):
        raise ValueError("Pantry package case set is empty, duplicated, or incomplete against the tracked project census")
    return cases


def add_tar_shell_evidence(report, metadata, destination):
    folder = destination / "tar-shell-pipefail"
    work = folder / "work"
    shutil.copytree(ROOT / "external/pkl-pantry", work, ignore=shutil.ignore_patterns(".git", "build", ".gradle"))
    binaries = folder / "bin"
    binaries.mkdir(parents=True)
    wrapper = binaries / "pkl"
    arguments = metadata["command"]
    wrapper.write_text(f"#!{sys.executable}\nimport os, sys\nos.execv({arguments[0]!r}, {arguments!r} + sys.argv[1:] + ['--cache-dir', {str(destination / 'cache')!r}])\n")
    wrapper.chmod(0o755)
    environment = dict(metadata["environment"])
    environment["PATH"] = str(binaries) + ":" + environment["PATH"]
    identities = []
    for name in ("bash", "tar"):
        executable = shutil.which(name, path=environment["PATH"])
        if not executable:
            raise ValueError(f"tar shell suite requires {name}")
        identity = capture([executable, "--version"], work, environment, folder / (name + "-identity"))
        identity["executable_sha256"] = sha(Path(executable).resolve())
        identities.append(identity)
    result = capture([shutil.which("bash", path=environment["PATH"]), "-e", "-o", "pipefail", str(work / "packages/pkl.tar/examples/test_examples.sh")],
        work, environment, folder / "process")
    result["pipeline_contract"] = "Upstream script unchanged; bash -e -o pipefail detects failure of every pipeline stage."
    result["host_identities"] = identities
    result["outcome"] = "java-success" if result["exit_code"] == 0 else "java-failure"
    result["script_sha256"] = sha(work / "packages/pkl.tar/examples/test_examples.sh")
    result["source_copy"] = str(work)
    result["wrapper_sha256"] = sha(wrapper)
    report["tar_shell_suite"] = result
    report["summary"]["tar_shell_success"] = result["exit_code"] == 0


def add_multi_file_evaluations(report, metadata, destination, workers=4):
    candidates = []
    for result in report["packaged_evaluations"]:
        stderr_path = Path(result["stderr"]["path"])
        if sha(stderr_path) != result["stderr"]["sha256"]:
            raise ValueError(f"recorded stderr changed: {stderr_path}")
        raw = stderr_path.read_bytes()
        if b"multiple-file output" in raw or b"`-m` flag" in raw:
            candidates.append(result["module_uri"])
    def evaluate(item):
        index, uri = item
        folder = destination / "packaged-multi-file-eval" / str(index)
        outputs = folder / "files"
        command = [*metadata["command"], "eval", "--no-project", "--settings", "pkl:settings",
            "--cache-dir", str(destination / "cache"), "--property", "pkl-pantry.testMode=1",
            "--multiple-file-output-path", str(outputs), uri]
        result = capture(command, destination, metadata["environment"], folder)
        result.update({"module_uri": uri, "expected_outcome": "observe", "mode": "multiple-file-output",
            "selection_reason": "Raw single-output diagnostic explicitly requires multiple-file output.",
            "artifacts": census(outputs) if outputs.exists() else [],
            "outcome": "java-success" if result["exit_code"] == 0 else "java-error-observed"})
        write_json(folder / "result.json", result)
        return result
    with ThreadPoolExecutor(max_workers=workers) as executor:
        evaluations = list(executor.map(evaluate, enumerate(sorted(candidates))))
    report["packaged_multi_file_evaluations"] = evaluations
    report["summary"]["packaged_multi_file_commands"] = len(evaluations)
    report["summary"]["packaged_multi_file_success"] = sum(r["exit_code"] == 0 for r in evaluations)
    report["summary"]["packaged_multi_file_error_observed"] = sum(r["exit_code"] != 0 for r in evaluations)


def add_configured_generator_evaluations(report, metadata, destination):
    specifications = [
        ("k8s.contrib.crd", "packages/k8s.contrib.crd/tests/fixtures/crds.yaml"),
        ("org.json_schema.contrib", "packages/org.json_schema.contrib/tests/fixtures/test_allOf.json"),
    ]
    results = []
    for index, (package_name, fixture_path) in enumerate(specifications):
        uri = next((package["uri"] + "#/generate.pkl" for package in report["generated_packages"]
                    if package["uri"].rsplit("/", 1)[1].split("@", 1)[0] == package_name), None)
        if uri is None:
            raise ValueError(f"generated package missing: {package_name}")
        folder = destination / "configured-generators" / str(index)
        folder.mkdir(parents=True)
        fixture = folder / Path(fixture_path).name
        shutil.copyfile(ROOT / "external/pkl-pantry" / fixture_path, fixture)
        outputs = folder / "files"
        command = [*metadata["command"], "eval", "--no-project", "--settings", "pkl:settings",
            "--cache-dir", str(destination / "cache"), "--property", "pkl-pantry.testMode=1",
            "--property", "source=" + fixture.as_uri(), "--multiple-file-output-path", str(outputs), uri]
        result = capture(command, destination, metadata["environment"], folder / "process")
        result.update({"module_uri": uri, "expected_outcome": "success", "fixture_source": fixture_path,
            "fixture_sha256": sha(fixture), "artifacts": census(outputs) if outputs.exists() else [],
            "outcome": "java-success" if result["exit_code"] == 0 else "java-failure"})
        results.append(result)
    report["configured_generator_evaluations"] = results
    report["summary"]["configured_generator_commands"] = len(results)
    report["summary"]["configured_generator_success"] = sum(r["exit_code"] == 0 for r in results)
    report["summary"]["configured_generator_failure"] = sum(r["exit_code"] != 0 for r in results)


def add_observed_error_reasons(report):
    categories = {}
    for result in report["packaged_evaluations"] + report.get("packaged_multi_file_evaluations", []):
        if result["exit_code"] == 0:
            continue
        raw = Path(result["stderr"]["path"]).read_bytes()
        if result["exit_code"] is None or result["exit_code"] < 0:
            category = "environment-timeout-or-signal"
        elif b"value is undefined" in raw or b"One of the following properties must be set" in raw or b"Cannot find resource `prop:" in raw:
            category = "requires-user-configuration"
        elif b"Cannot invoke abstract" in raw:
            category = "abstract-template"
        elif b"Type constraint" in raw:
            category = "default-configuration-violates-constraint"
        elif b"multiple-file output" in raw or b"`-m` flag" in raw:
            category = "requires-multiple-file-output"
        elif b"no expression passed" in raw:
            category = "requires-cli-expression"
        else:
            category = "unclassified-oracle-error"
        result["observed_error_category"] = category
        result["review_status"] = "provisional-observation-not-an-accepted-test-expectation"
        categories[category] = categories.get(category, 0) + 1
    report["summary"]["observed_error_categories"] = categories


def add_reporter_counts(report):
    """Count successful CLI reporter summaries; retain raw files for comparison.

    Dependency closures repeat suites, so these are invocation totals, never a
    count of distinct test definitions or distinct Pkl assertions.
    """
    test_total = assertion_total = summaries = 0
    for result in report["package_cases"]:
        raw_path = Path(result["stdout"]["path"])
        if sha(raw_path) != result["stdout"]["sha256"]:
            raise ValueError(f"recorded stdout changed: {raw_path}")
        rows = re.findall(rb"100\.0% tests pass \[(\d+) passed\], 100\.0% asserts pass \[(\d+) passed\]", raw_path.read_bytes())
        result["successful_reporter_summaries"] = [{"tests_passed": int(tests), "assertions_passed": int(assertions)}
                                                     for tests, assertions in rows]
        test_total += sum(int(tests) for tests, _ in rows)
        assertion_total += sum(int(assertions) for _, assertions in rows)
        summaries += len(rows)
    report["summary"]["successful_reporter_summaries"] = summaries
    report["summary"]["reported_test_invocations_including_dependency_repeats"] = test_total
    report["summary"]["reported_assertion_invocations_including_dependency_repeats"] = assertion_total
    report["summary"]["distinct_assertion_count"] = None


def run(root=ROOT, workers=4):
    manifest_bytes = (root / "compatibility/corpus.json").read_bytes()
    manifest = json.loads(manifest_bytes)
    cases = required_package_cases(manifest)
    if discover.encoded(discover.build_manifest(root)) != manifest_bytes:
        raise ValueError("corpus inventory is stale; run tools/discover.py before recording Pantry")
    metadata = oracle.ensure_oracle(root)
    destination = root / "target/m0/pantry"
    if destination.exists():
        raise ValueError(f"refusing to overwrite evidence: {destination}; move it before a fresh run")
    destination.mkdir(parents=True)
    (destination / "manifest.json").write_bytes(manifest_bytes)
    write_json(destination / "oracle.json", metadata)
    cache = destination / "cache"
    cache.mkdir()
    environment = metadata["environment"]
    base = metadata["command"]
    preparations = []
    remote = {}
    for lock in manifest["dependency_lockfiles"]:
        for dependency in lock["resolved_dependencies"].values():
            if dependency["type"] == "remote":
                uri = dependency["uri"].replace("projectpackage://", "package://", 1)
                remote[uri] = dependency["checksums"]["sha256"]
    for index, (uri, checksum) in enumerate(sorted(remote.items())):
        preparations.append(capture([*base, "download-package", "--cache-dir", str(cache), uri + "::sha256:" + checksum],
            root, environment, destination / "dependencies" / str(index)))
    source = root / "external/pkl-pantry"
    def package_case(item):
        index, case = item
        folder = destination / "packages" / str(index)
        work = folder / "work"
        shutil.copytree(source, work / "external/pkl-pantry", ignore=shutil.ignore_patterns(".git", "build", ".gradle"))
        outputs = folder / "generated"
        command = [arg.replace("{output_dir}", str(outputs)) for arg in case["command"]]
        command += ["--cache-dir", str(cache), "--allowed-resources", "file:,pkl:,https:,prop:,package:,projectpackage:"]
        result = capture([*base, *command], work, environment, folder / "process")
        result.update({"id": case["id"], "artifacts": census(outputs) if outputs.exists() else [],
                       "outcome": "java-success" if result["exit_code"] == 0 else "java-failure"})
        write_json(folder / "result.json", result)
        return result
    with ThreadPoolExecutor(max_workers=workers) as executor:
        package_results = list(executor.map(package_case, enumerate(cases)))
    generated = {}
    seed_errors = []
    for file in sorted((destination / "packages").glob("*/generated/**/*")):
        if not file.is_file() or file.suffix in (".sha256", ".zip"):
            continue
        try:
            item = seed_package(file, cache)
            if item["uri"] in generated and generated[item["uri"]] != item:
                raise ValueError(f"generated package variants disagree: {item['uri']}")
            generated[item["uri"]] = item
        except (OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
            seed_errors.append({"path": str(file), "reason": str(error)})
    modules = sorted({module for package in generated.values() for module in package["modules"]})
    if not modules:
        raise ValueError("generated packages contain no Pkl module cases")
    def evaluate(item):
        index, uri = item
        folder = destination / "packaged-eval" / str(index)
        # No project scope: module lookup is through the generated ZIP and metadata.
        command = [*base, "eval", "--no-project", "--settings", "pkl:settings", "--cache-dir", str(cache),
                   "--property", "pkl-pantry.testMode=1", uri]
        result = capture(command, destination, environment, folder)
        result.update({"module_uri": uri, "expected_outcome": "observe",
            "outcome": "java-success" if result["exit_code"] == 0 else "java-error-observed",
            "limitation": "Public/template modules can be abstract or need user configuration; an error is an observed baseline, never compatibility success."})
        write_json(folder / "result.json", result)
        return result
    with ThreadPoolExecutor(max_workers=workers) as executor:
        evaluations = list(executor.map(evaluate, enumerate(modules)))
    expected_projects = len(cases)
    report = {"schema_version": 1, "scope": "Java development-tree package/API tests and generated-ZIP module evaluation; no Rust compatibility comparison",
              "manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(), "oracle_lock_sha256": sha(root / "oracle.lock.json"),
              "dependency_preparations": preparations, "package_cases": package_results,
              "generated_packages": list(generated.values()), "seed_errors": seed_errors,
              "packaged_evaluations": evaluations, "cache_inputs": census(cache),
              "summary": {"required_project_packages": expected_projects, "attempted_project_packages": len(package_results),
                  "package_success": sum(r["exit_code"] == 0 for r in package_results),
                  "package_failure": sum(r["exit_code"] != 0 for r in package_results),
                  "generated_packages": len(generated), "packaged_module_commands": len(evaluations),
                  "packaged_module_success": sum(r["exit_code"] == 0 for r in evaluations),
                  "packaged_module_error_observed": sum(r["exit_code"] != 0 for r in evaluations),
                  "compatibility_passes": 0, "assertion_count": None}}
    add_tar_shell_evidence(report, metadata, destination)
    add_multi_file_evaluations(report, metadata, destination, workers)
    add_configured_generator_evaluations(report, metadata, destination)
    add_reporter_counts(report)
    add_observed_error_reasons(report)
    report["cache_inputs"] = census(cache)
    write_json(destination / "report.json", report)
    print(json.dumps(report["summary"], sort_keys=True), flush=True)
    return int(bool(seed_errors) or any(r["exit_code"] != 0 for r in preparations + package_results)
               or len(generated) != expected_projects or not report["summary"]["tar_shell_success"]
               or report["summary"]["configured_generator_failure"] != 0 or bool(invalid_observation_processes(report)))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workers", type=int, default=4)
    args = parser.parse_args()
    if not 1 <= args.workers <= 8:
        parser.error("--workers must be between 1 and 8")
    try:
        return run(workers=args.workers)
    except (OSError, ValueError, oracle.OracleError) as error:
        print(f"Pantry recording failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
