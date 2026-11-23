#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Local / dedicated verification-environment runner for pkl-rs.

GitHub Actions (`.github/workflows/ci.yml` and `.github/workflows/release.yml`)
is intentionally restricted to minimal low-cost checks, packaging smoke, and
tagged cross-platform release distribution. All expensive compatibility,
differential, cross-browser, fuzzing, memory-soak, and benchmark suites live
here so they execute reproducibly in a local or dedicated verification host
with fresh timestamped output directories and honest failure preservation.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from typing import Any

ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True)
class Step:
    name: str
    command: list[str]
    env_overrides: dict[str, str] | None = None


@dataclass(frozen=True)
class SuiteSpec:
    id: str
    description: str
    requires_java: bool
    requires_native_image: bool
    requires_playwright: bool


SUITE_SPECS: tuple[SuiteSpec, ...] = (
    SuiteSpec("workspace", "Full Rust workspace verify, fmt, clippy, wasm32 check, and all workspace tests", False, False, False),
    SuiteSpec("manifests", "All 12 milestone manifests, contracts, oracle pins, and active Python tooling unit tests", True, True, False),
    SuiteSpec("m0", "M0 pinned Java oracle smoke, replay, and selected historical regressions (--full-corpus optional)", True, False, False),
    SuiteSpec("m1", "M1 architecture experiments (frontend, runtime, Host, lifecycle, native/Node/Chromium, Graal archive)", True, False, True),
    SuiteSpec("m2", "M2 language differential verification (6 fast adapters or --full-corpus all 7 + --verify-full)", True, False, False),
    SuiteSpec("m3", "M3 1,781-item mechanical coverage probe and 3-lane raw-byte differential strict gate", True, False, False),
    SuiteSpec("m4", "M4 offline Host I/O, external readers, projects/packages (lanes 1-3 or --full-corpus all 5 Pantry lanes)", True, False, False),
    SuiteSpec("m5", "M5 Rust API, CLI, REPL, server, formatter, and pkldoc 5-lane differential & coverage gate", True, False, False),
    SuiteSpec("m6", "M6 Wasm & N-API 5-lane differential gate plus cross-browser Playwright smoke", True, False, True),
    SuiteSpec("m7-fuzz", "M7 seeded differential fuzzer (210 generated + saved regressions + --verify-strict)", True, False, False),
    SuiteSpec("m7-soak", "M7 long-running memory reclamation soak (native or --all-soak-targets 18 pairs)", False, False, False),
    SuiteSpec("m7-bench", "M7 performance & peak RSS benchmark suite (--baseline native-image|both|jvm)", False, True, False),
    SuiteSpec("m7-js", "M7 cross-browser & Node JS/Wasm performance and memory decomposition analysis", True, False, True),
    SuiteSpec("m7-release", "Unified M7 release-readiness audit (`m7_manifest.py --verify-release`)", False, False, False),
)

SUITE_BY_ID: dict[str, SuiteSpec] = {spec.id: spec for spec in SUITE_SPECS}

PRESETS: dict[str, tuple[str, ...]] = {
    "quick": ("workspace", "manifests"),
    "compat": ("m0", "m1", "m2", "m3", "m4", "m5", "m6"),
    "m7": ("m7-fuzz", "m7-soak", "m7-bench", "m7-js"),
    "full": (
        "workspace",
        "manifests",
        "m0",
        "m1",
        "m2",
        "m3",
        "m4",
        "m5",
        "m6",
        "m7-fuzz",
        "m7-soak",
        "m7-bench",
        "m7-js",
    ),
}


def build_suite_steps(
    suite_id: str,
    *,
    python_exe: str,
    out_root: Path,
    full_corpus: bool = False,
    baseline: str = "native-image",
    bench_profile: str = "full",
    browsers: str = "chromium,firefox,webkit",
    all_soak_targets: bool = False,
    m2_workers: int = 4,
    release_dir: Path | None = None,
) -> list[Step]:
    if suite_id not in SUITE_BY_ID:
        raise ValueError(f"Unknown verification suite: {suite_id}")

    py = python_exe
    py_env = {"PYTHONPATH": str(ROOT / "tools")}
    suite_out = out_root / suite_id

    if suite_id == "workspace":
        return [
            Step("xtask-verify", ["cargo", "xtask", "verify"]),
            Step("rustfmt-check", ["cargo", "fmt", "--all", "--", "--check"]),
            Step("clippy-workspace", ["cargo", "clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"]),
            Step("clippy-java-regex", ["cargo", "clippy", "-p", "java_regex", "--lib", "--locked", "--", "-D", "warnings"]),
            Step("clippy-wasm", ["cargo", "clippy", "-p", "pkl-wasm", "--target", "wasm32-unknown-unknown", "--locked", "--", "-D", "warnings"]),
            Step("check-wasm32", ["cargo", "check", "-p", "pkl-runtime", "-p", "pkl-formatter", "-p", "pkl-wasm", "--target", "wasm32-unknown-unknown", "--locked"]),
            Step("test-workspace", ["cargo", "test", "--workspace", "--locked"]),
        ]

    if suite_id == "manifests":
        return [
            Step("check-inventory", [py, "tools/inventory.py", "--check"]),
            Step("check-discover", [py, "tools/discover.py", "--check"]),
            Step("check-smoke-manifest", [py, "tools/smoke_manifest.py", "--check"]),
            Step("check-regression-manifest", [py, "tools/regression_manifest.py", "--check"]),
            Step("check-m2-manifest", [py, "tools/m2_manifest.py", "--check"]),
            Step("check-m2-module-eval", [py, "tools/m2_module_eval.py", "--check"]),
            Step("check-m2-jvm-classpath", [py, "tools/m2_jvm_classpath.py", "--check"]),
            Step("check-m3-manifest", [py, "tools/m3_manifest.py", "--check"]),
            Step("check-m4-manifest", [py, "tools/m4_manifest.py", "--check"]),
            Step("check-m5-manifest", [py, "tools/m5_manifest.py", "--check"]),
            Step("check-m6-manifest", [py, "tools/m6_manifest.py", "--check"]),
            Step("check-m7-manifest", [py, "tools/m7_manifest.py", "--check"]),
            Step("check-m0-benchmark-contract", [py, "tools/benchmark.py", "--check-contract"]),
            Step("check-m7-benchmark-contract", [py, "tools/m7_benchmark.py", "--check-contract"]),
            Step("verify-jvm-oracle", [py, "tools/oracle.py", "verify"]),
            Step("verify-native-image-oracle", [py, "tools/oracle.py", "verify", "--profile", "native-image"]),
            Step(
                "unit-tests-core-and-m3-m7",
                [
                    py,
                    "-m",
                    "unittest",
                    "tools/tests/test_local_verify.py",
                    "tools/tests/test_oracle.py",
                    "tools/tests/test_benchmark.py",
                    "tools/tests/test_compat.py",
                    "tools/tests/test_discover.py",
                    "tools/tests/test_evidence.py",
                    "tools/tests/test_inventory.py",
                    "tools/tests/test_pantry.py",
                    "tools/tests/test_regressions.py",
                    "tools/tests/test_m3_manifest.py",
                    "tools/tests/test_m3_differential.py",
                    "tools/tests/test_m4_manifest.py",
                    "tools/tests/test_m4_differential.py",
                    "tools/tests/test_m4_servers.py",
                    "tools/tests/test_m5_manifest.py",
                    "tools/tests/test_m5_differential.py",
                    "tools/tests/test_m5_repl_driver.py",
                    "tools/tests/test_m5_server_client.py",
                    "tools/tests/test_m6_manifest.py",
                    "tools/tests/test_m6_differential.py",
                    "tools/tests/test_m7_benchmark.py",
                    "tools/tests/test_m7_fuzz.py",
                    "tools/tests/test_m7_js_analysis.py",
                    "tools/tests/test_m7_manifest.py",
                    "tools/tests/test_m7_memory_soak.py",
                ],
                py_env,
            ),
        ]

    if suite_id == "m0":
        steps = [
            Step("oracle-smoke", [py, "tools/oracle.py", "smoke"]),
            Step("compat-record-smoke", [py, "tools/compat.py", "record", "--manifest", "compatibility/smoke.json", "--output", str(suite_out / "baseline")]),
            Step("compat-check-smoke", [py, "tools/compat.py", "check", "--manifest", "compatibility/smoke.json", "--output", str(suite_out / "baseline")]),
            Step("compat-record-regressions", [py, "tools/compat.py", "record", "--manifest", "compatibility/regressions.json", "--output", str(suite_out / "regressions")]),
            Step("compat-check-regressions", [py, "tools/compat.py", "check", "--manifest", "compatibility/regressions.json", "--output", str(suite_out / "regressions")]),
        ]
        if full_corpus:
            steps.append(Step("compat-record-full-corpus", [py, "tools/compat.py", "record", "--output", str(suite_out / "full-corpus")]))
        return steps

    if suite_id == "m1":
        return [
            Step("m1-contracts", [py, "tools/m1_frontend.py", "--check-contract"]),
            Step("m1-semantics-check", [py, "tools/m1_semantics.py", "--check"]),
            Step("m1-graal-evidence", [py, "tools/evidence.py", "verify", "reports/m1/graal-evidence.tar.gz"]),
            Step("m1-graal-cause-evidence", [py, "tools/evidence.py", "verify", "reports/m1/graal-cause-evidence.tar.gz"]),
            Step("m1-frontend", [py, "tools/m1_frontend.py", "--output", str(suite_out / "frontend")]),
            Step("m1-runtime", [py, "tools/m1_runtime.py", "--output", str(suite_out / "runtime")]),
            Step("m1-host", [py, "tools/m1_host.py", "--output", str(suite_out / "host-java")]),
            Step("m1-lifecycle", [py, "tools/m1_lifecycle.py", "--output", str(suite_out / "lifecycle")]),
            Step("m1-platforms", [py, "tools/m1_platforms.py", "--verify-contract", "--output", str(suite_out / "platforms")]),
        ]

    if suite_id == "m2":
        lang_dir = suite_out / "language"
        cls_dir = suite_out / "class-lifecycle"
        ident_dir = suite_out / "identifier"
        if full_corpus:
            rec_cmd = [py, "tools/m2_language.py", "--output", str(lang_dir), "--workers", str(m2_workers), "--record-only"]
        else:
            rec_cmd = [
                py,
                "tools/m2_language.py",
                "--output",
                str(lang_dir),
                "--workers",
                str(m2_workers),
                "--adapter",
                "language-api-string",
                "--adapter",
                "historical-collections-module-eval",
                "--adapter",
                "historical-cli-module-eval",
                "--adapter",
                "selected-regression-module-eval",
                "--adapter",
                "package-project-eval",
                "--adapter",
                "language-jvm-classpath-output",
                "--record-only",
            ]
        steps = [
            Step("m2-language-record", rec_cmd),
            Step("m2-class-lifecycle-record", [py, "tools/m2_class_lifecycle.py", "--output", str(cls_dir), "--record-only"]),
            Step("m2-class-lifecycle-verify", [py, "tools/m2_class_lifecycle.py", "--verify-contract", str(cls_dir)]),
            Step("m2-identifier-tables", [py, "tools/m2_identifier_tables.py", "--output", str(ident_dir)]),
            Step("m2-verify-development", [py, "tools/m2_language.py", "--verify-development", str(lang_dir)]),
        ]
        if full_corpus:
            steps.append(Step("m2-verify-full", [py, "tools/m2_language.py", "--verify-full", str(lang_dir)]))
        return steps

    if suite_id == "m3":
        census = suite_out / "census.json"
        diff_dir = suite_out / "diff"
        return [
            Step("m3-build-probe", ["cargo", "build", "--release", "-p", "pkl-runtime", "--example", "m3_coverage_probe", "--locked"]),
            Step("m3-run-probe", [py, "tools/m3_manifest.py", "--run-probe", "target/release/examples/m3_coverage_probe", "--census-out", str(census)]),
            Step("m3-verify-coverage", [py, "tools/m3_manifest.py", "--verify-coverage", str(census)]),
            Step("m3-differential", [py, "tools/m3_differential.py", "--output", str(diff_dir)]),
            Step("m3-verify-report", [py, "tools/m3_differential.py", "--verify-report", str(diff_dir)]),
            Step("m3-verify-strict", [py, "tools/m3_differential.py", "--verify-strict", str(diff_dir)]),
        ]

    if suite_id == "m4":
        diff_dir = suite_out / "diff"
        if full_corpus:
            return [
                Step("m4-differential-all5", [py, "tools/m4_differential.py", "--output", str(diff_dir)]),
                Step("m4-verify-strict", [py, "tools/m4_differential.py", "--verify-strict", str(diff_dir)]),
                Step("m4-verify-coverage", [py, "tools/m4_manifest.py", "--verify-coverage", str(diff_dir)]),
            ]
        return [
            Step(
                "m4-differential-lanes123",
                [
                    py,
                    "tools/m4_differential.py",
                    "--lane",
                    "host-io-http-security",
                    "--lane",
                    "external-readers",
                    "--lane",
                    "projects-packages-cache",
                    "--output",
                    str(diff_dir),
                ],
            ),
            Step("m4-verify-strict", [py, "tools/m4_differential.py", "--verify-strict", str(diff_dir)]),
        ]

    if suite_id == "m5":
        diff_dir = suite_out / "diff"
        return [
            Step("m5-differential", [py, "tools/m5_differential.py", "--output", str(diff_dir)]),
            Step("m5-verify-report", [py, "tools/m5_differential.py", "--verify-report", str(diff_dir)]),
            Step("m5-verify-strict", [py, "tools/m5_differential.py", "--verify-strict", str(diff_dir)]),
            Step("m5-verify-coverage", [py, "tools/m5_manifest.py", "--verify-coverage", str(diff_dir)]),
        ]

    if suite_id == "m6":
        diff_dir = suite_out / "diff"
        browser_dir = suite_out / "browser-smoke"
        return [
            Step("m6-differential", [py, "tools/m6_differential.py", "--output", str(diff_dir)]),
            Step("m6-verify-report", [py, "tools/m6_differential.py", "--verify-report", str(diff_dir)]),
            Step("m6-verify-strict", [py, "tools/m6_differential.py", "--verify-strict", str(diff_dir)]),
            Step("m6-verify-coverage", [py, "tools/m6_manifest.py", "--verify-coverage", str(diff_dir)]),
            Step("m6-browser-smoke", [py, "tools/m7_js_analysis.py", "--smoke-browsers", browsers, "--output", str(browser_dir)], py_env),
        ]

    if suite_id == "m7-fuzz":
        fuzz_dir = suite_out / "fuzz"
        return [
            Step(
                "m7-fuzz-run",
                [
                    py,
                    "tools/m7_fuzz.py",
                    "--output",
                    str(fuzz_dir),
                    "--count",
                    "210",
                    "--replay-regressions",
                    "tests/fixtures/m7/fuzz-regressions",
                ],
                py_env,
            ),
            Step("m7-fuzz-verify-strict", [py, "tools/m7_fuzz.py", "--verify-strict", str(fuzz_dir)], py_env),
        ]

    if suite_id == "m7-soak":
        soak_dir = suite_out / "soak"
        if all_soak_targets:
            return [
                Step("m7-soak-all", [py, "tools/m7_memory_soak.py", "--profile", "standard", "--output", str(soak_dir)], py_env),
                Step("m7-soak-verify-strict", [py, "tools/m7_memory_soak.py", "--verify-strict", str(soak_dir)], py_env),
            ]
        return [
            Step("m7-soak-native", [py, "tools/m7_memory_soak.py", "--target", "native", "--profile", "standard", "--output", str(soak_dir)], py_env),
            Step("m7-soak-verify-report", [py, "tools/m7_memory_soak.py", "--verify-report", str(soak_dir)], py_env),
        ]

    if suite_id == "m7-bench":
        bench_dir = suite_out / "bench"
        return [
            Step(
                "m7-bench-run",
                [
                    py,
                    "tools/m7_benchmark.py",
                    "--profile",
                    bench_profile,
                    "--baseline",
                    baseline,
                    "--output",
                    str(bench_dir),
                ],
                py_env,
            ),
            Step("m7-bench-verify-report", [py, "tools/m7_benchmark.py", "--verify-report", str(bench_dir)], py_env),
        ]

    if suite_id == "m7-js":
        js_dir = suite_out / "js-analysis"
        return [
            Step(
                "m7-js-analysis",
                [
                    py,
                    "tools/m7_js_analysis.py",
                    "--profile",
                    "full",
                    "--browsers",
                    browsers,
                    "--output",
                    str(js_dir),
                ],
                py_env,
            ),
        ]

    if suite_id == "m7-release":
        target_dir = release_dir if release_dir is not None else out_root
        return [
            Step("m7-verify-release", [py, "tools/m7_manifest.py", "--verify-release", str(target_dir)], py_env),
        ]

    raise AssertionError(f"Unhandled suite: {suite_id}")


def resolve_suite_selection(suites: list[str], preset: str | None) -> list[str]:
    selected: list[str] = []
    if preset:
        if preset not in PRESETS:
            raise ValueError(f"Unknown preset: {preset}")
        selected.extend(PRESETS[preset])
    for item in suites:
        if item not in SUITE_BY_ID:
            raise ValueError(f"Unknown verification suite: {item}")
        if item not in selected:
            selected.append(item)
    if not selected:
        raise ValueError("Specify at least one --suite or --preset (use --list to inspect available suites)")
    return selected


def run_suites(
    suite_ids: list[str],
    *,
    root: Path = ROOT,
    python_exe: str = sys.executable,
    out_root: Path,
    dry_run: bool = False,
    continue_on_error: bool = False,
    full_corpus: bool = False,
    baseline: str = "native-image",
    bench_profile: str = "full",
    browsers: str = "chromium,firefox,webkit",
    all_soak_targets: bool = False,
    m2_workers: int = 4,
    release_dir: Path | None = None,
) -> dict[str, Any]:
    suite_results: list[dict[str, Any]] = []
    overall_ok = True

    if not dry_run:
        out_root.mkdir(parents=True, exist_ok=True)

    for suite_id in suite_ids:
        steps = build_suite_steps(
            suite_id,
            python_exe=python_exe,
            out_root=out_root,
            full_corpus=full_corpus,
            baseline=baseline,
            bench_profile=bench_profile,
            browsers=browsers,
            all_soak_targets=all_soak_targets,
            m2_workers=m2_workers,
            release_dir=release_dir,
        )
        step_records: list[dict[str, Any]] = []
        suite_ok = True
        for step in steps:
            if dry_run:
                step_records.append(
                    {
                        "name": step.name,
                        "command": step.command,
                        "status": "planned",
                        "exit_code": None,
                        "duration_s": 0.0,
                    }
                )
                continue

            env = os.environ.copy()
            if step.env_overrides:
                env.update(step.env_overrides)
            t0 = time.monotonic()
            proc = subprocess.run(step.command, cwd=str(root), env=env)
            elapsed = round(time.monotonic() - t0, 3)
            ok = proc.returncode == 0
            step_records.append(
                {
                    "name": step.name,
                    "command": step.command,
                    "status": "pass" if ok else "fail",
                    "exit_code": proc.returncode,
                    "duration_s": elapsed,
                }
            )
            if not ok:
                suite_ok = False
                overall_ok = False
                if not continue_on_error:
                    break

        suite_results.append(
            {
                "suite": suite_id,
                "status": "planned" if dry_run else ("pass" if suite_ok else "fail"),
                "steps": step_records,
            }
        )
        if not suite_ok and not continue_on_error:
            break

    summary = {
        "schema_version": 1,
        "generated_at_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "provenance": "local-verification-environment-not-hosted-ci",
        "dry_run": dry_run,
        "output_root": str(out_root),
        "selected_suites": suite_ids,
        "overall_status": "planned" if dry_run else ("pass" if overall_ok else "fail"),
        "suites": suite_results,
    }
    if not dry_run:
        (out_root / "summary.json").write_text(
            json.dumps(summary, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
    return summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", action="store_true", help="List available suites and presets")
    parser.add_argument("--preset", choices=sorted(PRESETS.keys()), help="Run a predefined suite group")
    parser.add_argument("--suite", action="append", default=[], help="Verification suite to execute (repeatable)")
    parser.add_argument("--output-root", type=Path, default=None, help="Fresh output directory for evidence")
    parser.add_argument("--dry-run", action="store_true", help="Print planned commands without running them")
    parser.add_argument("--continue-on-error", action="store_true", help="Continue remaining steps/suites on failure")
    parser.add_argument("--full-corpus", action="store_true", help="Enable full M0/M2/M4 Pantry corpus scopes")
    parser.add_argument("--baseline", choices=["native-image", "both", "jvm"], default="native-image", help="M7 benchmark baseline")
    parser.add_argument("--bench-profile", choices=["full", "quick"], default="full", help="M7 benchmark profile")
    parser.add_argument("--browsers", default="chromium,firefox,webkit", help="Comma-separated Playwright browsers")
    parser.add_argument("--all-soak-targets", action="store_true", help="Run all 18 soak pairs (native + napi + wasm)")
    parser.add_argument("--m2-workers", type=int, default=4, help="Worker count for M2 language replay")
    parser.add_argument("--release-dir", type=Path, default=None, help="Existing M7 evidence dir for m7-release audit")
    args = parser.parse_args(argv)

    if args.list:
        print("Suites:")
        for spec in SUITE_SPECS:
            reqs = []
            if spec.requires_java:
                reqs.append("java-oracle")
            if spec.requires_native_image:
                reqs.append("native-image")
            if spec.requires_playwright:
                reqs.append("playwright")
            tag = f" [{', '.join(reqs)}]" if reqs else ""
            print(f"  {spec.id:12s} {spec.description}{tag}")
        print("\nPresets:")
        for name, members in PRESETS.items():
            print(f"  {name:12s} {', '.join(members)}")
        return 0

    try:
        selected = resolve_suite_selection(args.suite, args.preset)
    except ValueError as exc:
        print(f"local_verify: {exc}", file=sys.stderr)
        return 2

    ts = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
    out_root = args.output_root if args.output_root is not None else (ROOT / "target" / "local-verify" / ts)

    summary = run_suites(
        selected,
        python_exe=sys.executable,
        out_root=out_root,
        dry_run=args.dry_run,
        continue_on_error=args.continue_on_error,
        full_corpus=args.full_corpus,
        baseline=args.baseline,
        bench_profile=args.bench_profile,
        browsers=args.browsers,
        all_soak_targets=args.all_soak_targets,
        m2_workers=args.m2_workers,
        release_dir=args.release_dir,
    )
    print(json.dumps(summary, indent=2, ensure_ascii=False))
    return 0 if summary["overall_status"] in ("pass", "planned") else 1


if __name__ == "__main__":
    sys.exit(main())
