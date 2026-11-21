#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Mechanical M6 Wasm & N-API Product Completion manifest, capability accounting, and coverage gate.

Accounts cleanly for all M6 requirement groups across the 5 offline lanes:
  - wasm-browser
  - wasm-node
  - napi-node
  - cross-target-parity
  - packaging-and-install

Honesty invariants:
  - Freshness checks (`--check`) verify manifest/mapping/fixture/runner freshness ONLY
    (`coverage_gate: null`, NEVER claims runtime passes).
  - `--verify-oracle-coverage <dir>` validates Java-only oracle baseline coverage (`strict: false`).
  - `--verify-coverage <dir>` enforces `strict: true`:
      * `m6_differential.verify_report(..., strict=True)` (exact JDK lock, recorded-build, source_stability)
      * All 5 lanes executed
      * 100% of required executable cases `pass`
      * Explicit `capability-blocked` boundary cases verified separately and NEVER counted as compatibility passes
      * All requirement groups `implementation_status == "linked"` with verified Rust (`crates/**`)
        and JS/TS (`packages/**`) symbol links.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import sys

import m6_differential

ROOT = Path(__file__).resolve().parents[1]
INVENTORY_PATH = Path("compatibility/inventory.json")
MAPPINGS_PATH = Path("compatibility/m6-mappings.json")
M6_CASES_PATH = Path("tests/fixtures/m6/cases.json")
OUTPUT_PATH = Path("compatibility/m6-wasm-napi.json")

ALLOWED_RUST_SUFFIXES = {".rs"}
ALLOWED_PKG_SUFFIXES = {".mjs", ".cjs", ".js", ".ts", ".d.ts", ".json"}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encoded(value: object) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode("utf-8")


def read_properties(text: str) -> dict[str, str]:
    result = {}
    for line in text.splitlines():
        if not line or line.startswith("#"):
            continue
        k, v = line.split("=", 1)
        result[k] = v
    return result


def load_json(path: Path) -> dict:
    return json.loads(path.read_bytes())


def validate_baseline(root: Path, inventory: dict, mappings: dict, cases: dict) -> None:
    refs = read_properties((root / "references.lock").read_text(encoding="utf-8"))
    baseline = inventory.get("baseline", {})
    if baseline.get("version") != refs.get("baseline.pkl") or baseline.get("revision") != refs.get("pkl.revision"):
        raise ValueError("inventory.json baseline disagrees with references.lock")
    expected_base = {"version": baseline["version"], "revision": baseline["revision"]}
    if mappings.get("baseline") != expected_base:
        raise ValueError("m6-mappings.json baseline disagrees with inventory.json")
    if cases.get("baseline") != baseline["version"]:
        raise ValueError("tests/fixtures/m6/cases.json baseline disagrees with inventory.json")


def _valid_impl_link_path(p: Path) -> bool:
    if p.is_absolute() or ".." in p.parts:
        return False
    if p.parts[:1] == ("crates",):
        return p.suffix in ALLOWED_RUST_SUFFIXES
    if p.parts[:2] in (("packages", "pkl-wasm"), ("packages", "pkl-napi")):
        return p.suffix in ALLOWED_PKG_SUFFIXES or p.name.endswith(".d.ts")
    return False


def validate_mappings(root: Path, mappings: dict, cases_by_id: dict[str, dict]) -> dict:
    if mappings.get("schema_version") != 1:
        raise ValueError("invalid compatibility/m6-mappings.json schema_version")
    reqs = mappings.get("requirements", {})
    if not reqs:
        raise ValueError("compatibility/m6-mappings.json requirements map is empty")

    covered_ids: set[str] = set()
    validated_reqs: dict[str, dict] = {}
    for rid, row in sorted(reqs.items()):
        lane = row.get("lane")
        if lane not in m6_differential.ALL_LANES:
            raise ValueError(f"invalid lane for requirement {rid}: {lane}")
        cids = row.get("case_ids", [])
        if not cids:
            raise ValueError(f"requirement {rid} has empty case_ids")
        for cid in cids:
            if cid not in cases_by_id:
                raise ValueError(f"requirement {rid} references unknown case_id: {cid}")
            if cases_by_id[cid]["lane"] != lane:
                raise ValueError(f"requirement {rid} lane {lane} disagrees with case {cid} lane {cases_by_id[cid]['lane']}")
            covered_ids.add(cid)

        impl_status = row.get("implementation_status", "linked")
        if impl_status not in ("linked", "pending-wave2"):
            raise ValueError(f"invalid implementation_status for {rid}: {impl_status}")

        resolved_impl = []
        for link in row.get("implementation_links", []):
            rel, sym = link["path"], link["symbol"]
            p = Path(rel)
            if not _valid_impl_link_path(p):
                raise ValueError(f"invalid implementation link path for {rid}: {rel}")
            full = root / p
            if not full.is_file():
                raise ValueError(f"missing implementation link file for {rid}: {rel}")
            src = full.read_bytes()
            text = src.decode("utf-8")
            if not re.search(r"(?<![A-Za-z0-9_])" + re.escape(sym) + r"(?![A-Za-z0-9_])", text):
                raise ValueError(f"implementation link symbol missing for {rid}: {rel}::{sym}")
            resolved_impl.append({"path": rel, "symbol": sym, "sha256": digest(src)})

        if impl_status == "linked" and not resolved_impl:
            raise ValueError(f"requirement {rid} marked linked but has empty implementation_links")

        exec_cids = [cid for cid in cids if not cases_by_id[cid].get("capability_boundary")]
        cap_cids = [cid for cid in cids if cases_by_id[cid].get("capability_boundary")]

        validated_reqs[rid] = {
            "lane": lane,
            "description": row.get("description", ""),
            "case_ids": list(cids),
            "executable_case_ids": exec_cids,
            "capability_boundary_case_ids": cap_cids,
            "implementation_status": impl_status,
            "implementation_links": resolved_impl,
        }

    unmapped = sorted(set(cases_by_id) - covered_ids)
    if unmapped:
        raise ValueError(f"unmapped M6 cases in tests/fixtures/m6/cases.json: {unmapped}")
    return validated_reqs


def verify_differential_coverage(
    root: Path,
    report_dir: Path,
    mappings: dict,
    *,
    strict: bool = True,
) -> dict:
    report = m6_differential.verify_report(report_dir, strict=strict)
    live_cases_sha = digest((root / M6_CASES_PATH).read_bytes())
    if report.get("cases_manifest_sha256") != live_cases_sha:
        raise ValueError(
            "report cases_manifest_sha256 is stale vs live tests/fixtures/m6/cases.json "
            f"({report.get('cases_manifest_sha256')} != {live_cases_sha})"
        )
    results_by_id = {r["id"]: r for r in report["results"]}
    cases_doc = load_json(root / M6_CASES_PATH)
    cases_by_id = m6_differential.validate_cases(root, cases_doc)
    validate_mappings(root, mappings, cases_by_id)

    failures: list[str] = []
    req_statuses: dict[str, dict] = {}

    for lane in m6_differential.ALL_LANES:
        if lane not in report.get("lanes", []):
            failures.append(f"missing required M6 lane in report: {lane}")

    for rid, row in sorted(mappings["requirements"].items()):
        impl_status = row.get("implementation_status", "linked")
        if strict and impl_status != "linked":
            failures.append(f"requirement {rid} implementation_status is {impl_status!r} (expected 'linked')")
        cids = row["case_ids"]
        missing = [cid for cid in cids if cid not in results_by_id]
        exec_bad: list[str] = []
        cap_bad: list[str] = []
        exec_passed = 0
        cap_blocked_verified = 0

        for cid in cids:
            if cid not in results_by_id:
                continue
            r = results_by_id[cid]
            is_cap = bool(cases_by_id[cid].get("capability_boundary", False))
            if is_cap:
                if strict:
                    if r["status"] == "capability-blocked":
                        cap_blocked_verified += 1
                    else:
                        cap_bad.append(f"{cid}={r['status']}")
                else:
                    if r["status"] in ("capability-blocked", "oracle-recorded"):
                        cap_blocked_verified += 1
                    else:
                        cap_bad.append(f"{cid}={r['status']}")
            else:
                if strict:
                    if r["status"] == "pass":
                        exec_passed += 1
                    else:
                        exec_bad.append(f"{cid}={r['status']}")
                else:
                    if r["status"] in ("pass", "oracle-recorded"):
                        exec_passed += 1
                    else:
                        exec_bad.append(f"{cid}={r['status']}")

        if missing:
            failures.append(f"requirement {rid} missing cases in report: {missing[:5]}")
        if exec_bad:
            failures.append(f"requirement {rid} failing executable cases: {exec_bad[:5]}")
        if cap_bad:
            failures.append(f"requirement {rid} unverified capability-boundary cases: {cap_bad[:5]}")

        exec_total = sum(1 for cid in cids if not cases_by_id[cid].get("capability_boundary"))
        cap_total = sum(1 for cid in cids if cases_by_id[cid].get("capability_boundary"))
        req_ok = not missing and not exec_bad and not cap_bad and (not strict or impl_status == "linked")
        req_statuses[rid] = {
            "lane": row["lane"],
            "implementation_status": impl_status,
            "executable_total": exec_total,
            "executable_passed": exec_passed,
            "capability_boundary_total": cap_total,
            "capability_blocked_verified": cap_blocked_verified,
            "passed": req_ok,
        }

    return {
        "schema_version": 1,
        "strict": strict,
        "report_mode": report["mode"],
        "passed": len(failures) == 0,
        "requirement_count": len(req_statuses),
        "requirements_passed": sum(1 for v in req_statuses.values() if v["passed"]),
        "total_executed_cases": report["selected_count"],
        "executable_case_count": report["executable_case_count"],
        "capability_boundary_case_count": report["capability_boundary_case_count"],
        "compatibility_pass_count": report["compatibility_pass_count"],
        "capability_blocked_count": report["capability_blocked_count"],
        "status_counts": report["status_counts"],
        "lane_status_counts": report["lane_status_counts"],
        "cross_target_summary": report.get("cross_target_summary"),
        "failure_count": len(failures),
        "failures": failures,
        "requirements": req_statuses,
    }


def build_manifest(root: Path = ROOT) -> dict:
    inventory = load_json(root / INVENTORY_PATH)
    mappings = load_json(root / MAPPINGS_PATH)
    cases_doc = load_json(root / M6_CASES_PATH)

    validate_baseline(root, inventory, mappings, cases_doc)
    cases_by_id = m6_differential.validate_cases(root, cases_doc)
    validated_reqs = validate_mappings(root, mappings, cases_by_id)

    by_lane_total = dict(sorted(Counter(c["lane"] for c in cases_by_id.values()).items()))
    by_lane_exec = dict(sorted(Counter(c["lane"] for c in cases_by_id.values() if not c.get("capability_boundary")).items()))
    by_lane_cap = dict(sorted(Counter(c["lane"] for c in cases_by_id.values() if c.get("capability_boundary")).items()))
    req_impl_counts = dict(sorted(Counter(r["implementation_status"] for r in validated_reqs.values()).items()))

    sources = {
        INVENTORY_PATH.as_posix(): digest((root / INVENTORY_PATH).read_bytes()),
        MAPPINGS_PATH.as_posix(): digest((root / MAPPINGS_PATH).read_bytes()),
        M6_CASES_PATH.as_posix(): digest((root / M6_CASES_PATH).read_bytes()),
        m6_differential.ORACLE_PROBE_JAVA.as_posix(): digest((root / m6_differential.ORACLE_PROBE_JAVA).read_bytes()),
        m6_differential.PLAYWRIGHT_PACKAGE_JSON.as_posix(): digest((root / m6_differential.PLAYWRIGHT_PACKAGE_JSON).read_bytes()),
    }
    for runner in sorted((root / m6_differential.RUNNERS_DIR).glob("*.mjs")):
        sources[runner.relative_to(root).as_posix()] = digest(runner.read_bytes())

    exec_count = sum(1 for c in cases_by_id.values() if not c.get("capability_boundary"))
    cap_count = sum(1 for c in cases_by_id.values() if c.get("capability_boundary"))

    return {
        "schema_version": 1,
        "baseline": inventory["baseline"],
        "scope": (
            "Complete M6 Wasm & N-API Product Completion manifest: unified `@pkl-rs/wasm` (browser + Node, "
            "VFS, async loaders, pure-Wasm package archives) and `@pkl-rs/napi` (dual ESM+CJS, native FS/HTTP/"
            "HTTPS/CA/proxy/projects/packages/external readers, non-blocking async + cancel/timeout/close), "
            "lossless BigInt/Uint8Array/UTF-16 & 64-bit clock, 4-way cross-target raw-byte parity "
            "(Java == Native == N-API == Wasm), clean-install packaging verification, and explicit "
            "capability-gap honesty (`capability-blocked` separated and never counted as compatibility passes). "
            "Freshness checks never claim runtime compatibility passes (`coverage_gate: null`)."
        ),
        "api_contract": mappings["api_contract"],
        "capability_matrix": mappings["capability_matrix"],
        "sources": dict(sorted(sources.items())),
        "counts": {
            "m6_requirement_groups": len(validated_reqs),
            "m6_requirement_impl_status": req_impl_counts,
            "m6_total_cases": len(cases_by_id),
            "m6_executable_cases": exec_count,
            "m6_capability_boundary_cases": cap_count,
            "m6_cases_by_lane": by_lane_total,
            "m6_executable_cases_by_lane": by_lane_exec,
            "m6_capability_boundary_cases_by_lane": by_lane_cap,
            "m6_common_js_exports": len(mappings["api_contract"]["common_exports"]),
            "m6_capability_keys": len(mappings["api_contract"]["capability_keys"]),
        },
        "requirements": validated_reqs,
        "coverage_gate": None,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--write", action="store_true", help="write compatibility/m6-wasm-napi.json")
    parser.add_argument("--check", action="store_true", help="verify compatibility/m6-wasm-napi.json freshness (no runtime pass claim)")
    parser.add_argument("--verify-coverage", type=Path, help="verify 5-lane differential report meets 100%% strict M6 coverage")
    parser.add_argument("--verify-oracle-coverage", type=Path, help="verify 5-lane Java-only oracle baseline covers 100%% of lanes/cases")
    args = parser.parse_args()

    root = args.root.resolve()
    out_path = args.output.resolve() if args.output else (root / OUTPUT_PATH)

    try:
        if args.verify_coverage or args.verify_oracle_coverage:
            target = (args.verify_coverage or args.verify_oracle_coverage).resolve()
            strict = bool(args.verify_coverage)
            mappings = load_json(root / MAPPINGS_PATH)
            verdict = verify_differential_coverage(root, target, mappings, strict=strict)
            print(json.dumps({
                "strict": verdict["strict"],
                "passed": verdict["passed"],
                "requirement_count": verdict["requirement_count"],
                "requirements_passed": verdict["requirements_passed"],
                "total_executed_cases": verdict["total_executed_cases"],
                "executable_case_count": verdict["executable_case_count"],
                "capability_boundary_case_count": verdict["capability_boundary_case_count"],
                "compatibility_pass_count": verdict["compatibility_pass_count"],
                "capability_blocked_count": verdict["capability_blocked_count"],
                "status_counts": verdict["status_counts"],
                "failure_count": verdict["failure_count"],
            }, sort_keys=True))
            if not verdict["passed"]:
                for line in verdict["failures"][:40]:
                    print(f"M6 coverage gap: {line}", file=sys.stderr)
                return 1
            return 0

        manifest = build_manifest(root)
        data = encoded(manifest)
        if args.check:
            if not out_path.is_file() or out_path.read_bytes() != data:
                raise ValueError(f"M6 manifest missing or stale: {out_path}; run tools/m6_manifest.py --write")
        elif args.write:
            out_path.parent.mkdir(parents=True, exist_ok=True)
            out_path.write_bytes(data)
        print(json.dumps(manifest["counts"], sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        print(f"M6 manifest error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
