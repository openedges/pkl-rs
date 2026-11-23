#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Local / dedicated verification-environment runner for pkl-rs.

GitHub Actions (`.github/workflows/ci.yml` and `.github/workflows/release.yml`)
is intentionally restricted to minimal low-cost checks, packaging smoke, and
tagged cross-platform release distribution. All full workspace tests, active
and historical Python unit tests, standalone package tests, M0-M6 Java 0.32.0
differential suites, Playwright cross-browser matrices, M7 seeded fuzzing,
18-pair memory reclamation soak, and 34-workload benchmarks run here in a local
or dedicated verification host with fresh timestamped output directories and
honest failure preservation.
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
import tarfile
import time
from typing import Any

ROOT = Path(__file__).resolve().parents[1]

CORE_ACTIVE_PYTHON_TESTS: frozenset[str] = frozenset(
    {
        "test_benchmark.py",
        "test_compat.py",
        "test_discover.py",
        "test_evidence.py",
        "test_inventory.py",
        "test_local_verify.py",
        "test_oracle.py",
        "test_pantry.py",
        "test_regressions.py",
        "test_runtime_fixture_links.py",
        "test_runtime_source_inputs.py",
        "test_vendor_sources.py",
        "test_workspace_storage.py",
    }
)

ACTIVE_MILESTONE_TEST_PREFIXES: tuple[str, ...] = (
    "test_m1_",
    "test_m2_",
    "test_m3_",
    "test_m4_",
    "test_m5_",
    "test_m6_",
    "test_m7_",
)


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
    SuiteSpec(
        "workspace",
        "Full Rust workspace verify, fmt, clippy (workspace/java_regex/wasm32), wasm32 check, and all 378 workspace test targets",
        False,
        False,
        False,
    ),
    SuiteSpec(
        "manifests",
        "All 12 milestone manifests, contracts, oracle pins (JVM + native-image), and all 59 active Python tooling unit test modules",
        True,
        True,
        False,
    ),
    SuiteSpec(
        "packages",
        "Standalone @pkl-rs/wasm (gzip-9 <= 2.50 MiB + npm pack), @pkl-rs/napi (run + pack-install), and release CLI smoke",
        False,
        False,
        False,
    ),
    SuiteSpec(
        "platform-js-tests",
        "All 41 Node.js platform/adapter/transport unit test files under tools/platform/*.test.mjs (384 tests)",
        False,
        False,
        False,
    ),
    SuiteSpec(
        "py-historical",
        "All 75 historical M2 fixture/compare Python test suites (opt-in; preserves 2 Stage-65 stale-oracle.lock.json modules honestly)",
        True,
        False,
        False,
    ),
    SuiteSpec(
        "protobuf-strict",
        "Explicit ignored Rust strict Protobuf compatibility gate (tracks 18 upstream REFERENCE-NONPASS slots; expected exit 101)",
        False,
        False,
        False,
    ),
    SuiteSpec(
        "m0",
        "M0 pinned Java oracle smoke, smoke corpus record/replay comparison, and selected regressions (--full-corpus optional)",
        True,
        False,
        False,
    ),
    SuiteSpec(
        "m1",
        "M1 architecture experiments (frontend, runtime, Host, lifecycle, native/Node/Chromium, retained Graal report verification)",
        True,
        False,
        True,
    ),
    SuiteSpec(
        "m2",
        "M2 language differential verification (6 fast adapters or --full-corpus all 7 + --verify-full), class lifecycle, identifier tables",
        True,
        False,
        False,
    ),
    SuiteSpec(
        "m3",
        "M3 1,781-item mechanical coverage probe and 3-lane raw-byte differential strict gate",
        True,
        False,
        False,
    ),
    SuiteSpec(
        "m4",
        "M4 offline Host I/O, external readers, projects/packages (lanes 1-3 or --full-corpus all 5 Pantry 29-package lanes)",
        True,
        False,
        False,
    ),
    SuiteSpec(
        "m5",
        "M5 Rust API, CLI, REPL, server, formatter, and pkldoc 5-lane differential & coverage gate",
        True,
        False,
        False,
    ),
    SuiteSpec(
        "m6",
        "M6 Wasm & N-API 5-lane differential gate plus cross-browser Playwright smoke",
        True,
        False,
        True,
    ),
    SuiteSpec(
        "m7-fuzz",
        "M7 seeded differential fuzzer (210 generated + saved regressions + --verify-strict)",
        True,
        False,
        False,
    ),
    SuiteSpec(
        "m7-soak",
        "M7 long-running memory reclamation soak (native 6-pair or --all-soak-targets all 18 pairs + --verify-strict)",
        False,
        False,
        False,
    ),
    SuiteSpec(
        "m7-bench",
        "M7 performance & peak RSS benchmark suite (--baseline native-image|both|jvm, --verify-report / --verify-strict)",
        False,
        True,
        False,
    ),
    SuiteSpec(
        "m7-js",
        "M7 cross-browser & Node JS/Wasm performance and memory decomposition analysis",
        True,
        False,
        True,
    ),
    SuiteSpec(
        "m7-release",
        "Unified M7 release-readiness audit (`m7_manifest.py --verify-release` / `--benchmark-dir --soak-dir --fuzz-dir`)",
        False,
        False,
        False,
    ),
)

SUITE_BY_ID: dict[str, SuiteSpec] = {spec.id: spec for spec in SUITE_SPECS}

PRESETS: dict[str, tuple[str, ...]] = {
    "quick": ("workspace", "manifests", "platform-js-tests", "packages"),
    "compat": ("m0", "m1", "m2", "m3", "m4", "m5", "m6"),
    "m7": ("m7-fuzz", "m7-soak", "m7-bench", "m7-js", "m7-release"),
    "full": (
        "workspace",
        "manifests",
        "platform-js-tests",
        "packages",
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
        "m7-release",
    ),
    "full-strict": (
        "workspace",
        "manifests",
        "platform-js-tests",
        "packages",
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
        "m7-release",
    ),
}


def discover_python_test_modules(root: Path = ROOT) -> dict[str, list[str]]:
    """Partition all `tools/tests/test_*.py` files into active vs historical suites."""
    tests_dir = root / "tools" / "tests"
    all_files = sorted(p.name for p in tests_dir.glob("test_*.py"))
    active: list[str] = []
    historical: list[str] = []
    for name in all_files:
        if name in CORE_ACTIVE_PYTHON_TESTS or name.startswith(ACTIVE_MILESTONE_TEST_PREFIXES):
            active.append(f"tools/tests/{name}")
        else:
            historical.append(f"tools/tests/{name}")
    return {"active": active, "historical": historical}


def discover_platform_js_tests(root: Path = ROOT) -> list[str]:
    """Discover all Node.js unit test files under `tools/platform/*.test.mjs`."""
    platform_dir = root / "tools" / "platform"
    return sorted(f"tools/platform/{p.name}" for p in platform_dir.glob("*.test.mjs"))


def _helper_write_java_replay_runner(dest: Path) -> int:
    sys.path.insert(0, str(ROOT / "tools"))
    from oracle import ensure_oracle

    metadata = ensure_oracle(ROOT)
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(
        json.dumps({"command": metadata["command"], "kind": "java-replay-not-rust"}) + "\n",
        encoding="utf-8",
    )
    return 0


def _helper_extract_tar_gz(archive: Path, dest_dir: Path) -> int:
    dest_dir.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive, "r:gz") as tf:
        for member in tf.getmembers():
            member_path = (dest_dir / member.name).resolve()
            if not str(member_path).startswith(str(dest_dir.resolve())):
                raise ValueError(f"Unsafe tar member path: {member.name}")
        tf.extractall(dest_dir)
    return 0


def _helper_check_wasm_gzip_budget(wasm_path: Path, max_bytes: int) -> int:
    import gzip

    compressed = gzip.compress(wasm_path.read_bytes(), compresslevel=9)
    size = len(compressed)
    print(f"{wasm_path}: gzip-9 bytes = {size} (limit = {max_bytes})")
    if size > max_bytes:
        print(f"ERROR: gzip-9 size {size} exceeds budget {max_bytes}", file=sys.stderr)
        return 1
    return 0


def build_suite_steps(
    suite_id: str,
    *,
    root: Path = ROOT,
    python_exe: str,
    out_root: Path,
    full_corpus: bool = False,
    baseline: str = "native-image",
    bench_profile: str = "full",
    strict_bench: bool = False,
    browsers: str = "chromium,firefox,webkit",
    all_soak_targets: bool = False,
    m2_workers: int = 4,
    release_dir: Path | None = None,
    local_checkpoint: bool = False,
) -> list[Step]:
    if suite_id not in SUITE_BY_ID:
        raise ValueError(f"Unknown verification suite: {suite_id}")

    py = python_exe
    py_env = {"PYTHONPATH": f"{root / 'tools'}{os.pathsep}{root / 'tools' / 'tests'}"}
    suite_out = out_root / suite_id
    self_script = "tools/local_verify.py"

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
        discovered = discover_python_test_modules(root)
        active_mods = discovered["active"]
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
            Step("check-release-version-lockstep", [py, "tools/check_release_version.py"]),
            Step("verify-jvm-oracle", [py, "tools/oracle.py", "verify"]),
            Step("verify-native-image-oracle", [py, "tools/oracle.py", "verify", "--profile", "native-image"]),
            Step("unit-tests-all-active-python", [py, "-m", "unittest", *active_mods], py_env),
        ]

    if suite_id == "packages":
        return [
            Step("build-wasm-pkg", ["node", "packages/pkl-wasm/scripts/build.mjs"]),
            Step(
                "check-wasm-gzip-budget",
                [py, self_script, "--internal-check-wasm-gzip", "packages/pkl-wasm/dist/wasm/pkl_wasm_bg.wasm", "2621440"],
            ),
            Step("test-wasm-pkg", ["node", "--test", "packages/pkl-wasm/test/wasm.test.mjs"]),
            Step("build-napi-pkg", ["node", "packages/pkl-napi/scripts/build.mjs"]),
            Step("test-napi-pkg", ["node", "packages/pkl-napi/test/run.mjs"]),
            Step("test-napi-pack-install", ["node", "packages/pkl-napi/test/pack-install.mjs"]),
            Step("build-release-cli", ["cargo", "build", "--release", "--locked", "-p", "pkl-cli", "--bin", "pkl", "-p", "pkl-doc", "--bin", "pkldoc"]),
            Step("smoke-pkl-version", ["./target/release/pkl", "--version"]),
            Step("smoke-pkldoc-version", ["./target/release/pkldoc", "--version"]),
        ]

    if suite_id == "platform-js-tests":
        js_tests = discover_platform_js_tests(root)
        return [
            Step("unit-tests-platform-js", ["node", "--test", *js_tests]),
        ]

    if suite_id == "py-historical":
        discovered = discover_python_test_modules(root)
        historical_mods = discovered["historical"]
        return [
            Step("unit-tests-historical-m2-python", [py, "-m", "unittest", *historical_mods], py_env),
        ]

    if suite_id == "protobuf-strict":
        capture_dir = suite_out / "capture"
        return [
            Step(
                "protobuf-strict-ignored-gate",
                [
                    "cargo",
                    "test",
                    "-p",
                    "pkl-runtime",
                    "--test",
                    "m2_protobuf_java04_full",
                    "--locked",
                    "java04_strict_full_compatibility_gate",
                    "--",
                    "--ignored",
                    "--exact",
                    "--test-threads=1",
                    "--nocapture",
                ],
                {"PKL_PROTOBUF_CAPTURE_DIR": str(capture_dir.resolve())},
            ),
        ]

    if suite_id == "m0":
        baseline_dir = suite_out / "baseline"
        replay_runner = suite_out / "java-replay.json"
        replay_dir = suite_out / "replay"
        regressions_dir = suite_out / "regressions"
        steps = [
            Step("oracle-smoke", [py, "tools/oracle.py", "smoke"]),
            Step("compat-record-smoke", [py, "tools/compat.py", "record", "--manifest", "compatibility/smoke.json", "--output", str(baseline_dir)]),
            Step("compat-check-smoke", [py, "tools/compat.py", "check", "--manifest", "compatibility/smoke.json", "--output", str(baseline_dir)]),
            Step("compat-write-java-replay-runner", [py, self_script, "--internal-write-java-replay", str(replay_runner)]),
            Step(
                "compat-compare-java-replay",
                [
                    py,
                    "tools/compat.py",
                    "compare",
                    "--manifest",
                    "compatibility/smoke.json",
                    "--expected",
                    str(baseline_dir),
                    "--runner",
                    str(replay_runner),
                    "--output",
                    str(replay_dir),
                ],
            ),
            Step("compat-check-java-replay", [py, "tools/compat.py", "check", "--manifest", "compatibility/smoke.json", "--output", str(replay_dir)]),
            Step("compat-record-regressions", [py, "tools/compat.py", "record", "--manifest", "compatibility/regressions.json", "--output", str(regressions_dir)]),
            Step("compat-check-regressions", [py, "tools/compat.py", "check", "--manifest", "compatibility/regressions.json", "--output", str(regressions_dir)]),
        ]
        if full_corpus:
            steps.append(Step("compat-record-full-corpus", [py, "tools/compat.py", "record", "--output", str(suite_out / "full-corpus")]))
        return steps

    if suite_id == "m1":
        graal_dir = suite_out / "retained-graal"
        return [
            Step("m1-contracts", [py, "tools/m1_frontend.py", "--check-contract"]),
            Step("m1-semantics-check", [py, "tools/m1_semantics.py", "--check"]),
            Step("m1-graal-evidence-verify", [py, "tools/evidence.py", "verify", "reports/m1/graal-evidence.tar.gz"]),
            Step("m1-graal-cause-evidence-verify", [py, "tools/evidence.py", "verify", "reports/m1/graal-cause-evidence.tar.gz"]),
            Step("m1-graal-extract", [py, self_script, "--internal-extract-tar-gz", "reports/m1/graal-evidence.tar.gz", str(graal_dir)]),
            Step("m1-graal-check-report", [py, "tools/m1_graal.py", "--check-report", str(graal_dir)]),
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
        steps = [
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
        if strict_bench:
            steps.append(Step("m7-bench-verify-strict", [py, "tools/m7_benchmark.py", "--verify-strict", str(bench_dir)], py_env))
        return steps

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
        if release_dir is not None:
            cmd = [py, "tools/m7_manifest.py", "--verify-release", str(release_dir)]
        else:
            cmd = [
                py,
                "tools/m7_manifest.py",
                "--benchmark-dir",
                str(out_root / "m7-bench" / "bench"),
                "--soak-dir",
                str(out_root / "m7-soak" / "soak"),
                "--fuzz-dir",
                str(out_root / "m7-fuzz" / "fuzz"),
            ]
        if local_checkpoint:
            cmd.append("--local-checkpoint")
        return [Step("m7-verify-release", cmd, py_env)]

    raise AssertionError(f"Unhandled suite: {suite_id}")


def resolve_suite_selection(
    suites: list[str],
    preset: str | None,
    *,
    include_historical_tests: bool = False,
) -> list[str]:
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
    if include_historical_tests and "py-historical" not in selected:
        selected.append("py-historical")
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
    strict_bench: bool = False,
    browsers: str = "chromium,firefox,webkit",
    all_soak_targets: bool = False,
    m2_workers: int = 4,
    release_dir: Path | None = None,
    local_checkpoint: bool = False,
) -> dict[str, Any]:
    suite_results: list[dict[str, Any]] = []
    overall_ok = True

    if not dry_run:
        out_root.mkdir(parents=True, exist_ok=True)

    for suite_id in suite_ids:
        steps = build_suite_steps(
            suite_id,
            root=root,
            python_exe=python_exe,
            out_root=out_root,
            full_corpus=full_corpus,
            baseline=baseline,
            bench_profile=bench_profile,
            strict_bench=strict_bench,
            browsers=browsers,
            all_soak_targets=all_soak_targets,
            m2_workers=m2_workers,
            release_dir=release_dir,
            local_checkpoint=local_checkpoint,
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
    parser.add_argument("--include-historical-tests", action="store_true", help="Also run all 75 historical M2 Python fixture/compare suites")
    parser.add_argument("--output-root", type=Path, default=None, help="Fresh output directory for evidence")
    parser.add_argument("--dry-run", action="store_true", help="Print planned commands without running them")
    parser.add_argument("--continue-on-error", action="store_true", help="Continue remaining steps/suites on failure")
    parser.add_argument("--full-corpus", action="store_true", help="Enable full M0/M2 (4,575) / M4 Pantry (all 5 lanes) corpus scopes")
    parser.add_argument("--baseline", choices=["native-image", "both", "jvm"], default=None, help="M7 benchmark baseline (default: native-image, or both for full-strict)")
    parser.add_argument("--bench-profile", choices=["full", "quick"], default="full", help="M7 benchmark profile")
    parser.add_argument("--strict-bench", action="store_true", help="Also run `m7_benchmark.py --verify-strict`")
    parser.add_argument("--browsers", default="chromium,firefox,webkit", help="Comma-separated Playwright browsers")
    parser.add_argument("--all-soak-targets", action="store_true", help="Run all 18 soak pairs (native + napi + wasm)")
    parser.add_argument("--m2-workers", type=int, default=4, help="Worker count for M2 language replay")
    parser.add_argument("--release-dir", type=Path, default=None, help="Existing M7 evidence dir for m7-release audit")
    parser.add_argument("--local-checkpoint", action="store_true", help="Pass --local-checkpoint to m7_manifest.py in m7-release")
    # Internal step helpers (invoked by generated step commands so execution stays OS-portable).
    parser.add_argument("--internal-write-java-replay", type=Path, default=None, help=argparse.SUPPRESS)
    parser.add_argument("--internal-extract-tar-gz", nargs=2, metavar=("ARCHIVE", "DEST"), default=None, help=argparse.SUPPRESS)
    parser.add_argument("--internal-check-wasm-gzip", nargs=2, metavar=("WASM", "MAX_BYTES"), default=None, help=argparse.SUPPRESS)
    args = parser.parse_args(argv)

    if args.internal_write_java_replay is not None:
        return _helper_write_java_replay_runner(args.internal_write_java_replay)
    if args.internal_extract_tar_gz is not None:
        return _helper_extract_tar_gz(Path(args.internal_extract_tar_gz[0]), Path(args.internal_extract_tar_gz[1]))
    if args.internal_check_wasm_gzip is not None:
        return _helper_check_wasm_gzip_budget(Path(args.internal_check_wasm_gzip[0]), int(args.internal_check_wasm_gzip[1]))

    if args.list:
        discovered = discover_python_test_modules(ROOT)
        js_tests = discover_platform_js_tests(ROOT)
        print(
            f"Test discovery: {len(discovered['active'])} active Python modules, "
            f"{len(discovered['historical'])} historical Python modules, "
            f"{len(js_tests)} platform Node.js test files\n"
        )
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
            print(f"  {spec.id:16s} {spec.description}{tag}")
        print("\nPresets:")
        for name, members in PRESETS.items():
            extra = " (forces --full-corpus, --all-soak-targets, --baseline both, --strict-bench)" if name == "full-strict" else ""
            print(f"  {name:16s} {', '.join(members)}{extra}")
        return 0

    try:
        selected = resolve_suite_selection(
            args.suite,
            args.preset,
            include_historical_tests=args.include_historical_tests,
        )
    except ValueError as exc:
        print(f"local_verify: {exc}", file=sys.stderr)
        return 2

    is_full_strict = args.preset == "full-strict"
    full_corpus = args.full_corpus or is_full_strict
    all_soak_targets = args.all_soak_targets or is_full_strict
    strict_bench = args.strict_bench or is_full_strict
    baseline = args.baseline if args.baseline is not None else ("both" if is_full_strict else "native-image")

    ts = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
    out_root = args.output_root if args.output_root is not None else (ROOT / "target" / "local-verify" / ts)

    summary = run_suites(
        selected,
        python_exe=sys.executable,
        out_root=out_root,
        dry_run=args.dry_run,
        continue_on_error=args.continue_on_error,
        full_corpus=full_corpus,
        baseline=baseline,
        bench_profile=args.bench_profile,
        strict_bench=strict_bench,
        browsers=args.browsers,
        all_soak_targets=all_soak_targets,
        m2_workers=args.m2_workers,
        release_dir=args.release_dir,
        local_checkpoint=args.local_checkpoint,
    )
    print(json.dumps(summary, indent=2, ensure_ascii=False))
    return 0 if summary["overall_status"] in ("pass", "planned") else 1


if __name__ == "__main__":
    sys.exit(main())
