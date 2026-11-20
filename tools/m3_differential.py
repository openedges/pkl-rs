#!/usr/bin/env python3
"""Raw-byte and generated-file Java 0.32.0 differential runner for M3.

Reuses tools/oracle.py (pinned Eclipse Adoptium Temurin 21.0.13.1+2-LTS and
pkl-tools-0.32.0.jar) and tools/compat.py (raw stdout/stderr/exit and generated
file path-set + raw-byte capture) across 3 lean M3 lanes:
  1. formats-renderers: -f/--format, renderer suites, converters, stdlib reflection, Reference
  2. output-files:      -m/--multiple-file-output-path (stdout + path set + raw bytes)
  3. pklbinary-decode:  M3PklBinaryDecodeProbe.java vs Rust PklBinaryDecoder probe

Never normalizes output, trims whitespace, sorts maps, or hides failures.
"""
from __future__ import annotations

import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import compat
import m3_manifest
import oracle

ROOT = Path(__file__).resolve().parents[1]
CASES_PATH = Path("tests/fixtures/m3/cases.json")
JAVA_DECODE_PROBE = Path("tools/java/M3PklBinaryDecodeProbe.java")
ALL_LANES = ("formats-renderers", "output-files", "pklbinary-decode")
DEFAULT_TIMEOUT = 30.0


def apply_setup(generated: Path, setup: dict | None) -> set[str]:
    """Create declared pre-existing directories and symlinks inside `generated`."""
    declared_symlinks: set[str] = set()
    if not setup:
        return declared_symlinks
    for rel_dir in setup.get("dirs", []):
        target = compat.relative_path(generated, rel_dir)
        target.mkdir(parents=True, exist_ok=True)
    for link_rel, link_target in setup.get("symlinks", []):
        link_path = compat.relative_path(generated, link_rel)
        link_path.parent.mkdir(parents=True, exist_ok=True)
        link_path.symlink_to(link_target)
        declared_symlinks.add(Path(link_rel).as_posix())
    return declared_symlinks


def collect_generated_files(
    generated: Path,
    declared_symlinks: set[str],
    sandbox_parent: Path | None = None,
) -> dict[str, bytes]:
    """Collect every regular file under `generated` as raw bytes.

    Rejects any undeclared symlink; allows only symlinks explicitly pre-staged
    by a case's `setup.symlinks` contract (e.g., symlink escape / symlink output dir).
    When `sandbox_parent` is provided (the parent containing `generated`), any regular
    file written outside `generated` inside `sandbox_parent` is also captured under
    `escaped/<rel>` so path-traversal bugs cannot go silently undetected.
    """
    result: dict[str, bytes] = {}
    if generated.is_symlink():
        raise ValueError(f"Unexpected output root symlink: {generated}")
    if generated.exists():
        for current, dirs, files in os.walk(generated, followlinks=False):
            cur_path = Path(current)
            kept_dirs = []
            for d in sorted(dirs):
                p = cur_path / d
                rel = p.relative_to(generated).as_posix()
                if p.is_symlink():
                    if rel not in declared_symlinks:
                        raise ValueError(f"Unexpected output symlink: {p}")
                    continue
                kept_dirs.append(d)
            dirs[:] = kept_dirs
            for name in sorted(files):
                p = cur_path / name
                rel = p.relative_to(generated).as_posix()
                if p.is_symlink():
                    if rel not in declared_symlinks:
                        raise ValueError(f"Unexpected output symlink: {p}")
                    continue
                if p.is_file():
                    result[f"generated/{rel}"] = p.read_bytes()
    if sandbox_parent is not None and sandbox_parent.exists():
        for current, dirs, files in os.walk(sandbox_parent, followlinks=False):
            cur_path = Path(current)
            if cur_path == generated:
                dirs[:] = []
                continue
            kept_dirs = []
            for d in sorted(dirs):
                p = cur_path / d
                if p == generated:
                    continue
                if p.is_symlink():
                    raise ValueError(f"Unexpected escaped symlink: {p}")
                kept_dirs.append(d)
            dirs[:] = kept_dirs
            for name in sorted(files):
                p = cur_path / name
                if p.is_symlink():
                    raise ValueError(f"Unexpected escaped symlink: {p}")
                if p.is_file():
                    rel = p.relative_to(sandbox_parent).as_posix()
                    result[f"escaped/{rel}"] = p.read_bytes()
    return dict(sorted(result.items()))


def capture_invocation(
    command: list[str],
    cwd: Path,
    env: dict[str, str],
    generated: Path,
    declared_symlinks: set[str],
    timeout: float,
    sandbox_parent: Path | None = None,
    empty_dir: Path | None = None,
) -> dict:
    """Execute one process with POSIX process-group cleanup and collect raw channels."""
    timed_out = False
    try:
        # Pass a separate empty dir to compat.capture so our symlink-aware collector
        # inspects `generated` (and `sandbox_parent`) without tripping on setup symlinks.
        dummy_gen = empty_dir if empty_dir is not None else (generated.parent / (generated.name + ".empty"))
        dummy_gen.mkdir(parents=True, exist_ok=True)
        raw = compat.capture(command, cwd, env, timeout, dummy_gen)
    except compat.CaptureTimeout as err:
        raw = err.result
        timed_out = True
    raw["files"] = collect_generated_files(generated, declared_symlinks, sandbox_parent=sandbox_parent)
    raw["timed_out"] = timed_out
    raw["workspace_changes"] = {}
    return raw


def classify_result(
    case: dict,
    expected: dict,
    actual: dict | None,
) -> tuple[str, list[str]]:
    """Classify one Java vs Rust differential comparison strictly on raw bytes."""
    if expected["timed_out"]:
        return "oracle-error", ["oracle_timed_out"]
    if expected["exit_code"] not in case["expected_exit"]:
        return "oracle-contract-mismatch", [f"oracle_exit_{expected['exit_code']}"]
    if actual is None:
        return "candidate-unavailable", ["candidate_unavailable"]
    if actual["timed_out"]:
        return "candidate-timeout", ["candidate_timed_out"]
    if case["lane"] != "pklbinary-decode" and actual["exit_code"] in (2, 3, 4):
        kind = {
            2: "candidate-unsupported",
            3: "candidate-limit-exceeded",
            4: "candidate-internal-error",
        }[actual["exit_code"]]
        return kind, [f"candidate_exit_{actual['exit_code']}"]
    diffs = compat.differences(expected, actual)
    return ("mismatch", diffs) if diffs else ("pass", [])


def build_case_env(base_env: dict[str, str], case: dict, state_dir: Path, java_version: str | None = None) -> dict[str, str]:
    home = state_dir / "home"
    tmp = state_dir / "tmp"
    home.mkdir(parents=True, exist_ok=True)
    tmp.mkdir(parents=True, exist_ok=True)
    env = {
        **base_env,
        **case.get("environment", {}),
        "HOME": str(home),
        "TMPDIR": str(tmp),
        "TMP": str(tmp),
        "TEMP": str(tmp),
    }
    if java_version:
        env["PKL_ORACLE_JAVA_VERSION"] = java_version
    return env


def java_prefix_for_state(prefix: list[str], state_dir: Path) -> list[str]:
    home = state_dir / "home"
    tmp = state_dir / "tmp"
    out = []
    for arg in prefix:
        if arg.startswith("-Duser.home="):
            out.append(f"-Duser.home={home}")
        elif arg.startswith("-Djava.io.tmpdir="):
            out.append(f"-Djava.io.tmpdir={tmp}")
        else:
            out.append(arg)
    return out


def evidence_paths(directory: Path):
    for current, dirs, files in os.walk(directory, followlinks=False):
        dirs.sort()
        for name in dirs:
            p = Path(current) / name
            if p.is_symlink():
                raise ValueError(f"evidence directory contains symlink: {p}")
        for name in sorted(files):
            p = Path(current) / name
            if p.is_symlink() or not p.is_file():
                raise ValueError(f"evidence must contain regular files: {p}")
            if p != directory / "report.json":
                yield p


def run_single_case(
    root: Path,
    case: dict,
    output: Path,
    oracle_meta: dict,
    java_decode_classes: Path | None,
    module_probe: Path | None,
    decode_probe: Path | None,
    timeout: float,
) -> dict:
    cdir_name = compat.case_directory(case["id"])
    folder = output / "cases" / cdir_name
    folder.mkdir(parents=True)
    work_root = output / "_work" / cdir_name
    work_root.mkdir(parents=True)

    lane = case["lane"]
    setup = case.get("setup")
    java_version = oracle_meta.get("jdk", {}).get("properties", {}).get("java.version")

    # Use the exact same state & output directory paths sequentially for Java and Rust
    # (wiped and freshly re-staged between runs) so `output.files` stdout/stderr paths
    # match byte-for-byte with zero path normalization.
    case_state = work_root / "state"
    sandbox_parent = work_root / "sandbox"
    case_gen = sandbox_parent / "generated"
    empty_dir = work_root / "empty"

    # 1. Java oracle execution
    case_gen.mkdir(parents=True)
    java_symlinks = apply_setup(case_gen, setup)
    java_env = build_case_env(oracle_meta["environment"], case, case_state)

    if lane == "pklbinary-decode":
        assert java_decode_classes is not None
        base_jvm = list(oracle_meta["command"][:-1])
        base_jvm[-1] += os.pathsep + str(java_decode_classes)
        java_cmd = [*java_prefix_for_state(base_jvm, case_state), "M3PklBinaryDecodeProbe", str(root / case["command"][0])]
    else:
        args = [arg.replace("{output_dir}", str(case_gen)) for arg in case["command"]]
        java_cmd = [*java_prefix_for_state(oracle_meta["command"], case_state), *args]

    java_raw = capture_invocation(
        java_cmd,
        root,
        java_env,
        case_gen,
        java_symlinks,
        timeout,
        sandbox_parent=sandbox_parent,
        empty_dir=empty_dir,
    )
    java_stored = compat.store_capture(folder / "java", java_raw)

    # 2. Rust candidate execution (fresh clean re-staging at identical paths)
    rust_raw = None
    rust_stored = None
    rust_cmd = None
    candidate_bin = decode_probe if lane == "pklbinary-decode" else module_probe
    if candidate_bin is not None:
        shutil.rmtree(case_state, ignore_errors=True)
        shutil.rmtree(sandbox_parent, ignore_errors=True)
        case_gen.mkdir(parents=True)
        rust_symlinks = apply_setup(case_gen, setup)
        rust_env = build_case_env(oracle_meta["environment"], case, case_state, java_version=java_version)
        if lane == "pklbinary-decode":
            rust_cmd = [str(candidate_bin), str(root / case["command"][0])]
        else:
            args = [arg.replace("{output_dir}", str(case_gen)) for arg in case["command"]]
            rust_cmd = [str(candidate_bin), *args]
        rust_raw = capture_invocation(
            rust_cmd,
            root,
            rust_env,
            case_gen,
            rust_symlinks,
            timeout,
            sandbox_parent=sandbox_parent,
            empty_dir=empty_dir,
        )
        rust_stored = compat.store_capture(folder / "rust", rust_raw)

    # Clean up transient runtime scratch work dir (raw evidence is sealed in folder/{java,rust})
    shutil.rmtree(work_root, ignore_errors=True)

    status, diffs = classify_result(case, java_raw, rust_raw)
    return {
        "id": case["id"],
        "lane": lane,
        "directory": cdir_name,
        "status": status,
        "differences": diffs,
        "expected_exit": case["expected_exit"],
        "java": {
            "command": java_cmd,
            "result": java_stored,
        },
        "rust": None if rust_stored is None else {
            "command": rust_cmd,
            "result": rust_stored,
        },
    }


def build_example(root: Path, example_name: str, target_dir: Path, output: Path) -> tuple[Path, dict]:
    cmd = [
        "cargo",
        "build",
        "-p",
        "pkl-runtime",
        "--locked",
        "--release",
        "--example",
        example_name,
        "--target-dir",
        str(target_dir),
    ]
    proc = subprocess.run(cmd, cwd=root, capture_output=True, timeout=600)
    (output / f"build-{example_name}.stdout.bin").write_bytes(proc.stdout)
    (output / f"build-{example_name}.stderr.bin").write_bytes(proc.stderr)
    record = {
        "command": cmd,
        "exit_code": proc.returncode,
        "stdout_sha256": compat.digest(proc.stdout),
        "stderr_sha256": compat.digest(proc.stderr),
    }
    if proc.returncode != 0:
        raise ValueError(f"failed building {example_name} (rc={proc.returncode})")
    built = target_dir / "release/examples" / example_name
    if not built.is_file():
        raise ValueError(f"built example missing: {built}")
    return built, record


def freeze_artifact(artifacts_dir: Path, name: str, path: Path, source_binding: str, build_rec: dict | None) -> tuple[Path, dict]:
    frozen = artifacts_dir / name
    frozen.write_bytes(path.read_bytes())
    frozen.chmod(0o755)
    return frozen, {
        "name": name,
        "original_path": str(path),
        "sha256": oracle.sha256(frozen),
        "source_binding": source_binding,
        "build": build_rec,
    }


def execute(
    root: Path,
    output: Path,
    lanes: list[str],
    workers: int,
    timeout: float,
    module_candidate: Path | None,
    decode_candidate: Path | None,
    cargo_target_dir: Path | None,
    allow_local_jdk: bool,
) -> dict:
    cases_doc = json.loads((root / CASES_PATH).read_bytes())
    m3_manifest.validate_cases_manifest(root, cases_doc)
    selected = [c for c in cases_doc["cases"] if c["lane"] in lanes]
    if not selected:
        raise ValueError(f"no M3 cases matched lanes={lanes}")

    if output.exists() or output.is_symlink():
        raise ValueError(f"output directory must be fresh (does not exist): {output}")

    oracle_meta = oracle.ensure_oracle(root, fetch=False)
    if not allow_local_jdk and not oracle_meta.get("exact_jdk_lock_match", False):
        raise ValueError("pinned oracle JDK lock mismatch (exact_jdk_lock_match is false)")

    output.mkdir(parents=True, exist_ok=False)
    artifacts = output / "artifacts"
    artifacts.mkdir()
    compat.write_json(output / "oracle.json", oracle_meta)
    (output / "cases.snapshot.json").write_bytes((root / CASES_PATH).read_bytes())

    target_dir = cargo_target_dir.resolve() if cargo_target_dir else (root / "target")
    need_module_probe = any(c["lane"] in ("formats-renderers", "output-files") for c in selected)
    need_decode_probe = any(c["lane"] == "pklbinary-decode" for c in selected)

    bindings: dict[str, dict | None] = {"module_eval_probe": None, "m3_pklbinary_decode_probe": None}
    frozen_module_probe = None
    frozen_decode_probe = None
    java_decode_classes = None

    if need_module_probe:
        if module_candidate is not None:
            frozen_module_probe, bindings["module_eval_probe"] = freeze_artifact(
                artifacts, "module_eval_probe", module_candidate.resolve(), "unbound-caller-supplied", None
            )
        else:
            built, rec = build_example(root, "module_eval_probe", target_dir, output)
            frozen_module_probe, bindings["module_eval_probe"] = freeze_artifact(
                artifacts, "module_eval_probe", built, "recorded-build", rec
            )

    if need_decode_probe:
        java_decode_classes = artifacts / "java-decode"
        java_decode_classes.mkdir()
        javac = Path(oracle_meta["java"]).with_name("javac")
        compile_proc = subprocess.run(
            [str(javac), "-encoding", "UTF-8", "-cp", oracle_meta["artifact"], "-d", str(java_decode_classes), str(root / JAVA_DECODE_PROBE)],
            cwd=root,
            env=oracle_meta["environment"],
            capture_output=True,
            timeout=60,
        )
        (output / "javac-decode.stdout.bin").write_bytes(compile_proc.stdout)
        (output / "javac-decode.stderr.bin").write_bytes(compile_proc.stderr)
        if compile_proc.returncode != 0:
            raise ValueError(f"failed compiling {JAVA_DECODE_PROBE}: {compile_proc.stderr.decode(errors='replace')}")

        default_decode_src = root / "crates/pkl-runtime/examples/m3_pklbinary_decode_probe.rs"
        if decode_candidate is not None:
            frozen_decode_probe, bindings["m3_pklbinary_decode_probe"] = freeze_artifact(
                artifacts, "m3_pklbinary_decode_probe", decode_candidate.resolve(), "unbound-caller-supplied", None
            )
        elif default_decode_src.is_file():
            built, rec = build_example(root, "m3_pklbinary_decode_probe", target_dir, output)
            frozen_decode_probe, bindings["m3_pklbinary_decode_probe"] = freeze_artifact(
                artifacts, "m3_pklbinary_decode_probe", built, "recorded-build", rec
            )

    def _run(case: dict) -> dict:
        return run_single_case(
            root,
            case,
            output,
            oracle_meta,
            java_decode_classes,
            frozen_module_probe,
            frozen_decode_probe,
            timeout,
        )

    # output-files cases execute sequentially for clean isolated state; others may fan out
    if workers > 1:
        with ThreadPoolExecutor(max_workers=workers) as ex:
            results = list(ex.map(_run, selected))
    else:
        results = [_run(c) for c in selected]

    work_dir = output / "_work"
    if work_dir.exists():
        shutil.rmtree(work_dir)

    by_status = dict(sorted(Counter(r["status"] for r in results).items()))
    by_lane_status: dict[str, dict[str, int]] = {}
    for lane in lanes:
        lane_rows = [r for r in results if r["lane"] == lane]
        by_lane_status[lane] = dict(sorted(Counter(r["status"] for r in lane_rows).items()))

    report = {
        "schema_version": 1,
        "baseline": cases_doc["baseline"],
        "cases_manifest_sha256": oracle.sha256(root / CASES_PATH),
        "lanes": lanes,
        "workers": workers,
        "timeout_seconds": timeout,
        "oracle": oracle_meta,
        "candidates": bindings,
        "selected_count": len(selected),
        "status_counts": by_status,
        "lane_status_counts": by_lane_status,
        "results": results,
    }
    report["evidence_sha256"] = {
        str(p.relative_to(output).as_posix()): oracle.sha256(p) for p in sorted(evidence_paths(output))
    }
    compat.write_json(output / "report.json", report)
    verify_report(output, strict=False)
    return report


def verify_report(directory: Path, *, strict: bool = False) -> dict:
    report = json.loads((directory / "report.json").read_bytes())
    if report.get("schema_version") != 1:
        raise ValueError("unsupported M3 differential report schema_version")
    if json.loads((directory / "oracle.json").read_bytes()) != report["oracle"]:
        raise ValueError("oracle.json differs from report.json")

    recorded_evidence = report.get("evidence_sha256", {})
    actual_files = {str(p.relative_to(directory).as_posix()) for p in evidence_paths(directory)}
    if not recorded_evidence or set(recorded_evidence) != actual_files:
        raise ValueError("evidence file set differs from recorded evidence_sha256")
    for rel, expected_sha in recorded_evidence.items():
        if oracle.sha256(directory / rel) != expected_sha:
            raise ValueError(f"evidence file modified: {rel}")

    if oracle.sha256(directory / "cases.snapshot.json") != report.get("cases_manifest_sha256"):
        raise ValueError("cases.snapshot.json sha256 disagrees with cases_manifest_sha256")
    cases_doc = json.loads((directory / "cases.snapshot.json").read_bytes())
    cases_by_id = {c["id"]: c for c in cases_doc["cases"]}
    selected_ids = [c["id"] for c in cases_doc["cases"] if c["lane"] in report["lanes"]]
    result_ids = [r["id"] for r in report["results"]]
    if result_ids != selected_ids or len(result_ids) != report["selected_count"]:
        raise ValueError("result list order or membership differs from selected lanes")

    for row in report["results"]:
        case = cases_by_id[row["id"]]
        expected_dir = compat.case_directory(row["id"])
        if row["directory"] != expected_dir:
            raise ValueError(f"unexpected capture directory for {row['id']}")
        folder = directory / "cases" / expected_dir
        java_cap = compat.load_capture(folder / "java")
        rust_cap = compat.load_capture(folder / "rust") if row["rust"] is not None else None
        status, diffs = classify_result(case, java_cap, rust_cap)
        if row["status"] != status or row["differences"] != diffs:
            raise ValueError(f"recomputed status/differences disagree for {row['id']}: {row['status']} vs {status}")

    recomputed = dict(sorted(Counter(r["status"] for r in report["results"]).items()))
    if recomputed != report["status_counts"]:
        raise ValueError("recomputed status_counts disagree with report.json")
    recomputed_lanes = {
        lane: dict(sorted(Counter(r["status"] for r in report["results"] if r["lane"] == lane).items()))
        for lane in report["lanes"]
    }
    if recomputed_lanes != report["lane_status_counts"]:
        raise ValueError("recomputed lane_status_counts disagree with report.json")

    if strict:
        if not report["oracle"].get("exact_jdk_lock_match", False):
            raise ValueError("strict gate requires exact_jdk_lock_match == true")
        for name, binding in report.get("candidates", {}).items():
            if binding is not None and binding.get("source_binding") != "recorded-build":
                raise ValueError(f"strict gate requires recorded-build candidate for {name}")
        non_pass = [r for r in report["results"] if r["status"] != "pass"]
        if non_pass:
            summary = ", ".join(f"{r['id']}={r['status']}" for r in non_pass[:10])
            raise ValueError(f"strict gate failed: {len(non_pass)} non-pass case(s): {summary}")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--output", type=Path, help="fresh evidence directory for live differential run")
    action.add_argument("--verify-report", type=Path, help="verify integrity only (preserves recorded failures)")
    action.add_argument("--verify-strict", type=Path, help="verify integrity AND require 100% pass + exact JDK lock")
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--lane", action="append", choices=ALL_LANES, help="select specific lane(s); default runs all 3")
    parser.add_argument("--workers", type=int, default=1)
    parser.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT)
    parser.add_argument("--record-only", action="store_true", help="with --output, exit 0 after integrity check even with mismatches")
    parser.add_argument("--module-candidate", type=Path, help="caller-supplied module_eval_probe binary (unbound)")
    parser.add_argument("--decode-candidate", type=Path, help="caller-supplied PklBinaryDecoder probe binary (unbound)")
    parser.add_argument("--cargo-target-dir", type=Path, help="Cargo target directory for recorded builds")
    parser.add_argument("--allow-local-jdk", action="store_true", help="permit PKL_ALLOW_LOCAL_JDK=1 non-exact JDK in --record-only runs")
    args = parser.parse_args()

    try:
        if args.output:
            if args.workers < 1 or args.workers > 16:
                raise ValueError("workers must be between 1 and 16")
            lanes = list(dict.fromkeys(args.lane)) if args.lane else list(ALL_LANES)
            report = execute(
                args.root.resolve(),
                args.output.resolve(),
                lanes,
                args.workers,
                args.timeout,
                args.module_candidate,
                args.decode_candidate,
                args.cargo_target_dir,
                args.allow_local_jdk,
            )
            print(json.dumps({"status_counts": report["status_counts"], "lane_status_counts": report["lane_status_counts"]}, sort_keys=True))
            if args.record_only:
                return 0
            verify_report(args.output.resolve(), strict=True)
            return 0
        target = (args.verify_report or args.verify_strict).resolve()
        report = verify_report(target, strict=bool(args.verify_strict))
        print(json.dumps({"status_counts": report["status_counts"], "lane_status_counts": report["lane_status_counts"]}, sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"M3 differential error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
