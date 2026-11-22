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

    track_counts = dict(sorted(Counter(r["track"] for r in validated_reqs.values()).items()))
    impl_counts = dict(sorted(Counter(r["implementation_status"] for r in validated_reqs.values()).items()))
    platform_matrix = mappings.get("platform_matrix", [])

    return {
        "schema_version": 1,
        "baseline": inventory["baseline"],
        "scope": mappings["scope"],
        "target_ratios": bench_contract["target_ratios"],
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
        if "machine_start" in rep:
            sys_name = rep["machine_start"].get("system")
            arch = rep["machine_start"].get("machine")
        elif "host" in rep:
            sys_name = rep["host"].get("system")
            arch = rep["host"].get("machine")
        if sys_name == "Darwin" and arch == "arm64":
            observed_hosts.add("darwin-arm64")
        elif sys_name == "Darwin" and arch == "x86_64":
            observed_hosts.add("darwin-x64")
        elif sys_name == "Linux" and arch in ("x86_64", "amd64"):
            observed_hosts.add("linux-x64-gnu")
        elif sys_name == "Linux" and arch in ("aarch64", "arm64"):
            observed_hosts.add("linux-arm64-gnu")
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


def verify_release(
    root: Path,
    *,
    benchmark_dir: Path | None = None,
    soak_dir: Path | None = None,
    fuzz_dir: Path | None = None,
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

    integrity_failures: list[str] = []
    strict_failures: list[str] = []

    # 1. Benchmark verification
    bench_rep: dict[str, Any] | None = None
    bench_strict_ok = False
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
                vt = row["vs_live_temurin_21"]
                if vt["verdict"] != "pass":
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
    verified_platforms = sorted(verified_set)
    observed_non_passing_platforms = sorted(observed_non_passing_set)
    all_required_platforms = [
        p["id"] for p in mappings.get("platform_matrix", []) if p.get("required_for_release", True)
    ]
    unverified_platforms = [pid for pid in all_required_platforms if pid not in verified_platforms]

    integrity_passed = len(integrity_failures) == 0
    local_strict_passed = integrity_passed and bench_strict_ok and soak_strict_ok and fuzz_strict_ok
    release_ready = local_strict_passed and len(unverified_platforms) == 0

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
