#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Mechanical M5 Product APIs & Tools manifest, inventory accounting, and coverage gate.

Accounts with zero silent omissions for every M5-scoped category in
`compatibility/inventory.json` and `compatibility/corpus.json`:
  - `cli_command` (17: 8 M4+M5, 7 M5-new, 2 excluded codegen)
  - `cli_argument` (6: all 6 M5-covered)
  - `cli_option` (86, ID-and-module-aware: 12 M3, 29 M4, 26 M5-new, 19 excluded codegen)
  - `protocol_tag` (19: 13 M4/callbacks 0x26-0x32, 6 M5 0x20-0x25)
  - `protocol_field` (119: 37 M4 pkl-core, 82 M5 pkl-server)
  - `resource` (27: 17 M5 pkl-doc static web assets, 3 M4, 3 shared-core, 4 non-applicable)
  - `stdlib/doc-package-info.pkl` (8 items deferred from M3 to M5 pkldoc)
  - `formatter-snippet` (30 golden cases in `compatibility/corpus.json`)
  - Explicit 2026-11-20 JVM/codegen exclusions census (`java_api` 8,239, `kotlin_api` 597,
    codegen CLI 2+19, JVM SPI resources 2, JVM binding specs 73, JVM test suites 58)

Freshness checks (`--check`) NEVER claim runtime compatibility passes (`coverage_gate: null`).
Strict coverage (`--verify-coverage <diff-dir>`) requires:
  - `m5_differential.verify_report(..., strict=True)` (100% pass, recorded-build, exact JDK lock,
    source_stability == true)
  - All 5 lanes executed and 100% of requirement groups passing
  - All requirements have `implementation_status == "linked"` with verified Rust symbol links
  - All 17 vendored `pkl-doc` static web assets match upstream SHA-256 byte-for-byte
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import sys

import m5_differential
import m5_pkldoc_tree

ROOT = Path(__file__).resolve().parents[1]
INVENTORY_PATH = Path("compatibility/inventory.json")
CORPUS_PATH = Path("compatibility/corpus.json")
MAPPINGS_PATH = Path("compatibility/m5-mappings.json")
M5_CASES_PATH = Path("tests/fixtures/m5/cases.json")
OUTPUT_PATH = Path("compatibility/m5-product-tools.json")
UPSTREAM_PKLDOC_RES_PREFIX = "pkl-doc/src/main/resources/org/pkl/doc/"
EXCLUDED_JVM_TEST_MODULES = (
    "pkl-codegen-java",
    "pkl-codegen-kotlin",
    "pkl-config-java",
    "pkl-config-kotlin",
    "pkl-executor",
    "pkl-gradle",
)


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
        raise ValueError("m5-mappings.json baseline disagrees with inventory.json")
    if cases.get("baseline") != baseline["version"]:
        raise ValueError("tests/fixtures/m5/cases.json baseline disagrees with inventory.json")


def classify_inventory_items(root: Path, inventory: dict, corpus: dict, mappings: dict) -> dict:
    """Account for 100% of M5-scoped categories and explicitly excluded JVM/codegen census items."""
    items = inventory["items"]
    excl = mappings["excluded_2026_11_20"]

    # 1. cli_command (17)
    cmd_items = [i for i in items if i["category"] == "cli_command"]
    if len(cmd_items) != 17:
        raise ValueError(f"expected 17 cli_command items, got {len(cmd_items)}")
    m4_cmds = set(mappings["cli_commands_m4_and_m5"])
    m5_cmds = set(mappings["cli_commands_m5_new"])
    exc_cmds = set(excl["cli_commands"])
    cmd_counts = Counter()
    for i in cmd_items:
        sym = i["symbol"]
        if sym in m4_cmds:
            cmd_counts["m4-exercised-and-m5-product"] += 1
        elif sym in m5_cmds:
            cmd_counts["m5-new"] += 1
        elif sym in exc_cmds:
            cmd_counts["excluded"] += 1
        else:
            raise ValueError(f"unclassified cli_command: {sym}")

    # 2. cli_argument (6)
    arg_items = [i for i in items if i["category"] == "cli_argument"]
    if len(arg_items) != 6:
        raise ValueError(f"expected 6 cli_argument items, got {len(arg_items)}")
    arg_by_mod = dict(sorted(Counter(i["module"] for i in arg_items).items()))

    # 3. cli_option (86, strictly ID-and-module-aware!)
    opt_items = [i for i in items if i["category"] == "cli_option"]
    if len(opt_items) != 86:
        raise ValueError(f"expected 86 cli_option items, got {len(opt_items)}")
    m3_opt_syms = set(mappings["cli_options_m3_symbols"])
    m4_opt_syms = set(mappings["cli_options_m4_symbols"])
    exc_opt_mods = set(excl["cli_option_modules"])
    opt_counts = Counter()
    opt_m5_sub = Counter()
    for i in opt_items:
        mod = i["module"]
        path = i["source"]["path"]
        sym = i["symbol"]
        if mod in exc_opt_mods:
            opt_counts["excluded"] += 1
        elif mod == "pkl-doc":
            opt_counts["m5-new"] += 1
            opt_m5_sub["m5:pkldoc"] += 1
        elif path.endswith("FormatterCommand.kt"):
            opt_counts["m5-new"] += 1
            opt_m5_sub["m5:format"] += 1
        elif sym in m4_opt_syms:
            opt_counts["m4-exercised"] += 1
        elif sym in m3_opt_syms:
            opt_counts["m3-covered"] += 1
        else:
            opt_counts["m5-new"] += 1
            opt_m5_sub["m5:cli-other"] += 1

    # 4. protocol_tag (19)
    ptag_items = [i for i in items if i["category"] == "protocol_tag"]
    if len(ptag_items) != 19:
        raise ValueError(f"expected 19 protocol_tag items, got {len(ptag_items)}")
    m5_tags = set(mappings["protocol_tags_m5"])
    ptag_counts = Counter()
    for i in ptag_items:
        ptag_counts["m5-required" if i["symbol"] in m5_tags else "m4-and-m5-callbacks"] += 1

    # 5. protocol_field (119)
    pfield_items = [i for i in items if i["category"] == "protocol_field"]
    if len(pfield_items) != 119:
        raise ValueError(f"expected 119 protocol_field items, got {len(pfield_items)}")
    pfield_counts = Counter("m5-required" if i["module"] == "pkl-server" else "m4-required" for i in pfield_items)

    # 6. resource (27, incl. 17 pkl-doc web assets)
    res_items = [i for i in items if i["category"] == "resource"]
    if len(res_items) != 27:
        raise ValueError(f"expected 27 resource items, got {len(res_items)}")
    res_counts = Counter()
    pkldoc_assets: list[dict] = []
    vendored_dir_rel = mappings.get("pkldoc_vendored_assets_dir")
    vendored_dir = (root / vendored_dir_rel) if vendored_dir_rel else None
    vendored_matches = 0
    for i in sorted(res_items, key=lambda x: x["source"]["path"]):
        path = i["source"]["path"]
        if path.startswith(UPSTREAM_PKLDOC_RES_PREFIX):
            res_counts["m5-pkldoc"] += 1
            rel_asset = path[len(UPSTREAM_PKLDOC_RES_PREFIX):]
            up_bytes = (root / "external/pkl" / path).read_bytes()
            up_sha = digest(up_bytes)
            vend_state = "pending-track-d"
            vend_sha = None
            if vendored_dir is not None:
                vpath = vendored_dir / rel_asset
                if vpath.is_file():
                    vend_sha = digest(vpath.read_bytes())
                    if vend_sha == up_sha:
                        vend_state = "exact-match"
                        vendored_matches += 1
                    else:
                        vend_state = "sha256-mismatch"
                else:
                    vend_state = "missing"
            pkldoc_assets.append({
                "id": i["id"],
                "upstream_path": path,
                "relative_asset": rel_asset,
                "bytes": len(up_bytes),
                "upstream_sha256": up_sha,
                "vendored_sha256": vend_sha,
                "vendored_state": vend_state,
            })
        elif path.startswith("build-logic/src/main/resources/"):
            res_counts["non-applicable-build"] += 1
        elif path.startswith("pkl-core/src/main/resources/META-INF/services/"):
            res_counts["excluded-jvm-spi"] += 1
        elif i["module"] == "pkl-commons-cli":
            res_counts["m4-required"] += 1
        else:
            res_counts["shared-core"] += 1

    if len(pkldoc_assets) != 17:
        raise ValueError(f"expected 17 pkl-doc resources, got {len(pkldoc_assets)}")
    if vendored_dir is None:
        assets_status = "pending-track-d"
    elif vendored_matches == 17:
        assets_status = "exact-match"
    else:
        assets_status = "incomplete-or-mismatch"

    # 7. stdlib/doc-package-info.pkl (8 deferred from M3)
    dpi_items = [i for i in items if i["category"] == "stdlib" and i["source"]["path"] == "stdlib/doc-package-info.pkl"]
    if len(dpi_items) != 8:
        raise ValueError(f"expected 8 stdlib/doc-package-info.pkl items, got {len(dpi_items)}")

    # 8. Functional M5 stdlib modules
    m5_stdlib_paths = set(mappings["m5_functional_stdlib_sources"])
    m5_stdlib_by_file = dict(sorted(Counter(
        i["source"]["path"] for i in items if i["category"] == "stdlib" and i["source"]["path"] in m5_stdlib_paths
    ).items()))

    # 9. Specification headings
    spec_by_mod = Counter(i["source"]["path"].split("/")[2] for i in items if i["category"] == "specification")
    m5_spec_mods = ("pkl-cli", "bindings-specification", "pkl-doc")
    excl_spec_mods = ("java-binding", "kotlin-binding")

    # 10. Explicit 2026-11-20 JVM/codegen exclusions census (inventory-only; NEVER counted as passes!)
    java_api_items = [i for i in items if i["category"] == "java_api"]
    kotlin_api_items = [i for i in items if i["category"] == "kotlin_api"]
    excl_jvm_suites = sum(
        1 for s in corpus["suites"]
        if s["source"]["path"].split("/", 1)[0] in EXCLUDED_JVM_TEST_MODULES
    )

    return {
        "cli_command": {"total": len(cmd_items), "counts": dict(sorted(cmd_counts.items()))},
        "cli_argument": {"total": len(arg_items), "by_module": arg_by_mod},
        "cli_option": {
            "total": len(opt_items),
            "counts": dict(sorted(opt_counts.items())),
            "m5_new_breakdown": dict(sorted(opt_m5_sub.items())),
        },
        "protocol_tag": {"total": len(ptag_items), "counts": dict(sorted(ptag_counts.items()))},
        "protocol_field": {"total": len(pfield_items), "counts": dict(sorted(pfield_counts.items()))},
        "resource": {
            "total": len(res_items),
            "counts": dict(sorted(res_counts.items())),
            "pkldoc_assets_status": assets_status,
            "pkldoc_assets_exact_matches": vendored_matches,
            "pkldoc_assets": pkldoc_assets,
        },
        "stdlib_doc_package_info": {
            "total": len(dpi_items),
            "symbols": sorted(i["symbol"] for i in dpi_items),
        },
        "functional_stdlib": {
            "total": sum(m5_stdlib_by_file.values()),
            "by_source": m5_stdlib_by_file,
        },
        "specification": {
            "total": sum(spec_by_mod.values()),
            "m5_relevant": {m: spec_by_mod[m] for m in m5_spec_mods},
            "m5_relevant_total": sum(spec_by_mod[m] for m in m5_spec_mods),
            "excluded_jvm_bindings": {m: spec_by_mod[m] for m in excl_spec_mods},
            "excluded_jvm_bindings_total": sum(spec_by_mod[m] for m in excl_spec_mods),
        },
        "excluded_2026_11_20_census": {
            "policy": excl["policy"],
            "java_api_total": len(java_api_items),
            "java_api_by_scope": dict(sorted(Counter(i.get("api_scope") for i in java_api_items).items())),
            "kotlin_api_total": len(kotlin_api_items),
            "kotlin_api_by_scope": dict(sorted(Counter(i.get("api_scope") for i in kotlin_api_items).items())),
            "codegen_cli_commands": cmd_counts["excluded"],
            "codegen_cli_options": opt_counts["excluded"],
            "jvm_spi_resources": res_counts["excluded-jvm-spi"],
            "jvm_binding_spec_headings": sum(spec_by_mod[m] for m in excl_spec_mods),
            "excluded_corpus_jvm_test_files": excl_jvm_suites,
        },
    }


def validate_mappings(root: Path, mappings: dict, static_cases: dict[str, dict]) -> dict:
    if mappings.get("schema_version") != 1:
        raise ValueError("invalid compatibility/m5-mappings.json schema_version")
    reqs = mappings.get("requirements", {})
    if not reqs:
        raise ValueError("compatibility/m5-mappings.json requirements map is empty")

    covered_static_ids: set[str] = set()
    validated_reqs: dict[str, dict] = {}
    for rid, row in sorted(reqs.items()):
        lane = row.get("lane")
        if lane not in m5_differential.ALL_LANES:
            raise ValueError(f"invalid lane for requirement {rid}: {lane}")
        cids = row.get("case_ids", [])
        is_dynamic = bool(row.get("dynamic_mode"))
        if not is_dynamic and not cids:
            raise ValueError(f"static requirement {rid} has empty case_ids")
        for cid in cids:
            if cid not in static_cases:
                raise ValueError(f"requirement {rid} references unknown static case_id: {cid}")
            if static_cases[cid]["lane"] != lane:
                raise ValueError(f"requirement {rid} lane {lane} disagrees with case {cid} lane {static_cases[cid]['lane']}")
            covered_static_ids.add(cid)

        impl_status = row.get("implementation_status", "linked")
        if impl_status not in ("linked", "pending-wave2"):
            raise ValueError(f"invalid implementation_status for {rid}: {impl_status}")

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

        if impl_status == "linked" and not resolved_impl:
            raise ValueError(f"requirement {rid} marked linked but has empty implementation_links")

        validated_reqs[rid] = {
            "lane": lane,
            "description": row.get("description", ""),
            "case_ids": list(cids),
            "dynamic_mode": row.get("dynamic_mode"),
            "expected_dynamic_count": row.get("expected_dynamic_count"),
            "implementation_status": impl_status,
            "implementation_links": resolved_impl,
        }

    unmapped_static = sorted(set(static_cases) - covered_static_ids)
    if unmapped_static:
        raise ValueError(f"unmapped static M5 cases in tests/fixtures/m5/cases.json: {unmapped_static}")
    return validated_reqs


def verify_differential_coverage(
    root: Path,
    report_dir: Path,
    mappings: dict,
    *,
    strict: bool = True,
) -> dict:
    report = m5_differential.verify_report(report_dir, strict=strict)
    results_by_id = {r["id"]: r for r in report["results"]}
    results_by_mode: dict[str, list[dict]] = {}
    for r in report["results"]:
        results_by_mode.setdefault(r["exec_mode"], []).append(r)

    failures: list[str] = []
    req_statuses: dict[str, dict] = {}
    accepted_statuses = {"pass"} if strict else {"pass", "oracle-recorded"}

    for lane in m5_differential.ALL_LANES:
        if lane not in report.get("lanes", []):
            failures.append(f"missing required M5 lane in report: {lane}")

    if strict:
        # Strict M5 exit gate requires vendored pkldoc static asset byte parity!
        inventory = load_json(root / INVENTORY_PATH)
        corpus = load_json(root / CORPUS_PATH)
        inv_census = classify_inventory_items(root, inventory, corpus, mappings)
        if inv_census["resource"]["pkldoc_assets_status"] != "exact-match":
            failures.append(
                f"pkldoc vendored static assets parity unmet: {inv_census['resource']['pkldoc_assets_status']} "
                f"({inv_census['resource']['pkldoc_assets_exact_matches']}/17)"
            )

    for rid, row in sorted(mappings["requirements"].items()):
        impl_status = row.get("implementation_status", "linked")
        if strict and impl_status != "linked":
            failures.append(f"requirement {rid} implementation_status is {impl_status!r} (expected 'linked')")
        if row.get("dynamic_mode"):
            dmode = row["dynamic_mode"]
            mode_rows = results_by_mode.get(dmode, [])
            exp_n = row.get("expected_dynamic_count")
            bad = [r["id"] for r in mode_rows if r["status"] not in accepted_statuses]
            if exp_n is not None and len(mode_rows) != exp_n:
                failures.append(f"requirement {rid} expected {exp_n} dynamic cases for {dmode}, got {len(mode_rows)}")
            if bad:
                failures.append(f"requirement {rid} has {len(bad)} non-passing case(s) for {dmode}: {bad[:5]}")
            req_statuses[rid] = {
                "lane": row["lane"],
                "implementation_status": impl_status,
                "executed": len(mode_rows),
                "accepted": len(mode_rows) - len(bad),
                "passed": (exp_n is None or len(mode_rows) == exp_n) and not bad and (not strict or impl_status == "linked"),
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
                "implementation_status": impl_status,
                "executed": len(row["case_ids"]) - len(missing),
                "accepted": len(row["case_ids"]) - len(missing) - len(bad),
                "passed": not missing and not bad and (not strict or impl_status == "linked"),
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
    cases_doc = load_json(root / M5_CASES_PATH)

    validate_baseline(root, inventory, corpus, mappings, cases_doc)
    static_cases = m5_differential.validate_static_cases(root, cases_doc)
    fmt_snippets = m5_differential.enumerate_formatter_snippet_cases(root)
    validated_reqs = validate_mappings(root, mappings, static_cases)
    inv_census = classify_inventory_items(root, inventory, corpus, mappings)

    static_by_lane = dict(sorted(Counter(c["lane"] for c in static_cases.values()).items()))
    total_by_lane = dict(static_by_lane)
    total_by_lane["formatter"] = total_by_lane.get("formatter", 0) + len(fmt_snippets)
    total_by_lane = dict(sorted(total_by_lane.items()))
    req_impl_counts = dict(sorted(Counter(r["implementation_status"] for r in validated_reqs.values()).items()))

    return {
        "schema_version": 1,
        "baseline": inventory["baseline"],
        "scope": (
            "Complete M5 Product APIs & Tools manifest: Rust public evaluator API, full product `pkl` CLI "
            "(eval/test/run/shell-completion/project/download-package/analyze/--help/--version), `pkl repl` "
            "(pipe + POSIX PTY), `pkl server` (MessagePack 0x20-0x32), `pkl format` (30 corpus goldens + "
            "CLI + V1/V2), and standalone `pkldoc` (HTML/JSON/symlinks/17 static web assets/stdlib "
            "doc-package-info.pkl). 2026-11-20 JVM/codegen exclusions are preserved as census-only items. "
            "Freshness checks never claim runtime compatibility passes (`coverage_gate: null`)."
        ),
        "sources": {
            INVENTORY_PATH.as_posix(): digest((root / INVENTORY_PATH).read_bytes()),
            CORPUS_PATH.as_posix(): digest((root / CORPUS_PATH).read_bytes()),
            MAPPINGS_PATH.as_posix(): digest((root / MAPPINGS_PATH).read_bytes()),
            M5_CASES_PATH.as_posix(): digest((root / M5_CASES_PATH).read_bytes()),
            m5_differential.API_PROBE_JAVA.as_posix(): digest((root / m5_differential.API_PROBE_JAVA).read_bytes()),
            "tools/m5_server_client.py": digest((root / "tools/m5_server_client.py").read_bytes()),
            "tools/m5_repl_driver.py": digest((root / "tools/m5_repl_driver.py").read_bytes()),
            "tools/m5_pkldoc_tree.py": digest((root / "tools/m5_pkldoc_tree.py").read_bytes()),
        },
        "counts": {
            "inventory_cli_command_total": inv_census["cli_command"]["total"],
            "inventory_cli_command_dispositions": inv_census["cli_command"]["counts"],
            "inventory_cli_argument_total": inv_census["cli_argument"]["total"],
            "inventory_cli_option_total": inv_census["cli_option"]["total"],
            "inventory_cli_option_dispositions": inv_census["cli_option"]["counts"],
            "inventory_cli_option_m5_new_breakdown": inv_census["cli_option"]["m5_new_breakdown"],
            "inventory_protocol_tag_total": inv_census["protocol_tag"]["total"],
            "inventory_protocol_tag_dispositions": inv_census["protocol_tag"]["counts"],
            "inventory_protocol_field_total": inv_census["protocol_field"]["total"],
            "inventory_protocol_field_dispositions": inv_census["protocol_field"]["counts"],
            "inventory_resource_total": inv_census["resource"]["total"],
            "inventory_resource_dispositions": inv_census["resource"]["counts"],
            "pkldoc_vendored_assets_status": inv_census["resource"]["pkldoc_assets_status"],
            "pkldoc_vendored_assets_exact_matches": inv_census["resource"]["pkldoc_assets_exact_matches"],
            "stdlib_doc_package_info_items": inv_census["stdlib_doc_package_info"]["total"],
            "functional_stdlib_items": inv_census["functional_stdlib"]["total"],
            "corpus_formatter_snippet_cases": len(fmt_snippets),
            "excluded_2026_11_20_java_api": inv_census["excluded_2026_11_20_census"]["java_api_total"],
            "excluded_2026_11_20_kotlin_api": inv_census["excluded_2026_11_20_census"]["kotlin_api_total"],
            "m5_requirement_groups": len(validated_reqs),
            "m5_requirement_impl_status": req_impl_counts,
            "m5_static_cases": len(static_cases),
            "m5_dynamic_formatter_snippet_cases": len(fmt_snippets),
            "m5_total_5lane_cases": len(static_cases) + len(fmt_snippets),
            "m5_cases_by_lane": total_by_lane,
        },
        "inventory_accounting": inv_census,
        "formatter_snippets": [
            {
                "id": c["id"],
                "corpus_case_id": c["corpus_case_id"],
                "input_path": c["input_path"],
                "input_sha256": c["input_sha256"],
                "golden_path": c["golden_path"],
                "golden_sha256": c["golden_sha256"],
                "expected_exit": c["expected_exit"],
            }
            for c in fmt_snippets
        ],
        "requirements": validated_reqs,
        "coverage_gate": None,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--write", action="store_true", help="write compatibility/m5-product-tools.json")
    parser.add_argument("--check", action="store_true", help="verify compatibility/m5-product-tools.json freshness (no runtime pass claim)")
    parser.add_argument("--verify-coverage", type=Path, help="verify 5-lane differential report meets 100%% strict M5 coverage")
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
                    print(f"M5 coverage gap: {line}", file=sys.stderr)
                return 1
            return 0

        manifest = build_manifest(root)
        data = encoded(manifest)
        if args.check:
            if not out_path.is_file() or out_path.read_bytes() != data:
                raise ValueError(f"M5 manifest missing or stale: {out_path}; run tools/m5_manifest.py --write")
        elif args.write:
            out_path.parent.mkdir(parents=True, exist_ok=True)
            out_path.write_bytes(data)
        print(json.dumps(manifest["counts"], sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        print(f"M5 manifest error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
