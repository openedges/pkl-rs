#!/usr/bin/env python3
"""Deterministic, conservative inventory of the pinned reference corpus (stdlib only)."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import posixpath
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "compatibility/corpus.json"
REV1 = ROOT / "compatibility/rev1-inventory.json"
SNIPPETS = "pkl-core/src/test/files/LanguageSnippetTests/"
FORMATTER = "pkl-formatter/src/test/files/FormatterSnippetTests/"
ENVIRONMENT = {"LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "TZ": "UTC", "TERM": "dumb", "NO_COLOR": "1"}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encoded(value: object) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode()


def git(repo: Path, *args: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(repo), *args])


def tracked(repo: Path) -> list[str]:
    return sorted(p.decode() for p in git(repo, "ls-files", "-z").split(b"\0") if p)


def locks() -> dict[str, str]:
    return dict(line.split("=", 1) for line in (ROOT / "references.lock").read_text().splitlines()
                if line and not line.startswith("#"))


def golden_paths(path: str, available: set[str]) -> list[str]:
    relative = path.removeprefix(SNIPPETS + "input/")
    # AbstractLanguageSnippetTestsEngine.expectedOutputFileFor: a hidden extension
    # selects the renderer format; an ordinary .pkl selects .pcf.
    output = relative[:-4] if re.fullmatch(r".*[.]([^.]*)[.]pkl", relative) else relative[:-3] + "pcf"
    stdout = SNIPPETS + "output/" + output
    error = stdout.rsplit(".", 1)[0] + ".err"
    return [p for p in (stdout, error) if p in available]


def snippet_block(path: str, text: str) -> str | None:
    relative = path.removeprefix(SNIPPETS + "input/")
    if relative.startswith("native/"):
        return "Upstream JVM suite excludes native/; requires the native-only module host contract."
    if relative.startswith(("packages/", "projects/")):
        return "Requires upstream PackageServer, generated TLS certificate/test-port, and project fixture settings."
    if "modulepath:" in text or "import1b.pkl" in path:
        return "Requires upstream JVM test classpath/modulepath fixture adapter."
    if any(token in text for token in ("https://localhost", "http://localhost", "https://127.0.0.1", "package://", "projectpackage://")):
        return "Requires pinned package/HTTP fixture server and generated test TLS certificate."
    return None


def source_ref(name: str, revision: str, path: str, sha256: str) -> dict:
    return {"repository": name, "revision": revision, "path": path, "sha256": sha256}


def case(name: str, revision: str, path: str, sha256: str, suite: str,
         command: list[str], reason: str | None = None) -> dict:
    return {"id": f"{name}:{suite}:{path}", "source": source_ref(name, revision, path, sha256),
            "suite": suite, "command": command, "cwd": ".", "expected_exit": [0],
            "expected_outcome": "success", "channels": ["stdout", "stderr", "exit_code", "files"],
            "target": ["native", "wasm", "napi"], "environment": dict(ENVIRONMENT),
            "host_requirements": ["filesystem", "source-tree-resource-set"], "resources": [],
            "execution": {"status": "blocked" if reason else "ready", "reason": reason},
            "assertion_count": None,
            "assertion_count_reason": "Requires executing the suite; source files are not assertions."}


def local_project_closure(package: str, dependency_locks: dict[str, dict]) -> list[str]:
    selected = set()
    pending = [package]
    while pending:
        current = pending.pop()
        if current in selected:
            continue
        selected.add(current)
        for dependency in dependency_locks.get(current, {}).get("resolvedDependencies", {}).values():
            if dependency.get("type") == "local":
                resolved = posixpath.normpath(posixpath.join(current, dependency["path"]))
                if not resolved.startswith("packages/"):
                    raise ValueError(f"local Pantry dependency escapes packages/: {resolved}")
                pending.append(resolved)
    return sorted(selected)


def pantry_resources(path: str, hashes: dict[str, str], prefix: str) -> list[dict]:
    package = "/".join(path.split("/")[:2])
    selected = ["packages/basePklProject.pkl", "packages/moduleNameTest.pkl", package + "/PklProject",
                package + "/PklProject.deps.json", path + "-expected.pcf"]
    return [{"path": prefix + p, "sha256": hashes[p]} for p in selected if p in hashes]


def build_manifest(root: Path = ROOT) -> dict:
    pin = locks()
    result = {"schema_version": 1, "baseline": pin["baseline.pkl"], "sources": [], "files": [],
              "cases": [], "suites": [], "optional_sources": [], "dependency_lockfiles": [], "external_inputs": [], "summary": {}}
    for name in ("pkl", "pkl-pantry"):
        repo = root / pin[name + ".path"]
        revision = pin[name + ".revision"]
        if git(repo, "rev-parse", "HEAD").decode().strip() != revision:
            raise ValueError(f"{name}: checkout does not match references.lock")
        paths = tracked(repo)
        prefix = pin[name + ".path"] + "/"
        resources = []
        hashes = {}
        for path in paths:
            file = repo / path
            # Git tracks symlink bytes, not the referred-to file's bytes.
            data = file.readlink().as_posix().encode() if file.is_symlink() else file.read_bytes()
            hashes[path] = digest(data)
            resources.append({"path": prefix + path, "sha256": hashes[path], "bytes": len(data),
                              "kind": "symlink" if file.is_symlink() else "file"})
        result["sources"].append({"repository": name, "url": pin[name + ".url"],
                                  "revision": revision, "path": pin[name + ".path"],
                                  "resource_set_sha256": digest(encoded(resources)), "resources": resources})
        available = set(paths)
        dependency_locks = {}
        if name == "pkl-pantry":
            for path in paths:
                if path.endswith("PklProject.deps.json"):
                    lock = json.loads((repo / path).read_bytes())
                    dependency_locks[str(Path(path).parent)] = lock
                    result["dependency_lockfiles"].append({
                        "source": source_ref(name, revision, path, hashes[path]),
                        "schema_version": lock["schemaVersion"],
                        "resolved_dependencies": lock["resolvedDependencies"],
                        "verification": "Declared lock hashes only; package downloads and payloads require oracle resolution."})
            result["external_inputs"].append({"source": prefix + "packages/pkl.github.dependabotManagedActions/internal/LockfileGenerator.pkl",
                "kind": "dynamic-https-response", "uri_template": "https://github.com/{owner}/{repo}.git/info/refs?service=git-upload-pack",
                "response_sha256": None, "status": "unfixed",
                "reason": "Live GitHub refs are not pinned response bytes; testMode and hermetic fixture coverage must be verified."})
        for path in paths:
            if "/src/test/" in "/" + path and path.endswith((".kt", ".java")):
                result["suites"].append({"id": name + ":suite-source:" + path,
                    "source": source_ref(name, revision, path, hashes[path]),
                    "kind": "test-or-test-support-source", "case_count": None, "assertion_count": None,
                    "execution": {"status": "blocked", "reason": "Requires upstream JVM/Gradle suite adapter; source may define dynamic tests or support code."}})
            if name == "pkl-pantry" and path == "packages/pkl.tar/examples/test_examples.sh":
                result["suites"].append({"id": name + ":shell-suite:" + path,
                    "source": source_ref(name, revision, path, hashes[path]),
                    "kind": "shell-integration-suite", "case_count": None, "assertion_count": None,
                    "execution": {"status": "blocked", "reason": "Requires a pinned pkl executable on PATH, bash/env, and tar implementation; raw archive evaluation does not run tar integration assertions."}})
            if not (path.endswith(".pkl") or Path(path).name == "PklProject"):
                continue
            text = (repo / path).read_text(errors="replace")
            record = {"source": source_ref(name, revision, path, hashes[path]), "kind": "source-or-support",
                      "case_ids": [], "status": "inventoried", "reason": "Not an entrypoint selected by a supported upstream suite contract."}
            generated = []
            if name == "pkl" and path.startswith(SNIPPETS + "input/") and path.endswith(".pkl"):
                command = ["eval", "--no-cache", "--no-project", "--settings", "pkl:settings"]
                for value in ("name1=value1", "name2=value2", "/foo/bar=foobar"):
                    command += ["--property", value]
                for value in ("NAME1=value1", "NAME2=value2", "/foo/bar=foobar", "foo bar=foo bar", "file:///foo/bar=file:///foo/bar"):
                    command += ["--env-var", value]
                command += [prefix + path]
                item = case(name, revision, path, hashes[path], "language-snippet", command, snippet_block(path, text))
                goldens = golden_paths(path, available)
                item["upstream_goldens"] = [{"path": prefix + p, "sha256": hashes[p], "use": "provenance-only-normalized-by-upstream"} for p in goldens]
                item["resources"] = [{"path": prefix + p, "sha256": hashes[p]} for p in goldens]
                item["expected_outcome"] = "observe" if any(p.endswith(".err") for p in goldens) else "success"
                item["expected_exit"] = [0, 1] if item["expected_outcome"] == "observe" else [0]
                item["upstream_outcome"] = "error-or-logged-diagnostic" if item["expected_outcome"] == "observe" else "success"
                generated.append(item)
                record["kind"] = "language-snippet-input"
            elif name == "pkl" and path.startswith(FORMATTER + "input/"):
                generated.append(case(name, revision, path, hashes[path], "formatter-snippet", [],
                                      "Upstream calls Formatter.format(text); CLI formatting/output adapter is not implemented."))
                record["kind"] = "formatter-snippet-input"
            elif name == "pkl-pantry":
                package = "/".join(path.split("/")[:2])
                project = package + "/PklProject"
                common = ["--project-dir", prefix + package, "--property", "pkl-pantry.testMode=1"]
                if path == "packages/moduleNameTest.pkl":
                    generated.append(case(name, revision, path, hashes[path], "module-names", ["test", "--no-project", prefix + path]))
                    record["kind"] = "pantry-test-input"
                elif "/tests/" in path and "/tests/fixtures/" not in path and path.endswith(".pkl"):
                    generated.append(case(name, revision, path, hashes[path], "project-test", ["test", *common, prefix + path]))
                    record["kind"] = "pantry-test-input"
                elif "/examples/" in path and path.endswith(".pkl"):
                    generated.append(case(name, revision, path, hashes[path], "example", ["eval", *common, prefix + path]))
                    record["kind"] = "pantry-example-candidate"
                    generated[-1]["expected_outcome"] = "observe"
                    generated[-1]["expected_exit"] = [0, 1]
                    generated[-1]["selection_note"] = "Example candidate, including support/abstract modules; oracle outcome must be reviewed before acceptance."
                elif path == project:
                    projects = local_project_closure(package, dependency_locks)
                    generated.append(case(name, revision, path, hashes[path], "package-api-tests",
                        ["project", "package", "--skip-publish-check", "--output-path", "{output_dir}/%{name}/%{version}",
                         "--property", "pkl-pantry.testMode=1", *[prefix + p for p in projects]]))
                    generated[-1]["host_requirements"].append("isolated-writable-workspace")
                    generated[-1]["package_contract"] = "Runs apiTests against development modules, verifies imports, and emits metadata/ZIP/checksum files; does not evaluate ZIP modules."
                    record["kind"] = "pantry-project"
                for item in generated:
                    item["resources"] = pantry_resources(path, hashes, prefix)
                    item["host_requirements"] += ["project-resolution", "package-cache", "https-package-registry"]
                    item["dependency_lock"] = prefix + package + "/PklProject.deps.json" if package + "/PklProject.deps.json" in hashes else None
                    if project not in available and path != "packages/moduleNameTest.pkl":
                        item["execution"] = {"status": "blocked", "reason": "No package PklProject at the selected directory; command contract needs review."}
            elif name == "pkl" and path.startswith(FORMATTER + "output/"):
                record["kind"] = "formatter-golden"
            for item in generated:
                # All tracked repository resources are pinned at source level; this list
                # adds known immediate inputs, not a claim of complete dependency analysis.
                item["resource_set"] = name
                result["cases"].append(item)
                record["case_ids"].append(item["id"])
            if generated:
                record["reason"] = None
            result["files"].append(record)
    if REV1.exists():
        snapshot = json.loads(REV1.read_bytes())
        result["optional_sources"].append({"repository": "rev1", "revision": snapshot["revision"],
            "snapshot": "compatibility/rev1-inventory.json", "sha256": digest(REV1.read_bytes()),
            "pkl_file_count": len(snapshot["files"]), "execution": {"status": "blocked",
            "reason": "Historical source inventory only. Inputs are not vendored or selected for Java 0.32.0 verification; sibling checkout is optional and never required by CI."}})
    result["summary"] = {"pkl_files": sum(f["source"]["path"].endswith(".pkl") for f in result["files"]),
        "project_files": sum(Path(f["source"]["path"]).name == "PklProject" for f in result["files"]),
        "file_kinds": dict(sorted(Counter(f["kind"] for f in result["files"]).items())),
        "cases": len(result["cases"]), "case_suites": dict(sorted(Counter(c["suite"] for c in result["cases"]).items())),
        "case_execution": dict(sorted(Counter(c["execution"]["status"] for c in result["cases"]).items())),
        "jvm_test_source_files": sum(s["kind"] == "test-or-test-support-source" for s in result["suites"]),
        "shell_integration_suites": sum(s["kind"] == "shell-integration-suite" for s in result["suites"]), "assertions": None,
        "assertion_count_reason": "Source census and selected command count do not establish dynamically evaluated assertion counts.",
        "compatibility_passes": 0, "verification_status": "inventory-only"}
    return result


def snapshot_rev1(repo: Path) -> None:
    revision = git(repo, "rev-parse", "HEAD").decode().strip()
    rows = []
    for path in tracked(repo):
        if path.endswith(".pkl"):
            data = git(repo, "show", f"{revision}:{path}")
            rows.append({"path": path, "sha256": digest(data), "bytes": len(data),
                "kind": "regression-candidate" if ("/tests/" in path or path.startswith("tests/")) else "source-or-support",
                "execution": "not-selected"})
    suite_sources = []
    for path in tracked(repo):
        if path.endswith((".rs", ".py")) and ("/tests/" in path or path.startswith("tests/") or "/test_" in path):
            data = git(repo, "show", f"{revision}:{path}")
            suite_sources.append({"path": path, "sha256": digest(data), "bytes": len(data),
                "kind": "historical-test-or-support-source", "case_count": None, "assertion_count": None,
                "execution": "not-selected"})
    licenses = [{"path": p, "sha256": digest(git(repo, "show", f"{revision}:{p}"))}
                for p in tracked(repo) if Path(p).name.startswith(("LICENSE", "COPYING", "NOTICE"))]
    value = {"schema_version": 1, "repository": "rev1", "revision": revision,
             "provenance": "Optional historical local reference; Git object bytes at this revision, not working-tree output.",
             "files": rows, "suite_sources": suite_sources, "licenses": licenses, "assertion_count": None,
             "status": "inventory-only-not-verification"}
    REV1.write_bytes(encoded(value))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if committed corpus differs from discovery")
    parser.add_argument("--snapshot-rev1", type=Path, help="explicitly refresh optional historical source census")
    args = parser.parse_args()
    if args.check and args.snapshot_rev1:
        parser.error("--check cannot refresh historical provenance")
    try:
        if args.snapshot_rev1:
            snapshot_rev1(args.snapshot_rev1)
        data = encoded(build_manifest())
        if args.check:
            if not MANIFEST.exists() or MANIFEST.read_bytes() != data:
                print("corpus inventory is stale; run python3 tools/discover.py", file=sys.stderr)
                return 1
        else:
            MANIFEST.parent.mkdir(parents=True, exist_ok=True)
            MANIFEST.write_bytes(data)
        print(json.dumps(json.loads(data)["summary"], sort_keys=True))
        return 0
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"discovery failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
