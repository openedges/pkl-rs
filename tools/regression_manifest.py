#!/usr/bin/env python3
"""Validate selected vendored historical regressions without a sibling checkout."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
REVISION = "008930df07f457fefd5d6f4ac9308ff3063daabc"
PREFIX = "tests/fixtures/rev1/"
HISTORICAL = "crates/pkl-core/tests/fixtures/"
SELECTION = {
    "diagnostics/t36/form.pkl": "diagnostics/t36/dep/form.pkl",
    "diagnostics/t36/foreign.pkl": "diagnostics/t36/app/foreign.pkl",
    "diagnostics/t36/negative.pkl": "diagnostics/t36/app/negative.pkl",
    **{path: path for path in (
        "class_authority/t40/repro.pkl", "class_authority/t40/repro_lib.pkl",
        "class_authority/t40/foreign_namesake/repro.pkl",
        "class_authority/t40/foreign_namesake/repro_lib.pkl",
        "class_authority/t40/foreign_namesake/foreign.pkl")},
}
PROJECT = b'amends "pkl:Project"\ndependencies { ["dep"] { uri = "package://example.test/dep@1.0.0" } }\n'
DEPS = b'{"schemaVersion":1,"resolvedDependencies":{"package://example.test/dep@1":{"type":"local","uri":"projectpackage://example.test/dep@1.0.0","path":"../dep"}}}\n'
DERIVED = {"diagnostics/t36/app/PklProject": PROJECT, "diagnostics/t36/app/PklProject.deps.json": DEPS}
CORE_ASSERTIONS = "crates/pkl-core/src/tests/bytecode_tests/class_surface_execution.rs"
CLI_ASSERTIONS = "crates/pkl-cli/tests/imported_collection_authority.rs"

REVIEWED_ORACLE = {'version': '0.32.0', 'profile': 'reference', 'artifact_sha256': 'c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059', 'lock_sha256': '7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b', 'source_revision': 'e51fb2f91774265666f94980ec630db794a4db2d', 'jdk': {'runtime_modules_sha256': '40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf', 'release_sha256': 'ab66b8afeea98b25f1869fbe7c0dde368d448e7cc0f7bab0359b322ade134fe1', 'properties': {'java.runtime.version': '21.0.12.1+1-LTS', 'java.vendor': 'Eclipse Adoptium', 'java.vm.name': 'OpenJDK 64-Bit Server VM', 'java.vm.version': '21.0.12.1+1-LTS'}}, 'review': 'Fresh Java 0.32.0 raw outputs and semantic failure locations reviewed; historical Rust outputs are not used as expectations.'}
REVIEWED_OBSERVATIONS = {'t36-foreign-nominal-identity': {'exit_code': 1, 'sha256': {'stderr': '0ea3a01eae8b63f5990f26af740c0def8ae6bb5861bb629f526e607ad27e4cb9', 'stdout': 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'}, 'generated_files': {}, 'comparison_role': 'Observed raw hashes preserve provenance for this recorded environment; the runner captures and compares its own raw Java baseline.'}, 't40-imported-declaration-authority': {'exit_code': 0, 'sha256': {'stderr': 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855', 'stdout': '4d4d0946a537296a4ac9e2b9a1705e3cb4fa597fbcb4ba14b57b9275a71851ec'}, 'generated_files': {}, 'comparison_role': 'Observed raw hashes preserve provenance for this recorded environment; the runner captures and compares its own raw Java baseline.'}, 't40-foreign-namesake': {'exit_code': 1, 'sha256': {'stderr': '508e9aadbe9918b03998926c72f074301535483ab2bd3622367d3942ced9b24e', 'stdout': 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'}, 'generated_files': {}, 'comparison_role': 'Observed raw hashes preserve provenance for this recorded environment; the runner captures and compares its own raw Java baseline.'}}

def sha(data):
    return hashlib.sha256(data).hexdigest()


def encode(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode()


def snapshot(root):
    data = (root / "compatibility/rev1-inventory.json").read_bytes()
    value = json.loads(data)
    if value.get("revision") != REVISION or value.get("repository") != "rev1":
        raise ValueError("unexpected historical inventory revision/repository")
    return value, sha(data)


def import_sources(repository, root=ROOT):
    historical, _ = snapshot(root)
    rows = {r["path"]: r for r in historical["files"] + historical["licenses"]}
    selected = {HISTORICAL + original: PREFIX + local for original, local in SELECTION.items()}
    selected.update({name: PREFIX + name for name in ("LICENSE", "NOTICE")})
    contents = {}
    for original, target in selected.items():
        content = subprocess.check_output(["git", "-C", str(repository), "show", f"{REVISION}:{original}"])
        if original not in rows or sha(content) != rows[original]["sha256"]:
            raise ValueError(f"historical object does not match pinned snapshot: {original}")
        contents[target] = content
    contents.update({PREFIX + path: data for path, data in DERIVED.items()})
    # Validate every original before writing any imported material.
    for target, content in sorted(contents.items()):
        path = root / target
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)


def build(root=ROOT):
    historical, snapshot_sha = snapshot(root)
    rows = {row["path"]: row for row in historical["files"] + historical["licenses"] + historical["suite_sources"]}
    resources = []
    by_local = {}
    selections = {HISTORICAL + original: PREFIX + local for original, local in SELECTION.items()}
    selections.update({name: PREFIX + name for name in ("LICENSE", "NOTICE")})
    for original, target in sorted(selections.items()):
        data = (root / target).read_bytes()
        expected = rows[original]["sha256"]
        if sha(data) != expected:
            raise ValueError(f"vendored bytes differ from pinned historical object: {target}")
        entry = {"path": target, "sha256": expected, "bytes": len(data), "kind": "license" if original in ("LICENSE", "NOTICE") else "unchanged-pkl-input",
            "original": {"repository": "rev1", "revision": REVISION, "path": original, "sha256": expected}}
        resources.append(entry)
        by_local[target] = entry
    for local, data in sorted(DERIVED.items()):
        target = PREFIX + local
        if (root / target).read_bytes() != data:
            raise ValueError(f"derived project fixture is missing or changed: {target}")
        entry = {"path": target, "sha256": sha(data), "bytes": len(data), "kind": "derived-project-settings",
            "derivation": {"repository": "rev1", "revision": REVISION, "path": CORE_ASSERTIONS,
                "sha256": rows[CORE_ASSERTIONS]["sha256"], "function": "foreign_class_constraint_diagnostic",
                "adaptation": "Preserve harness project declaration; replace temporary absolute dependency path with ../dep. No Pkl input source is rewritten."}}
        resources.append(entry)
        by_local[target] = entry
    expected_paths = {entry["path"] for entry in resources}
    actual_paths = {path.relative_to(root).as_posix() for path in (root / PREFIX).rglob("*") if path.is_file()}
    if actual_paths != expected_paths:
        raise ValueError("vendored regression directory has missing or uninventoried files")
    specs = [
        ("t36-foreign-nominal-identity", "diagnostics/t36/app/negative.pkl", "diagnostics/t36/", "error", CORE_ASSERTIONS, "foreign_class_constraint_diagnostic", True),
        ("t40-imported-declaration-authority", "class_authority/t40/repro.pkl", "class_authority/t40/", "success", CORE_ASSERTIONS, "imported_collection_member_declaration_authority", False),
        ("t40-foreign-namesake", "class_authority/t40/foreign_namesake/repro.pkl", "class_authority/t40/foreign_namesake/", "error", CLI_ASSERTIONS, "foreign_namesake_bytecode_declaration_frame_matches_apple", False),
    ]
    cases = []
    for identifier, local, family, historical_outcome, assertion_path, assertion_function, project in specs:
        target = PREFIX + local
        command = ["eval", "--settings", "pkl:settings", "--color=never"]
        command += ["--project-dir", PREFIX + "diagnostics/t36/app"] if project else ["--no-project"]
        command.append(target)
        selected_resources = [resource for resource in resources if resource["path"].startswith(PREFIX + family)]
        if identifier == "t40-imported-declaration-authority":
            selected_resources = [resource for resource in selected_resources if "/foreign_namesake/" not in resource["path"]]
        cases.append({"id": "rev1:" + identifier, "suite": "selected-historical-regression",
            "source": {"repository": "pkl-rs", "path": target, "sha256": by_local[target]["sha256"]},
            "original": by_local[target]["original"], "command": command, "cwd": ".", "environment": {},
            "host_requirements": ["filesystem", "local-project-dependency"] if project else ["filesystem"],
            "expected_outcome": "success" if REVIEWED_OBSERVATIONS[identifier]["exit_code"] == 0 else "failure",
            "expected_exit": [REVIEWED_OBSERVATIONS[identifier]["exit_code"]],
            "oracle_observation": REVIEWED_OBSERVATIONS[identifier],
            "channels": ["stdout", "stderr", "exit_code", "files"], "target": ["native", "wasm", "napi"],
            "execution": {"status": "ready", "reason": None},
            "resources": [{"path": resource["path"], "sha256": resource["sha256"]} for resource in selected_resources],
            "historical_assertion": {"repository": "rev1", "revision": REVISION, "path": assertion_path,
                "sha256": rows[assertion_path]["sha256"], "function": assertion_function,
                "expected_outcome": historical_outcome,
                "authority": "Historical Rust assertion is selection context only; fresh pinned Java 0.32.0 output determines compatibility."},
            "assertion_count": None})
    return {"schema_version": 1, "scope": "Selected historical regression inputs; independent Java 0.32.0 observations required",
        "provenance": {"historical_repository": "rev1", "revision": REVISION,
            "snapshot": "compatibility/rev1-inventory.json", "snapshot_sha256": snapshot_sha},
        "licenses": [{"path": PREFIX + name, "sha256": by_local[PREFIX + name]["sha256"]} for name in ("LICENSE", "NOTICE")],
        "files": resources, "cases": cases, "reviewed_oracle": REVIEWED_ORACLE,
        "summary": {"unchanged_pkl_inputs": len(SELECTION), "derived_project_settings": len(DERIVED),
            "command_cases": len(cases), "compatibility_passes": 0, "assertions": None}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--import-from", type=Path, help="explicitly import pinned Git objects; normal checks never access a sibling")
    args = parser.parse_args()
    if args.check and args.import_from:
        parser.error("--check does not import or rewrite fixtures")
    try:
        if args.import_from:
            import_sources(args.import_from)
        data = encode(build())
        path = ROOT / "compatibility/regressions.json"
        if args.check:
            if not path.is_file() or path.read_bytes() != data:
                raise ValueError("selected regression manifest missing or stale")
        else:
            path.write_bytes(data)
        print("3 selected regression contracts verified; expected exits follow reviewed Java 0.32.0 observations")
        return 0
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"regression discovery failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
