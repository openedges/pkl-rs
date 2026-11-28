#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Unified Milestone M7 Release-Readiness Manifest & Verification Gate.

Aggregates ALL prior milestone inventories (M0-M6) together with M7 performance &
peak RSS benchmarks (`tools/m7_benchmark.py`), long-running memory reclamation soak
(`tools/m7_memory_soak.py`), seeded differential fuzzing (`tools/m7_fuzz.py`),
and explicit OS/arch/browser/hosted-CI platform matrix accounting.

Strict honesty invariants:
  - `--check` verifies manifest, mapping, fixture, and prior-manifest freshness ONLY
    (`coverage_gate: null`, NEVER claims runtime passes or performance parity).
  - `--verify-coverage <dir>` / `--verify-release <dir>` independently re-verifies
    SHA-256 sealed M7 benchmark, soak, and fuzz reports via their own verifiers:
      * Requires 0 raw-byte fuzz mismatches, 0 memory leaks, verified pinned Temurin
        21.0.13.1+2-LTS (`exact_jdk_lock_match: true`), `recorded-build`, and
        `source_stability: true`.
      * Explicitly enumerates EVERY workload whose Rust/Java execution time or
        peak RSS ratio exceeds 1.00 or is `inconclusive` (NEVER hides regressions
        behind averages!).
      * Explicitly enumerates EVERY OS/architecture/browser/hosted-CI target in the
        release platform matrix that lacks verified evidence (`release_ready: false`
        whenever any required platform or workload target remains unmet!).
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import sys
from typing import Any

import m7_benchmark
import m7_fuzz
import m7_memory_soak

ROOT = Path(__file__).resolve().parents[1]
INVENTORY_PATH = Path("compatibility/inventory.json")
MAPPINGS_PATH = Path("compatibility/m7-mappings.json")
OUTPUT_PATH = Path("compatibility/m7-release.json")

ALLOWED_LINK_SUFFIXES = {".rs", ".py", ".mjs", ".cjs", ".js", ".ts", ".yml", ".yaml", ".json"}
VALID_TRACKS = {"benchmark", "soak", "fuzz", "release-matrix"}
HEX_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
HEX_COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
REQUIRED_CI_NATIVE_PLATFORMS = (
    "darwin-arm64",
    "darwin-x64",
    "linux-x64-gnu",
    "linux-arm64-gnu",
    "linux-x64-musl",
    "linux-arm64-musl",
    "win32-x64-msvc",
)
REQUIRED_PUBLISHABLE_CRATES = (
    "pkl-runtime",
    "pkl-formatter",
    "pkl-host-native",
    "pkl-lang",
    "pkl-repl",
    "pkl-server",
    "pkl-doc",
    "pkl-cli",
)
REQUIRED_JS_BROWSER_WORKLOADS = ("eval-text", "typed-conversion")
BROWSER_TO_PLATFORM_ID = {
    "chromium": "browser-chromium",
    "firefox": "browser-firefox",
    "webkit": "browser-webkit",
}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encoded(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode("utf-8")


def read_properties(text: str) -> dict[str, str]:
    out: dict[str, str] = {}
    for line in text.splitlines():
        if not line or line.startswith("#"):
            continue
        k, v = line.split("=", 1)
        out[k] = v
    return out


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_bytes())


def expected_benchmark_workload_ids(contract: dict[str, Any]) -> set[str]:
    ids: set[str] = set()
    for w in contract["core_workloads"]:
        for mode in w["modes"]:
            ids.add(f"{w['id']}:{mode}")
    for w in contract["product_workloads"]:
        kind = w["kind"]
        if kind == "eval_cold_and_persistent":
            ids.add(f"{w['id']}:cold-cli")
            ids.add(f"{w['id']}:persistent")
        elif kind == "server_msgpack":
            ids.add(f"{w['id']}:steady-server")
        elif kind == "js_binding":
            ids.add(f"{w['id']}:{w['target']}")
        else:
            ids.add(f"{w['id']}:cold-cli")
    return ids


def expected_soak_pairs() -> set[str]:
    return {f"{t}/{s}" for t in m7_memory_soak.ALL_TARGETS for s in m7_memory_soak.ALL_SCENARIOS}


def expected_fuzz_categories() -> set[str]:
    return set(m7_fuzz.ALL_CATEGORIES)


def validate_baseline(root: Path, inventory: dict[str, Any], mappings: dict[str, Any], bench_contract: dict[str, Any]) -> None:
    refs = read_properties((root / "references.lock").read_text(encoding="utf-8"))
    baseline = inventory.get("baseline", {})
    if baseline.get("version") != refs.get("baseline.pkl") or baseline.get("revision") != refs.get("pkl.revision"):
        raise ValueError("inventory.json baseline disagrees with references.lock")
    expected_base = {"version": baseline["version"], "revision": baseline["revision"]}
    if mappings.get("baseline") != expected_base:
        raise ValueError("compatibility/m7-mappings.json baseline disagrees with inventory.json")
    if bench_contract.get("baseline") != baseline["version"]:
        raise ValueError("benchmarks/m7_workloads.json baseline disagrees with inventory.json")


def _valid_link_path(p: Path) -> bool:
    if p.is_absolute() or ".." in p.parts:
        return False
    return p.suffix in ALLOWED_LINK_SUFFIXES


def validate_prior_milestones(root: Path, mappings: dict[str, Any]) -> list[dict[str, Any]]:
    rows = mappings.get("prior_milestones", [])
    if len(rows) < 6:
        raise ValueError("m7-mappings.json must reference all prior milestone manifests (M0, M2-M6)")
    resolved: list[dict[str, Any]] = []
    seen: set[str] = set()
    for row in rows:
        ms = row["milestone"]
        if ms in seen:
            raise ValueError(f"duplicate prior milestone: {ms}")
        seen.add(ms)
        rel = Path(row["manifest"])
        if rel.is_absolute() or ".." in rel.parts:
            raise ValueError(f"unsafe prior manifest path: {rel}")
        full = root / rel
        if not full.is_file():
            raise ValueError(f"missing prior milestone manifest for {ms}: {rel}")
        raw = full.read_bytes()
        doc = json.loads(raw)
        if doc.get("baseline", {}).get("version") != "0.32.0":
            raise ValueError(f"prior milestone manifest {rel} baseline version != 0.32.0")
        resolved.append(
            {
                "milestone": ms,
                "manifest": rel.as_posix(),
                "sha256": digest(raw),
                "bytes": len(raw),
                "description": row.get("description", ""),
                "counts": doc.get("counts") or {},
            }
        )
    return resolved


def validate_mappings(
    root: Path,
    mappings: dict[str, Any],
    bench_contract: dict[str, Any],
) -> dict[str, Any]:
    if mappings.get("schema_version") != 1:
        raise ValueError("invalid compatibility/m7-mappings.json schema_version")
    reqs = mappings.get("requirements", {})
    if not reqs:
        raise ValueError("compatibility/m7-mappings.json requirements map is empty")

    all_bench_ids = expected_benchmark_workload_ids(bench_contract)
    all_soak_pairs = expected_soak_pairs()
    all_fuzz_cats = expected_fuzz_categories()

    covered_bench: set[str] = set()
    covered_soak: set[str] = set()
    covered_fuzz: set[str] = set()
    validated: dict[str, Any] = {}

    for rid, row in sorted(reqs.items()):
        track = row.get("track")
        if track not in VALID_TRACKS:
            raise ValueError(f"invalid track for {rid}: {track}")
        impl_status = row.get("implementation_status", "linked")
        if impl_status not in ("linked", "pending"):
            raise ValueError(f"invalid implementation_status for {rid}: {impl_status}")

        resolved_links = []
        for link in row.get("implementation_links", []):
            rel, sym = link["path"], link["symbol"]
            p = Path(rel)
            if not _valid_link_path(p):
                raise ValueError(f"invalid implementation link path for {rid}: {rel}")
            full = root / p
            if not full.is_file():
                raise ValueError(f"missing implementation link file for {rid}: {rel}")
            src = full.read_bytes()
            text = src.decode("utf-8")
            if not re.search(r"(?<![A-Za-z0-9_])" + re.escape(sym) + r"(?![A-Za-z0-9_])", text):
                raise ValueError(f"implementation link symbol missing for {rid}: {rel}::{sym}")
            resolved_links.append({"path": rel, "symbol": sym, "sha256": digest(src)})

        if impl_status == "linked" and not resolved_links:
            raise ValueError(f"requirement {rid} marked linked but has empty implementation_links")

        entry: dict[str, Any] = {
            "track": track,
            "description": row.get("description", ""),
            "implementation_status": impl_status,
            "implementation_links": resolved_links,
        }

        if track == "benchmark":
            wids = row.get("workload_ids", [])
            if not wids:
                raise ValueError(f"benchmark requirement {rid} has empty workload_ids")
            for wid in wids:
                if wid not in all_bench_ids:
                    raise ValueError(f"benchmark requirement {rid} references unknown workload_id: {wid}")
                if wid in covered_bench:
                    raise ValueError(f"duplicate benchmark workload_id mapping: {wid}")
                covered_bench.add(wid)
            entry["workload_ids"] = list(wids)
        elif track == "soak":
            pairs = row.get("soak_pairs", [])
            if not pairs:
                raise ValueError(f"soak requirement {rid} has empty soak_pairs")
            for pair in pairs:
                if pair not in all_soak_pairs:
                    raise ValueError(f"soak requirement {rid} references unknown soak_pair: {pair}")
                if pair in covered_soak:
                    raise ValueError(f"duplicate soak_pair mapping: {pair}")
                covered_soak.add(pair)
            entry["soak_pairs"] = list(pairs)
        elif track == "fuzz":
            cats = row.get("fuzz_categories", [])
            if not cats:
                raise ValueError(f"fuzz requirement {rid} has empty fuzz_categories")
            for cat in cats:
                if cat not in all_fuzz_cats:
                    raise ValueError(f"fuzz requirement {rid} references unknown fuzz category: {cat}")
                if cat in covered_fuzz:
                    raise ValueError(f"duplicate fuzz category mapping: {cat}")
                covered_fuzz.add(cat)
            entry["fuzz_categories"] = list(cats)

        validated[rid] = entry

    unmapped_bench = sorted(all_bench_ids - covered_bench)
    if unmapped_bench:
        raise ValueError(f"unmapped M7 benchmark workloads: {unmapped_bench}")
    unmapped_soak = sorted(all_soak_pairs - covered_soak)
    if unmapped_soak:
        raise ValueError(f"unmapped M7 soak (target/scenario) pairs: {unmapped_soak}")
    unmapped_fuzz = sorted(all_fuzz_cats - covered_fuzz)
    if unmapped_fuzz:
        raise ValueError(f"unmapped M7 fuzz categories: {unmapped_fuzz}")

    return validated


def build_manifest(root: Path = ROOT) -> dict[str, Any]:
    root = root.resolve()
    inventory = load_json(root / INVENTORY_PATH)
    mappings = load_json(root / MAPPINGS_PATH)
    bench_contract = m7_benchmark.load_contract(root)

    validate_baseline(root, inventory, mappings, bench_contract)
    prior = validate_prior_milestones(root, mappings)
    validated_reqs = validate_mappings(root, mappings, bench_contract)

    sources = {
        "references.lock": digest((root / "references.lock").read_bytes()),
        "oracle.lock.json": digest((root / "oracle.lock.json").read_bytes()),
        "tools/oracle.py": digest((root / "tools/oracle.py").read_bytes()),
        INVENTORY_PATH.as_posix(): digest((root / INVENTORY_PATH).read_bytes()),
        MAPPINGS_PATH.as_posix(): digest((root / MAPPINGS_PATH).read_bytes()),
        m7_benchmark.WORKLOADS_PATH.as_posix(): digest((root / m7_benchmark.WORKLOADS_PATH).read_bytes()),
        m7_benchmark.M0_GRAALVM_REF_PATH.as_posix(): digest((root / m7_benchmark.M0_GRAALVM_REF_PATH).read_bytes()),
        "tools/m7_benchmark.py": digest((root / "tools/m7_benchmark.py").read_bytes()),
        "tools/m7_memory_soak.py": digest((root / "tools/m7_memory_soak.py").read_bytes()),
        "tools/m7_fuzz.py": digest((root / "tools/m7_fuzz.py").read_bytes()),
    }
    for pm in prior:
        sources[pm["manifest"]] = pm["sha256"]

    native_lock = m7_benchmark.validate_native_image_lock(root)
    _, _, native_ids = m7_benchmark.expected_workload_sets(bench_contract)
    pinned_native_cfg = mappings.get("pinned_native_image_reference")
    if not isinstance(pinned_native_cfg, dict) or pinned_native_cfg.get("status") != "pinned-required-release-baseline":
        raise ValueError(
            "compatibility/m7-mappings.json must promote native-image-reference to pinned-required-release-baseline"
        )
    pinned_native_ref = {
        "id": pinned_native_cfg["id"],
        "status": pinned_native_cfg["status"],
        "baseline_version": inventory["baseline"]["version"],
        "lock_file": "oracle.lock.json",
        "scope": pinned_native_cfg["scope"],
        "required_workload_count": len(native_ids),
        "required_workload_ids": sorted(native_ids),
        "pinned_platforms": sorted(native_lock["distributions"].keys()),
        "distributions": native_lock["distributions"],
    }

    track_counts = dict(sorted(Counter(r["track"] for r in validated_reqs.values()).items()))
    impl_counts = dict(sorted(Counter(r["implementation_status"] for r in validated_reqs.values()).items()))
    platform_matrix = mappings.get("platform_matrix", [])

    return {
        "schema_version": 1,
        "baseline": inventory["baseline"],
        "scope": mappings["scope"],
        "target_ratios": bench_contract["target_ratios"],
        "native_image_targets": bench_contract["native_image_targets"],
        "browser_wasm_targets": bench_contract["browser_wasm_targets"],
        "pinned_native_image_reference": pinned_native_ref,
        "excluded_by_scope_2026_11_20": mappings["excluded_by_scope_2026_11_20"],
        "unpinned_optional_references": mappings["unpinned_optional_references"],
        "platform_matrix": platform_matrix,
        "prior_milestones": prior,
        "sources": dict(sorted(sources.items())),
        "counts": {
            "m7_requirement_groups": len(validated_reqs),
            "m7_requirements_by_track": track_counts,
            "m7_requirement_impl_status": impl_counts,
            "m7_benchmark_workload_modes": len(expected_benchmark_workload_ids(bench_contract)),
            "m7_native_image_required_workloads": len(native_ids),
            "m7_native_image_pinned_platforms": len(native_lock["distributions"]),
            "m7_soak_target_scenarios": len(expected_soak_pairs()),
            "m7_fuzz_categories": len(expected_fuzz_categories()),
            "prior_milestone_manifests": len(prior),
            "platform_matrix_entries": len(platform_matrix),
            "excluded_scope_entries": len(mappings["excluded_by_scope_2026_11_20"]),
            "unpinned_optional_references": len(mappings["unpinned_optional_references"]),
        },
        "requirements": validated_reqs,
        "coverage_gate": None,
    }


def _detect_platforms(
    bench_report: dict[str, Any] | None,
    soak_report: dict[str, Any] | None,
    fuzz_report: dict[str, Any] | None,
    *,
    bench_strict_ok: bool,
    soak_strict_ok: bool,
    fuzz_strict_ok: bool,
) -> tuple[set[str], set[str]]:
    """Return `(verified_platforms, observed_non_passing_platforms)`.

    Per AGENTS.md honesty rules, a platform is counted in `verified_platforms` ONLY
    when its required strict gate actually PASSED. Executed hosts/browsers whose
    strict gate failed are reported separately in `observed_non_passing_platforms`.
    """
    observed_hosts: set[str] = set()
    for rep in (bench_report, soak_report, fuzz_report):
        if not rep:
            continue
        sys_name = None
        arch = None
        libc = None
        if "machine_start" in rep:
            sys_name = rep["machine_start"].get("system")
            arch = rep["machine_start"].get("machine")
            libc = rep["machine_start"].get("libc")
        elif "host" in rep:
            sys_name = rep["host"].get("system")
            arch = rep["host"].get("machine")
            libc = rep["host"].get("libc")
        linux_abi = "musl" if libc == "musl" else "gnu"
        if sys_name == "Darwin" and arch == "arm64":
            observed_hosts.add("darwin-arm64")
        elif sys_name == "Darwin" and arch == "x86_64":
            observed_hosts.add("darwin-x64")
        elif sys_name == "Linux" and arch in ("x86_64", "amd64"):
            observed_hosts.add(f"linux-x64-{linux_abi}")
        elif sys_name == "Linux" and arch in ("aarch64", "arm64"):
            observed_hosts.add(f"linux-arm64-{linux_abi}")
        elif sys_name == "Windows" and arch in ("AMD64", "x86_64"):
            observed_hosts.add("win32-x64-msvc")

    verified: set[str] = set()
    observed_non_passing: set[str] = set()
    if bench_strict_ok and soak_strict_ok and fuzz_strict_ok:
        verified.update(observed_hosts)
    else:
        observed_non_passing.update(observed_hosts)

    if bench_report:
        browser_rows = [r for r in bench_report.get("results", []) if r.get("family") == "wasm-browser"]
        if browser_rows:
            all_browser_pass = (
                bench_strict_ok
                and all(
                    r.get("correctness_verified")
                    and not r.get("rust", {}).get("browserErrors")
                    and r.get("vs_live_temurin_21", {}).get("verdict") == "pass"
                    for r in browser_rows
                )
            )
            if all_browser_pass:
                verified.add("browser-chromium")
            else:
                observed_non_passing.add("browser-chromium")

    return verified, observed_non_passing


def verify_js_analysis_report(
    js_analysis_dir: Path,
    *,
    browser_smoke_dir: Path | None = None,
) -> dict[str, Any]:
    """Verify a multi-browser `tools/m7_js_analysis.py` output directory.

    Each browser (`chromium`, `firefox`, `webkit`) is promoted to `verified_platforms`
    ONLY when both required browser workloads (`eval-text`, `typed-conversion`) pass the
    2026-11-22 browser Wasm contract with `preflight_verified == True`, and (if smoke
    results are supplied or present under `<dir>/smoke/report.json`) the functional smoke
    passed for that browser.
    """
    report_path = js_analysis_dir / "report.json"
    if not report_path.is_file():
        raise ValueError(f"missing JS analysis report: {report_path}")
    raw = report_path.read_bytes()
    doc = json.loads(raw)
    if doc.get("schema_version") != 1 or doc.get("mode") != "js-analysis":
        raise ValueError(f"invalid JS analysis report header in {report_path}")
    results = doc.get("results")
    if not isinstance(results, list) or not results:
        raise ValueError(f"JS analysis report has empty results: {report_path}")

    smoke_by_browser: dict[str, str] = {}
    smoke_path = None
    if browser_smoke_dir is not None:
        smoke_path = browser_smoke_dir / "report.json"
        if not smoke_path.is_file():
            raise ValueError(f"missing browser smoke report: {smoke_path}")
    elif (js_analysis_dir / "smoke" / "report.json").is_file():
        smoke_path = js_analysis_dir / "smoke" / "report.json"
    if smoke_path is not None:
        smoke_doc = json.loads(smoke_path.read_bytes())
        if smoke_doc.get("schema_version") != 1 or smoke_doc.get("mode") != "smoke-browsers":
            raise ValueError(f"invalid browser smoke report header in {smoke_path}")
        for bname, brow in (smoke_doc.get("results") or {}).items():
            smoke_by_browser[str(bname)] = str(brow.get("status", "unknown"))

    by_browser: dict[str, dict[str, dict[str, Any]]] = {}
    for row in results:
        if row.get("kind") != "wasm-browser":
            continue
        engine = row.get("browser_engine")
        if engine not in BROWSER_TO_PLATFORM_ID:
            continue
        # Ignore diagnostic `--chromium-no-gpu` rows when gating standard browser targets.
        if row.get("launch_args"):
            continue
        workload = str(row.get("workload", ""))
        by_browser.setdefault(engine, {})[workload] = row

    verified_browsers: set[str] = set()
    non_passing_browsers: set[str] = set()
    browser_details: dict[str, Any] = {}
    for engine, pid in sorted(BROWSER_TO_PLATFORM_ID.items()):
        wmap = by_browser.get(engine)
        if not wmap:
            continue
        smoke_ok = (engine not in smoke_by_browser) or (smoke_by_browser[engine] == "pass")
        all_workloads_ok = all(
            (w in wmap)
            and wmap[w].get("status") == "ok"
            and bool(wmap[w].get("preflight_verified"))
            and (wmap[w].get("contract_verdict") or {}).get("verdict") == "pass"
            for w in REQUIRED_JS_BROWSER_WORKLOADS
        )
        passed = smoke_ok and all_workloads_ok
        browser_details[pid] = {
            "engine": engine,
            "workloads": {
                w: {
                    "status": wmap[w].get("status") if w in wmap else "missing",
                    "verdict": (wmap[w].get("contract_verdict") or {}).get("verdict") if w in wmap else "missing",
                    "time_ratio_vs_java": wmap[w].get("time_ratio_vs_java") if w in wmap else None,
                    "wasm_linear_memory_bytes": wmap[w].get("wasm_linear_memory_bytes") if w in wmap else None,
                }
                for w in REQUIRED_JS_BROWSER_WORKLOADS
            },
            "smoke_status": smoke_by_browser.get(engine),
            "passed": passed,
        }
        if passed:
            verified_browsers.add(pid)
        else:
            non_passing_browsers.add(pid)

    return {
        "report_path": str(report_path),
        "report_sha256": digest(raw),
        "profile": doc.get("profile"),
        "wasm_binary_gzip_bytes": doc.get("wasm_binary_gzip_bytes"),
        "verified_platforms": verified_browsers,
        "observed_non_passing_platforms": non_passing_browsers,
        "browsers": browser_details,
    }


def verify_ci_attestation(
    attestation_path: Path,
    *,
    expected_commit: str | None = None,
) -> dict[str, Any]:
    """Verify a structured hosted-CI release attestation generated by `.github/workflows/release.yml`.

    Never accepts local provenance as `hosted-ci`, requires valid SHA-256 digests for
    built artifacts on every declared platform, and enforces exact `source_commit` match
    when cross-checked against local benchmark/soak/fuzz evidence.
    """
    if not attestation_path.is_file():
        raise ValueError(f"missing CI attestation file: {attestation_path}")
    raw = attestation_path.read_bytes()
    doc = json.loads(raw)
    if doc.get("schema_version") != 1:
        raise ValueError("CI attestation schema_version must be 1")
    if doc.get("provenance") != "hosted-ci":
        raise ValueError(f"CI attestation provenance must be 'hosted-ci', got {doc.get('provenance')!r}")
    if doc.get("workflow") != "Release":
        raise ValueError(f"CI attestation workflow must be 'Release', got {doc.get('workflow')!r}")

    repo = doc.get("github_repository")
    if not isinstance(repo, str) or "/" not in repo or not repo.strip():
        raise ValueError("CI attestation missing valid github_repository")
    run_id = str(doc.get("github_run_id", "")).strip()
    if not run_id.isdigit():
        raise ValueError(f"CI attestation missing numeric github_run_id: {run_id!r}")
    commit = doc.get("source_commit")
    if not isinstance(commit, str) or not HEX_COMMIT_RE.match(commit):
        raise ValueError(f"CI attestation invalid source_commit: {commit!r}")
    if expected_commit and commit != expected_commit:
        raise ValueError(
            f"CI attestation source_commit ({commit}) does not match local verification git_head ({expected_commit})"
        )
    checksums_sha = doc.get("checksums_sha256")
    if not isinstance(checksums_sha, str) or not HEX_SHA256_RE.match(checksums_sha):
        raise ValueError("CI attestation missing valid checksums_sha256")

    wasm_pkg = doc.get("wasm_package")
    if not isinstance(wasm_pkg, dict):
        raise ValueError("CI attestation missing wasm_package section")
    if not wasm_pkg.get("tests_passed"):
        raise ValueError("CI attestation wasm_package.tests_passed is not true")
    if not HEX_SHA256_RE.match(str(wasm_pkg.get("sha256", ""))):
        raise ValueError("CI attestation wasm_package missing valid sha256")
    gzip_bytes = wasm_pkg.get("gzip_bytes")
    if not isinstance(gzip_bytes, int) or gzip_bytes <= 0 or gzip_bytes > 2_621_440:
        raise ValueError(f"CI attestation wasm_package gzip_bytes out of budget: {gzip_bytes!r}")

    crates = doc.get("crates_packaged")
    if not isinstance(crates, list) or not set(REQUIRED_PUBLISHABLE_CRATES).issubset(set(crates)):
        missing_crates = sorted(set(REQUIRED_PUBLISHABLE_CRATES) - set(crates or []))
        raise ValueError(f"CI attestation missing packaged crates: {missing_crates}")

    platforms = doc.get("platforms")
    if not isinstance(platforms, dict) or not platforms:
        raise ValueError("CI attestation missing platforms map")

    verified: set[str] = set()
    non_passing: set[str] = set()
    for pid in REQUIRED_CI_NATIVE_PLATFORMS:
        entry = platforms.get(pid)
        if not isinstance(entry, dict):
            continue
        ok = (
            entry.get("status") == "pass"
            and bool(entry.get("smoke_verified"))
            and isinstance(entry.get("cli_archive"), dict)
            and HEX_SHA256_RE.match(str(entry["cli_archive"].get("sha256", "")))
            and isinstance(entry.get("pkldoc_archive"), dict)
            and HEX_SHA256_RE.match(str(entry["pkldoc_archive"].get("sha256", "")))
            and isinstance(entry.get("napi_addon"), dict)
            and HEX_SHA256_RE.match(str(entry["napi_addon"].get("sha256", "")))
        )
        if ok:
            verified.add(pid)
        else:
            non_passing.add(pid)

    if set(REQUIRED_CI_NATIVE_PLATFORMS).issubset(verified) and not non_passing:
        verified.add("hosted-ci")
    else:
        non_passing.add("hosted-ci")

    return {
        "attestation_path": str(attestation_path),
        "attestation_sha256": digest(raw),
        "provenance": "hosted-ci",
        "workflow": doc["workflow"],
        "github_repository": repo,
        "github_run_id": run_id,
        "source_commit": commit,
        "checksums_sha256": checksums_sha,
        "verified_platforms": verified,
        "observed_non_passing_platforms": non_passing,
    }


def _extract_bundle_git_head(
    bench_rep: dict[str, Any] | None,
    soak_rep: dict[str, Any] | None,
    fuzz_rep: dict[str, Any] | None,
    js_rep: dict[str, Any] | None = None,
) -> tuple[str | None, list[str]]:
    heads: set[str] = set()
    for rep in (bench_rep, soak_rep, fuzz_rep, js_rep):
        if not isinstance(rep, dict):
            continue
        head = (rep.get("source") or {}).get("git_head") or rep.get("git_head")
        if isinstance(head, str) and HEX_COMMIT_RE.match(head):
            heads.add(head)
    if len(heads) == 1:
        return next(iter(heads)), []
    if len(heads) > 1:
        return None, [f"mismatched git_head commits across M7 reports: {sorted(heads)}"]
    return None, []


def verify_release(
    root: Path,
    *,
    benchmark_dir: Path | None = None,
    soak_dir: Path | None = None,
    fuzz_dir: Path | None = None,
    js_analysis_dir: Path | None = None,
    browser_smoke_dir: Path | None = None,
    ci_attestation_path: Path | None = None,
    bundle_dir: Path | None = None,
    local_checkpoint: bool = False,
) -> dict[str, Any]:
    root = root.resolve()
    manifest = build_manifest(root)
    mappings = load_json(root / MAPPINGS_PATH)

    if bundle_dir is not None:
        b = bundle_dir.resolve()
        if benchmark_dir is None and (b / "benchmark").is_dir():
            benchmark_dir = b / "benchmark"
        if soak_dir is None and (b / "soak").is_dir():
            soak_dir = b / "soak"
        if fuzz_dir is None and (b / "fuzz").is_dir():
            fuzz_dir = b / "fuzz"
        if js_analysis_dir is None and (b / "js-analysis").is_dir():
            js_analysis_dir = b / "js-analysis"
        if browser_smoke_dir is None and (b / "browser-smoke").is_dir():
            browser_smoke_dir = b / "browser-smoke"
        if ci_attestation_path is None and (b / "ci-attestation.json").is_file():
            ci_attestation_path = b / "ci-attestation.json"

    integrity_failures: list[str] = []
    strict_failures: list[str] = []

    # 1. Benchmark verification
    bench_rep: dict[str, Any] | None = None
    bench_strict_ok = False
    unmet_native_image: list[dict[str, Any]] = []
    unmet_perf: list[dict[str, Any]] = []
    unmet_graalvm: list[dict[str, Any]] = []
    if benchmark_dir is None or not benchmark_dir.is_dir():
        integrity_failures.append("missing M7 benchmark report directory")
        strict_failures.append("missing M7 benchmark report directory")
    else:
        try:
            bench_rep = m7_benchmark.verify_report(benchmark_dir, root=root, strict=False)
        except ValueError as err:
            integrity_failures.append(f"benchmark integrity failed: {err}")
        if bench_rep is not None:
            for row in bench_rep.get("results", []):
                mode = row.get("mode")
                vni = row.get("vs_official_native_image")
                if mode in m7_benchmark.NATIVE_IMAGE_ELIGIBLE_MODES:
                    if not isinstance(vni, dict):
                        unmet_native_image.append(
                            {
                                "id": row["id"],
                                "time_ratio": None,
                                "rss_ratio": None,
                                "verdict": "missing",
                                "reason": "missing required Official Apple Pkl 0.32.0 native-image comparison",
                            }
                        )
                    elif vni["verdict"] != "pass":
                        unmet_native_image.append(
                            {
                                "id": row["id"],
                                "time_ratio": vni["time_ratio"],
                                "rss_ratio": vni["rss_ratio"],
                                "verdict": vni["verdict"],
                                "reason": vni["reason"],
                            }
                        )
                vt = row.get("vs_live_temurin_21")
                if isinstance(vt, dict) and vt["verdict"] != "pass":
                    unmet_perf.append(
                        {
                            "id": row["id"],
                            "time_ratio": vt["time_ratio"],
                            "rss_ratio": vt["rss_ratio"],
                            "verdict": vt["verdict"],
                            "reason": vt["reason"],
                        }
                    )
                vg = row.get("vs_m0_graalvm_25_0_1")
                if vg is not None and vg["verdict"] != "pass":
                    unmet_graalvm.append(
                        {
                            "id": row["id"],
                            "time_ratio": vg["time_ratio"],
                            "rss_ratio": vg["rss_ratio"],
                            "verdict": vg["verdict"],
                            "reason": vg["reason"],
                        }
                    )
            try:
                m7_benchmark.verify_report(benchmark_dir, root=root, strict=True)
                bench_strict_ok = True
            except ValueError as err:
                strict_failures.append(f"benchmark strict gate: {err}")

    # 2. Memory soak verification
    soak_rep: dict[str, Any] | None = None
    soak_strict_ok = False
    leaking_runs: list[str] = []
    if soak_dir is None or not soak_dir.is_dir():
        integrity_failures.append("missing M7 memory soak report directory")
        strict_failures.append("missing M7 memory soak report directory")
    else:
        try:
            soak_rep = m7_memory_soak.verify_report(soak_dir, strict=False)
        except ValueError as err:
            integrity_failures.append(f"memory soak integrity failed: {err}")
        if soak_rep is not None:
            for row in soak_rep.get("results", []):
                if row.get("verdict") != "bounded":
                    leaking_runs.append(f"{row['target']}/{row['scenario']}={row['verdict']}")
            try:
                m7_memory_soak.verify_report(soak_dir, strict=True)
                soak_strict_ok = True
            except ValueError as err:
                strict_failures.append(f"memory soak strict gate: {err}")

    # 3. Differential fuzz verification
    fuzz_rep: dict[str, Any] | None = None
    fuzz_strict_ok = False
    fuzz_mismatches: list[str] = []
    if fuzz_dir is None or not fuzz_dir.is_dir():
        integrity_failures.append("missing M7 differential fuzz report directory")
        strict_failures.append("missing M7 differential fuzz report directory")
    else:
        try:
            fuzz_rep = m7_fuzz.verify_report(fuzz_dir, strict=False)
        except ValueError as err:
            integrity_failures.append(f"differential fuzz integrity failed: {err}")
        if fuzz_rep is not None:
            for row in fuzz_rep.get("results", []):
                if row.get("status") != "pass":
                    fuzz_mismatches.append(f"{row['id']}={row['status']}")
            try:
                m7_fuzz.verify_report(fuzz_dir, strict=True)
                fuzz_strict_ok = True
            except ValueError as err:
                strict_failures.append(f"differential fuzz strict gate: {err}")

    verified_set, observed_non_passing_set = _detect_platforms(
        bench_rep,
        soak_rep,
        fuzz_rep,
        bench_strict_ok=bench_strict_ok,
        soak_strict_ok=soak_strict_ok,
        fuzz_strict_ok=fuzz_strict_ok,
    )

    # 4. Optional multi-browser JS analysis verification (Chromium/Firefox/WebKit)
    js_analysis_summary: dict[str, Any] | None = None
    if js_analysis_dir is not None:
        try:
            js_res = verify_js_analysis_report(
                js_analysis_dir,
                browser_smoke_dir=browser_smoke_dir,
            )
            verified_set.update(js_res["verified_platforms"])
            observed_non_passing_set.update(js_res["observed_non_passing_platforms"])
            js_analysis_summary = {
                "report_sha256": js_res["report_sha256"],
                "profile": js_res["profile"],
                "wasm_binary_gzip_bytes": js_res["wasm_binary_gzip_bytes"],
                "browsers": js_res["browsers"],
            }
            if js_res["observed_non_passing_platforms"]:
                strict_failures.append(
                    "JS multi-browser verification had non-passing browsers: "
                    + ", ".join(sorted(js_res["observed_non_passing_platforms"]))
                )
        except ValueError as err:
            integrity_failures.append(f"JS analysis verification failed: {err}")
            strict_failures.append(f"JS analysis verification failed: {err}")

    # 5. Optional hosted-CI 7-platform release attestation (distinct provenance from local)
    js_raw_rep = (
        load_json(js_analysis_dir / "report.json")
        if js_analysis_dir is not None and (js_analysis_dir / "report.json").is_file()
        else None
    )
    expected_commit, head_mismatch_errors = _extract_bundle_git_head(
        bench_rep, soak_rep, fuzz_rep, js_raw_rep
    )
    if head_mismatch_errors:
        integrity_failures.extend(head_mismatch_errors)
        strict_failures.extend(head_mismatch_errors)

    ci_attestation_summary: dict[str, Any] | None = None
    if ci_attestation_path is not None:
        try:
            ci_res = verify_ci_attestation(
                ci_attestation_path,
                expected_commit=expected_commit,
            )
            verified_set.update(ci_res["verified_platforms"])
            observed_non_passing_set.update(ci_res["observed_non_passing_platforms"])
            ci_attestation_summary = {
                "attestation_sha256": ci_res["attestation_sha256"],
                "provenance": ci_res["provenance"],
                "workflow": ci_res["workflow"],
                "github_repository": ci_res["github_repository"],
                "github_run_id": ci_res["github_run_id"],
                "source_commit": ci_res["source_commit"],
                "checksums_sha256": ci_res["checksums_sha256"],
                "verified_platforms": sorted(ci_res["verified_platforms"]),
            }
            if ci_res["observed_non_passing_platforms"]:
                strict_failures.append(
                    "hosted-CI attestation had non-passing platforms: "
                    + ", ".join(sorted(ci_res["observed_non_passing_platforms"]))
                )
        except ValueError as err:
            integrity_failures.append(f"CI attestation verification failed: {err}")
            strict_failures.append(f"CI attestation verification failed: {err}")

    # Never report a platform as both verified and non-passing; non-passing takes precedence.
    verified_set.difference_update(observed_non_passing_set)
    verified_platforms = sorted(verified_set)
    observed_non_passing_platforms = sorted(observed_non_passing_set)
    all_required_platforms = [
        p["id"] for p in mappings.get("platform_matrix", []) if p.get("required_for_release", True)
    ]
    unverified_platforms = [pid for pid in all_required_platforms if pid not in verified_platforms]

    integrity_passed = len(integrity_failures) == 0
    local_strict_passed = integrity_passed and bench_strict_ok and soak_strict_ok and fuzz_strict_ok
    release_ready = local_strict_passed and len(strict_failures) == 0 and len(unverified_platforms) == 0

    return {
        "schema_version": 1,
        "baseline": manifest["baseline"],
        "local_checkpoint_mode": local_checkpoint,
        "integrity_passed": integrity_passed,
        "integrity_failures": integrity_failures,
        "benchmark_strict_passed": bench_strict_ok,
        "soak_strict_passed": soak_strict_ok,
        "fuzz_strict_passed": fuzz_strict_ok,
        "local_strict_gates_passed": local_strict_passed,
        "release_ready": release_ready,
        "provenance": {
            "local_verification": {
                "benchmark_strict_passed": bench_strict_ok,
                "soak_strict_passed": soak_strict_ok,
                "fuzz_strict_passed": fuzz_strict_ok,
                "git_head": expected_commit,
            },
            "js_multi_browser": js_analysis_summary,
            "hosted_ci_attestation": ci_attestation_summary,
        },
        "pinned_native_image_reference": {
            "id": manifest["pinned_native_image_reference"]["id"],
            "status": manifest["pinned_native_image_reference"]["status"],
            "required_workload_count": manifest["pinned_native_image_reference"]["required_workload_count"],
            "pinned_platforms": manifest["pinned_native_image_reference"]["pinned_platforms"],
        },
        "unmet_performance_workloads_official_native_image": unmet_native_image,
        "unmet_performance_workloads_live_temurin_21": unmet_perf,
        "unmet_performance_workloads_m0_graalvm_25_0_1": unmet_graalvm,
        "non_bounded_soak_runs": leaking_runs,
        "non_pass_fuzz_cases": fuzz_mismatches,
        "verified_platforms": verified_platforms,
        "observed_non_passing_platforms": observed_non_passing_platforms,
        "unverified_platforms": unverified_platforms,
        "excluded_by_scope_2026_11_20": [e["id"] for e in mappings["excluded_by_scope_2026_11_20"]],
        "unpinned_optional_references": [e["id"] for e in mappings["unpinned_optional_references"]],
        "strict_failures": strict_failures,
        "manifest_counts": manifest["counts"],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--write", action="store_true", help="write compatibility/m7-release.json (coverage_gate: null)")
    parser.add_argument("--check", action="store_true", help="verify compatibility/m7-release.json freshness (no runtime pass claim)")
    parser.add_argument("--verify-release", type=Path, default=None, help="verify release bundle directory containing benchmark/, soak/, fuzz/")
    parser.add_argument("--verify-coverage", type=Path, default=None, help="alias for --verify-release")
    parser.add_argument("--benchmark-dir", type=Path, default=None)
    parser.add_argument("--soak-dir", type=Path, default=None)
    parser.add_argument("--fuzz-dir", type=Path, default=None)
    parser.add_argument(
        "--js-analysis-dir",
        type=Path,
        default=None,
        help="verified tools/m7_js_analysis.py multi-browser output directory",
    )
    parser.add_argument(
        "--browser-smoke-dir",
        type=Path,
        default=None,
        help="optional tools/m7_js_analysis.py --smoke-browsers output directory",
    )
    parser.add_argument(
        "--ci-attestation",
        type=Path,
        default=None,
        help="hosted-CI release attestation JSON generated by .github/workflows/release.yml",
    )
    parser.add_argument(
        "--local-checkpoint",
        action="store_true",
        help="gate exit code on local strict gates while STILL reporting unverified platforms and release_ready=false honestly",
    )
    args = parser.parse_args()
    root = args.root.resolve()
    out_path = args.output.resolve() if args.output else (root / OUTPUT_PATH)

    try:
        bundle = args.verify_release or args.verify_coverage
        if bundle or args.benchmark_dir or args.soak_dir or args.fuzz_dir:
            verdict = verify_release(
                root,
                benchmark_dir=args.benchmark_dir,
                soak_dir=args.soak_dir,
                fuzz_dir=args.fuzz_dir,
                js_analysis_dir=args.js_analysis_dir,
                browser_smoke_dir=args.browser_smoke_dir,
                ci_attestation_path=args.ci_attestation,
                bundle_dir=bundle,
                local_checkpoint=args.local_checkpoint,
            )
            print(json.dumps(verdict, ensure_ascii=False, sort_keys=True, indent=2))
            ok = verdict["local_strict_gates_passed"] if args.local_checkpoint else verdict["release_ready"]
            if not ok:
                for line in verdict["strict_failures"]:
                    print(f"M7 release gate: {line}", file=sys.stderr)
                if not args.local_checkpoint and verdict["unverified_platforms"]:
                    print(
                        f"M7 release gate: unverified release platforms: {verdict['unverified_platforms']}",
                        file=sys.stderr,
                    )
                return 1
            return 0

        manifest = build_manifest(root)
        data = encoded(manifest)
        if args.check:
            if not out_path.is_file() or out_path.read_bytes() != data:
                raise ValueError(f"M7 manifest missing or stale: {out_path}; run tools/m7_manifest.py --write")
        elif args.write:
            out_path.parent.mkdir(parents=True, exist_ok=True)
            out_path.write_bytes(data)
        print(json.dumps(manifest["counts"], sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError, RuntimeError) as err:
        print(f"M7 manifest error: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
