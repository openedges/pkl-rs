#!/usr/bin/env python3
"""Mechanical M4 Host I/O, External Readers, Projects, Packages & Pantry manifest and gate.

Reads all M4-relevant categories directly from compatibility/inventory.json
(resource 27, protocol_tag 19, protocol_field 119, cli_command 17, cli_option 86,
and 104 functional M4 stdlib items), compatibility/corpus.json (125 Pantry dev cases
+ 1 shell suite + 69 M2-linked package/project/classpath snippets),
reports/m0/pantry-evidence.tar.gz (7 offline remote deps + 206 packaged modules),
tests/fixtures/m4/cases.json (86 static cases across Lanes 1-3), and
compatibility/m4-mappings.json with zero silent omissions.

Freshness checks (`--check`) NEVER claim runtime compatibility passes (`coverage_gate: null`).
Runtime coverage requires `--verify-coverage <diff-dir>` against a strictly passing
5-lane differential report from tools/m4_differential.py.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import sys

import m4_differential
import m4_pantry_offline as pantry_offline

ROOT = Path(__file__).resolve().parents[1]
INVENTORY_PATH = Path("compatibility/inventory.json")
CORPUS_PATH = Path("compatibility/corpus.json")
MAPPINGS_PATH = Path("compatibility/m4-mappings.json")
M4_CASES_PATH = Path("tests/fixtures/m4/cases.json")
OUTPUT_PATH = Path("compatibility/m4-io-packages.json")
UPSTREAM_CA_ROOTS = Path("external/pkl/pkl-commons-cli/src/main/resources/org/pkl/commons/cli/PklCARoots.pem")
VENDORED_CA_ROOTS = Path("crates/pkl-host-native/assets/PklCARoots.pem")


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


def validate_baseline(root: Path, inventory: dict, corpus: dict, mappings: dict, cases: dict) -> None:
    refs = read_properties((root / "references.lock").read_text(encoding="utf-8"))
    baseline = inventory.get("baseline", {})
    if baseline.get("version") != refs.get("baseline.pkl") or baseline.get("revision") != refs.get("pkl.revision"):
        raise ValueError("inventory.json baseline disagrees with references.lock")
    if corpus.get("baseline") != baseline["version"]:
        raise ValueError("corpus.json baseline disagrees with references.lock")
    expected_base = {"version": baseline["version"], "revision": baseline["revision"]}
    if mappings.get("baseline") != expected_base:
        raise ValueError("m4-mappings.json baseline disagrees with inventory.json")
    if cases.get("baseline") != baseline["version"]:
        raise ValueError("tests/fixtures/m4/cases.json baseline disagrees with inventory.json")


def classify_inventory_items(inventory: dict, mappings: dict) -> dict:
    """Account for 100% of items in M4-tracked inventory categories with zero omissions."""
    items = inventory["items"]
    deferred = mappings["deferred_categories"]

    # 1. resource (27)
    res_items = [i for i in items if i["category"] == "resource"]
    if len(res_items) != 27:
        raise ValueError(f"expected 27 resource items in inventory.json, got {len(res_items)}")
    res_counts = Counter()
    res_ledger = []
    def_prefixes = [(d["prefix"], d["milestone"], d["reason"]) for d in deferred["resource_deferred"]]
    na_prefixes = [(d["prefix"], d["reason"]) for d in deferred["resource_non_applicable"]]
    for item in res_items:
        path = item["source"]["path"]
        status = None
        note = None
        for pref, ms, reason in def_prefixes:
            if path.startswith(pref):
                status, note = f"deferred:{ms}", reason
                break
        if status is None:
            for pref, reason in na_prefixes:
                if path.startswith(pref):
                    status, note = "non-applicable", reason
                    break
        if status is None:
            if item["module"] == "pkl-commons-cli":
                status, note = "m4-required", "Bundled CA roots / CLI project fixture"
            else:
                status, note = "shared-core", "Core release/errorMessages resource shared across M2-M4"
        res_counts[status] += 1
        res_ledger.append({"id": item["id"], "path": path, "disposition": status, "note": note})

    # 2. protocol_tag (19)
    ptag_items = [i for i in items if i["category"] == "protocol_tag"]
    if len(ptag_items) != 19:
        raise ValueError(f"expected 19 protocol_tag items, got {len(ptag_items)}")
    deferred_tags = set(deferred["protocol_tag_deferred"]["symbols"])
    ptag_counts = Counter()
    for item in ptag_items:
        st = f"deferred:{deferred['protocol_tag_deferred']['milestone']}" if item["symbol"] in deferred_tags else "m4-required"
        ptag_counts[st] += 1

    # 3. protocol_field (119)
    pfield_items = [i for i in items if i["category"] == "protocol_field"]
    if len(pfield_items) != 119:
        raise ValueError(f"expected 119 protocol_field items, got {len(pfield_items)}")
    pfield_counts = Counter()
    for item in pfield_items:
        st = (
            f"deferred:{deferred['protocol_field_deferred']['milestone']}"
            if item["module"] == deferred["protocol_field_deferred"]["module"]
            else "m4-required"
        )
        pfield_counts[st] += 1

    # 4. cli_command (17)
    cmd_items = [i for i in items if i["category"] == "cli_command"]
    if len(cmd_items) != 17:
        raise ValueError(f"expected 17 cli_command items, got {len(cmd_items)}")
    m4_cmds = set(mappings["m4_cli_commands"])
    def_cmds = set(deferred["cli_command_deferred"]["symbols"])
    exc_cmds = set(deferred["cli_command_excluded"]["symbols"])
    cmd_counts = Counter()
    for item in cmd_items:
        sym = item["symbol"]
        if sym in m4_cmds:
            cmd_counts["m4-exercised"] += 1
        elif sym in def_cmds:
            cmd_counts["deferred:M5"] += 1
        elif sym in exc_cmds:
            cmd_counts["excluded"] += 1
        else:
            raise ValueError(f"unclassified cli_command in inventory.json: {sym}")

    # 5. cli_option (86)
    opt_items = [i for i in items if i["category"] == "cli_option"]
    if len(opt_items) != 86:
        raise ValueError(f"expected 86 cli_option items, got {len(opt_items)}")
    m4_opts = set(mappings["m4_cli_options"])
    m3_opts = {"-f", "--format", "-m", "--multiple-file-output-path", "-o", "--output-path", "-x", "--expression", "--module-output-separator"}
    opt_counts = Counter()
    for item in opt_items:
        if item["module"] in ("pkl-codegen-java", "pkl-codegen-kotlin"):
            opt_counts["excluded"] += 1
        elif item["symbol"] in m4_opts:
            opt_counts["m4-required"] += 1
        elif item["symbol"] in m3_opts:
            opt_counts["m3-covered"] += 1
        else:
            opt_counts["deferred:M5"] += 1

    # 6. Functional M4 stdlib items (already byte-verified in M3)
    m4_stdlib_paths = set(mappings["m4_functional_stdlib_sources"])
    m4_stdlib_by_file = Counter(
        i["source"]["path"] for i in items if i["category"] == "stdlib" and i["source"]["path"] in m4_stdlib_paths
    )

    return {
        "resource": {"total": len(res_items), "counts": dict(sorted(res_counts.items())), "ledger": res_ledger},
        "protocol_tag": {"total": len(ptag_items), "counts": dict(sorted(ptag_counts.items()))},
        "protocol_field": {"total": len(pfield_items), "counts": dict(sorted(pfield_counts.items()))},
        "cli_command": {"total": len(cmd_items), "counts": dict(sorted(cmd_counts.items()))},
        "cli_option": {"total": len(opt_items), "counts": dict(sorted(opt_counts.items()))},
        "functional_stdlib": {
            "total": sum(m4_stdlib_by_file.values()),
            "by_source": dict(sorted(m4_stdlib_by_file.items())),
        },
    }


def validate_mappings(root: Path, mappings: dict, static_cases: dict[str, dict]) -> dict:
    if mappings.get("schema_version") != 1:
        raise ValueError("invalid compatibility/m4-mappings.json schema_version")
    reqs = mappings.get("requirements", {})
    if not reqs:
        raise ValueError("compatibility/m4-mappings.json requirements map is empty")

    covered_static_ids: set[str] = set()
    validated_reqs: dict[str, dict] = {}
    for rid, row in sorted(reqs.items()):
        lane = row.get("lane")
        if lane not in m4_differential.ALL_LANES:
            raise ValueError(f"invalid lane for requirement {rid}: {lane}")
        cids = row.get("case_ids", [])
        is_dynamic = bool(row.get("dynamic_lane"))
        if not is_dynamic and not cids:
            raise ValueError(f"static requirement {rid} has empty case_ids")
        for cid in cids:
            if cid not in static_cases:
                raise ValueError(f"requirement {rid} references unknown static case_id: {cid}")
            if static_cases[cid]["lane"] != lane:
                raise ValueError(f"requirement {rid} lane {lane} disagrees with case {cid} lane {static_cases[cid]['lane']}")
            covered_static_ids.add(cid)

        resolved_impl = []
        for link in row.get("implementation_links", []):
            rel, sym = link["path"], link["symbol"]
            p = Path(rel)
            if p.is_absolute() or ".." in p.parts or p.parts[:1] != ("crates",) or p.suffix != ".rs":
                raise ValueError(f"invalid implementation link path for {rid}: {rel}")
            full = root / p
            if not full.is_file():
                raise ValueError(f"missing implementation link file for {rid}: {rel}")
            src = full.read_bytes()
            if not re.search(r"\b" + re.escape(sym) + r"\b", src.decode("utf-8")):
                raise ValueError(f"implementation link symbol missing for {rid}: {rel}::{sym}")
            resolved_impl.append({"path": rel, "symbol": sym, "sha256": digest(src)})

        if not resolved_impl:
            raise ValueError(f"empty implementation_links for requirement {rid}")

        validated_reqs[rid] = {
            "lane": lane,
            "description": row.get("description", ""),
            "case_ids": list(cids),
            "dynamic_lane": row.get("dynamic_lane"),
            "expected_dynamic_count": row.get("expected_dynamic_count"),
            "implementation_links": resolved_impl,
        }

    unmapped_static = sorted(set(static_cases) - covered_static_ids)
    if unmapped_static:
        raise ValueError(f"unmapped static M4 cases in tests/fixtures/m4/cases.json: {unmapped_static}")
    return validated_reqs


def verify_differential_coverage(
    root: Path,
    report_dir: Path,
    mappings: dict,
    *,
    strict: bool = True,
) -> dict:
    """Join a sealed M4 differential report against every requirement and lane."""
    report = m4_differential.verify_report(report_dir, strict=strict)
    results_by_id = {r["id"]: r for r in report["results"]}
    results_by_lane: dict[str, list[dict]] = {}
    for r in report["results"]:
        results_by_lane.setdefault(r["lane"], []).append(r)

    failures: list[str] = []
    req_statuses: dict[str, dict] = {}
    accepted_statuses = {"pass"} if strict else {"pass", "oracle-recorded"}

    for lane in m4_differential.ALL_LANES:
        if lane not in report.get("lanes", []):
            failures.append(f"missing required M4 lane in report: {lane}")

    for rid, row in sorted(mappings["requirements"].items()):
        if row.get("dynamic_lane"):
            dlane = row["dynamic_lane"]
            lane_rows = results_by_lane.get(dlane, [])
            exp_n = row.get("expected_dynamic_count")
            bad = [r["id"] for r in lane_rows if r["status"] not in accepted_statuses]
            if exp_n is not None and len(lane_rows) != exp_n:
                failures.append(f"requirement {rid} expected {exp_n} dynamic cases in {dlane}, got {len(lane_rows)}")
            if bad:
                failures.append(f"requirement {rid} has {len(bad)} non-passing case(s) in {dlane}: {bad[:5]}")
            req_statuses[rid] = {
                "lane": dlane,
                "executed": len(lane_rows),
                "accepted": len(lane_rows) - len(bad),
                "passed": (exp_n is None or len(lane_rows) == exp_n) and not bad,
            }
        else:
            missing = [cid for cid in row["case_ids"] if cid not in results_by_id]
            bad = [cid for cid in row["case_ids"] if cid in results_by_id and results_by_id[cid]["status"] not in accepted_statuses]
            if missing:
                failures.append(f"requirement {rid} missing cases in report: {missing[:5]}")
            if bad:
                failures.append(f"requirement {rid} failing cases: {bad[:5]}")
            req_statuses[rid] = {
                "lane": row["lane"],
                "executed": len(row["case_ids"]) - len(missing),
                "accepted": len(row["case_ids"]) - len(missing) - len(bad),
                "passed": not missing and not bad,
            }

    return {
        "schema_version": 1,
        "strict": strict,
        "report_mode": report["mode"],
        "passed": len(failures) == 0,
        "requirement_count": len(req_statuses),
        "requirements_passed": sum(1 for v in req_statuses.values() if v["passed"]),
        "total_executed_cases": report["selected_count"],
        "status_counts": report["status_counts"],
        "lane_status_counts": report["lane_status_counts"],
        "failure_count": len(failures),
        "failures": failures,
        "requirements": req_statuses,
    }


def build_manifest(root: Path = ROOT) -> dict:
    inventory = load_json(root / INVENTORY_PATH)
    corpus = load_json(root / CORPUS_PATH)
    mappings = load_json(root / MAPPINGS_PATH)
    cases_doc = load_json(root / M4_CASES_PATH)

    validate_baseline(root, inventory, corpus, mappings, cases_doc)
    static_cases = m4_differential.validate_static_cases(root, cases_doc)
    validated_reqs = validate_mappings(root, mappings, static_cases)
    inv_census = classify_inventory_items(inventory, mappings)

    # Verify byte-for-byte parity of vendored PklCARoots.pem against pinned upstream resource
    upstream_ca = root / UPSTREAM_CA_ROOTS
    vendored_ca = root / VENDORED_CA_ROOTS
    if not vendored_ca.is_file():
        raise ValueError(f"missing vendored bundled CA roots: {VENDORED_CA_ROOTS}")
    upstream_ca_sha = digest(upstream_ca.read_bytes())
    vendored_ca_sha = digest(vendored_ca.read_bytes())
    if upstream_ca_sha != vendored_ca_sha:
        raise ValueError(
            f"vendored {VENDORED_CA_ROOTS} sha256 ({vendored_ca_sha}) disagrees with upstream {UPSTREAM_CA_ROOTS} ({upstream_ca_sha})"
        )

    # Verify offline Pantry archive & corpus alignment
    remote_pkgs = pantry_offline.load_archive_remote_packages(root)
    pantry_dev = pantry_offline.enumerate_pantry_dev_cases(root)
    pantry_pkg = pantry_offline.enumerate_m0_packaged_module_census(root)

    pantry_dev_total = len(pantry_dev["all_cases"]) + len(pantry_dev["shell_suites"])
    pantry_pkg_total = (
        pantry_pkg["module_count"]
        + len(pantry_pkg["multi_file_uris"])
        + len(pantry_pkg["configured_generators"])
        + len(pantry_pkg["cold_cache_mirror_targets"])
    )

    static_by_lane = dict(sorted(Counter(c["lane"] for c in static_cases.values()).items()))

    return {
        "schema_version": 1,
        "baseline": inventory["baseline"],
        "scope": (
            "Complete M4 Host I/O, URI, modulepath, HTTP/HTTPS/CA/redirect/proxy/headers/rewrites, "
            "security/rootDir, external readers (0x26-0x32), PklProject/settings/resolve/package/cache, "
            "and offline Pantry dev+packaged inventory & case manifest. Freshness checks never claim "
            "runtime compatibility passes (`coverage_gate: null`)."
        ),
        "sources": {
            INVENTORY_PATH.as_posix(): digest((root / INVENTORY_PATH).read_bytes()),
            CORPUS_PATH.as_posix(): digest((root / CORPUS_PATH).read_bytes()),
            MAPPINGS_PATH.as_posix(): digest((root / MAPPINGS_PATH).read_bytes()),
            M4_CASES_PATH.as_posix(): digest((root / M4_CASES_PATH).read_bytes()),
            m4_differential.SERVER_SOURCE.as_posix(): digest((root / m4_differential.SERVER_SOURCE).read_bytes()),
            m4_differential.READER_BIN.as_posix(): digest((root / m4_differential.READER_BIN).read_bytes()),
            VENDORED_CA_ROOTS.as_posix(): vendored_ca_sha,
            pantry_offline.PANTRY_EVIDENCE_ARCHIVE.as_posix(): pantry_offline.sha256_file(
                root / pantry_offline.PANTRY_EVIDENCE_ARCHIVE
            ),
        },
        "counts": {
            "inventory_resource_total": inv_census["resource"]["total"],
            "inventory_resource_dispositions": inv_census["resource"]["counts"],
            "inventory_protocol_tag_total": inv_census["protocol_tag"]["total"],
            "inventory_protocol_tag_dispositions": inv_census["protocol_tag"]["counts"],
            "inventory_protocol_field_total": inv_census["protocol_field"]["total"],
            "inventory_protocol_field_dispositions": inv_census["protocol_field"]["counts"],
            "inventory_cli_command_total": inv_census["cli_command"]["total"],
            "inventory_cli_command_dispositions": inv_census["cli_command"]["counts"],
            "inventory_cli_option_total": inv_census["cli_option"]["total"],
            "inventory_cli_option_dispositions": inv_census["cli_option"]["counts"],
            "functional_stdlib_items": inv_census["functional_stdlib"]["total"],
            "bundled_ca_roots_exact_match": True,
            "m4_requirement_groups": len(validated_reqs),
            "m4_static_cases": len(static_cases),
            "m4_static_cases_by_lane": static_by_lane,
            "pantry_verified_offline_remote_packages": len(remote_pkgs),
            "pantry_dev_cases": pantry_dev_total,
            "pantry_packaged_cases": pantry_pkg_total,
            "m4_total_5lane_cases": len(static_cases) + pantry_dev_total + pantry_pkg_total,
            "pantry_unfixed_external_inputs": len(pantry_dev["external_inputs"]),
        },
        "inventory_accounting": inv_census,
        "pantry_offline_remote_packages": {
            uri: {"metadata_sha256": p["metadata_sha256"], "zip_sha256": p["zip_sha256"]}
            for uri, p in sorted(remote_pkgs.items())
        },
        "pantry_unfixed_external_inputs": pantry_dev["external_inputs"],
        "requirements": validated_reqs,
        "coverage_gate": None,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--write", action="store_true", help="write compatibility/m4-io-packages.json")
    parser.add_argument("--check", action="store_true", help="verify compatibility/m4-io-packages.json freshness (no runtime pass claim)")
    parser.add_argument("--verify-coverage", type=Path, help="verify 5-lane differential report meets 100%% strict M4 coverage")
    parser.add_argument("--verify-oracle-coverage", type=Path, help="verify 5-lane Java-only oracle baseline report covers 100%% of lanes/cases")
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
                "status_counts": verdict["status_counts"],
                "failure_count": verdict["failure_count"],
            }, sort_keys=True))
            if not verdict["passed"]:
                for line in verdict["failures"][:40]:
                    print(f"M4 coverage gap: {line}", file=sys.stderr)
                return 1
            return 0

        manifest = build_manifest(root)
        data = encoded(manifest)
        if args.check:
            if not out_path.is_file() or out_path.read_bytes() != data:
                raise ValueError(f"M4 manifest missing or stale: {out_path}; run tools/m4_manifest.py --write")
        elif args.write:
            out_path.parent.mkdir(parents=True, exist_ok=True)
            out_path.write_bytes(data)
        print(json.dumps(manifest["counts"], sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        print(f"M4 manifest error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
