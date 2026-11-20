"""Unit and negative tests for tools/m3_manifest.py mechanical M3 coverage gate."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m3_manifest

ROOT = m3_manifest.ROOT


def build_complete_synthetic_census(inventory: dict, mappings: dict) -> dict:
    """Build a synthetic 100%-passing census matching all 1,781 M3 inventory items."""
    m3 = m3_manifest.extract_m3_inventory(inventory)
    deferred_paths = {d["path"] for d in mappings.get("deferred_sources", [])}
    inv_source_sha = {src["path"]: src["sha256"] for src in inventory["sources"]}

    vendored_files = []
    for src_path in sorted(p for p in inv_source_sha if p.startswith("stdlib/")):
        fname = Path(src_path).name
        if src_path in deferred_paths:
            vendored_files.append(
                {
                    "filename": fname,
                    "upstream_sha256": inv_source_sha[src_path],
                    "vendored_sha256": None,
                    "status": "missing",
                }
            )
        else:
            vendored_files.append(
                {
                    "filename": fname,
                    "upstream_sha256": inv_source_sha[src_path],
                    "vendored_sha256": inv_source_sha[src_path],
                    "status": "exact_match",
                }
            )

    modules: dict[str, dict] = {}
    for item in m3["stdlib"]:
        src_path = item["source"]["path"]
        if src_path in deferred_paths:
            continue
        uri = "pkl:" + Path(src_path).stem
        mod = modules.setdefault(
            uri,
            {
                "status": "loaded",
                "error": None,
                "reflected": {
                    "uri": uri,
                    "module_name": None,
                    "properties": [],
                    "methods": [],
                    "type_aliases": [],
                    "classes": {},
                },
            },
        )
        if item.get("visibility", "public") == "local":
            continue
        ref = mod["reflected"]
        kind = item["kind"]
        sym = item["symbol"]
        if kind == "module":
            ref["module_name"] = sym
        else:
            _, rest = sym.split("#", 1)
            if "." in rest:
                cls_name, mem_name = rest.split(".", 1)
                cls_obj = ref["classes"].setdefault(cls_name, {"properties": [], "methods": []})
                bucket = "properties" if kind == "property" else "methods"
                if mem_name not in cls_obj[bucket]:
                    cls_obj[bucket].append(mem_name)
            else:
                if kind == "class":
                    ref["classes"].setdefault(rest, {"properties": [], "methods": []})
                elif kind == "typealias":
                    if rest not in ref["type_aliases"]:
                        ref["type_aliases"].append(rest)
                elif kind == "property":
                    if rest not in ref["properties"]:
                        ref["properties"].append(rest)
                elif kind == "function":
                    if rest not in ref["methods"]:
                        ref["methods"].append(rest)

    builtins = {
        item["id"]: {
            "symbol": item["symbol"],
            "kind": item["kind"],
            "status": "implemented",
            "outcome": "evaluated_ok",
            "detail": None,
        }
        for item in m3["builtin"]
    }
    renderers = {
        item["id"]: {
            "symbol": item["symbol"],
            "kind": item["kind"],
            "status": "implemented",
            "outcome": "evaluated_ok",
            "detail": None,
        }
        for item in [
            *m3["renderer"],
            {"id": m3_manifest.PKL_BINARY_FORMAT_ID, "symbol": "pkl-binary", "kind": "output-format"},
        ]
    }
    return {
        "schema_version": 1,
        "vendored_stdlib_files": vendored_files,
        "modules": modules,
        "builtins": builtins,
        "renderers": renderers,
    }


class M3ManifestTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.inventory = m3_manifest.load_json(ROOT / m3_manifest.INVENTORY_PATH)
        cls.mappings = m3_manifest.load_json(ROOT / m3_manifest.MAPPINGS_PATH)
        cls.cases_doc = m3_manifest.load_json(ROOT / m3_manifest.M3_CASES_PATH)
        cls.m2_doc = m3_manifest.load_json(ROOT / m3_manifest.M2_MANIFEST_PATH)
        cls.m2_case_ids = {row["id"] for row in cls.m2_doc.get("cases", [])}

    def test_inventory_extraction_and_probe_plan_counts(self) -> None:
        m3 = m3_manifest.extract_m3_inventory(self.inventory)
        self.assertEqual(len(m3["stdlib"]), 1222)
        self.assertEqual(len(m3["builtin"]), 542)
        self.assertEqual(len(m3["renderer"]), 17)

        plan_lines = m3_manifest.build_probe_plan(self.inventory)
        self.assertEqual(len(plan_lines), 1 + 542 + 17 + 1)
        seen_ids = set()
        for line in plan_lines[1:]:
            cols = line.split("\t")
            self.assertEqual(len(cols), 7)
            category, iid, _sym, _kind, src, expr, _fmt = cols
            self.assertIn(category, ("builtin", "renderer"))
            self.assertNotIn(iid, seen_ids)
            seen_ids.add(iid)
            self.assertTrue(src.strip())
            self.assertTrue(expr.strip())
        self.assertIn(m3_manifest.PKL_BINARY_FORMAT_ID, seen_ids)

    def test_repo_manifest_build_passes_with_expected_counts(self) -> None:
        manifest = m3_manifest.build_manifest(ROOT)
        counts = manifest["counts"]
        self.assertEqual(counts["inventory_total_m3_category_items"], 1781)
        self.assertEqual(counts["stdlib_total"], 1222)
        self.assertEqual(counts["stdlib_m3_required"], 1214)
        self.assertEqual(counts["stdlib_deferred"], 8)
        self.assertEqual(counts["builtin_total"], 542)
        self.assertEqual(counts["renderer_total"], 17)
        self.assertEqual(counts["probe_plan_rows"], 560)
        self.assertEqual(counts["m3_differential_cases"], 118)
        self.assertEqual(
            counts["m3_differential_cases_by_lane"],
            {"formats-renderers": 77, "output-files": 13, "pklbinary-decode": 28},
        )
        self.assertIsNone(manifest["coverage_gate"])

    def test_baseline_and_case_manifest_tamper_rejected(self) -> None:
        bad_inv = copy.deepcopy(self.inventory)
        bad_inv["baseline"]["revision"] = "0000000000000000000000000000000000000000"
        with self.assertRaisesRegex(ValueError, "inventory baseline"):
            m3_manifest.validate_baseline(ROOT, bad_inv, self.mappings, self.cases_doc)

        bad_map = copy.deepcopy(self.mappings)
        bad_map["baseline"]["version"] = "0.99.0"
        with self.assertRaisesRegex(ValueError, "m3-mappings.json baseline"):
            m3_manifest.validate_baseline(ROOT, self.inventory, bad_map, self.cases_doc)

        bad_cases = copy.deepcopy(self.cases_doc)
        bad_cases["cases"][0]["inputs"][0]["sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "sha256 mismatch"):
            m3_manifest.validate_cases_manifest(ROOT, bad_cases)

        bad_path = copy.deepcopy(self.cases_doc)
        bad_path["cases"][0]["inputs"][0]["path"] = "../outside.pkl"
        with self.assertRaisesRegex(ValueError, "invalid M3 fixture input path"):
            m3_manifest.validate_cases_manifest(ROOT, bad_path)

        dup_cases = copy.deepcopy(self.cases_doc)
        dup_cases["cases"].append(copy.deepcopy(dup_cases["cases"][0]))
        with self.assertRaisesRegex(ValueError, "duplicate M3 case id"):
            m3_manifest.validate_cases_manifest(ROOT, dup_cases)

        bad_lane = copy.deepcopy(self.cases_doc)
        bad_lane["cases"][0]["lane"] = "mystery-lane"
        with self.assertRaisesRegex(ValueError, "unknown M3 lane"):
            m3_manifest.validate_cases_manifest(ROOT, bad_lane)

    def test_mapping_validation_rejects_missing_module_symbol_or_case(self) -> None:
        m3_cases = m3_manifest.validate_cases_manifest(ROOT, self.cases_doc)

        missing_mod = copy.deepcopy(self.mappings)
        del missing_mod["module_links"]["pkl:math"]
        with self.assertRaisesRegex(ValueError, "module_links mismatch"):
            m3_manifest.validate_mappings(ROOT, self.inventory, missing_mod, m3_cases, self.m2_case_ids)

        bad_sym = copy.deepcopy(self.mappings)
        bad_sym["module_links"]["pkl:math"]["implementation_links"][0]["symbol"] = "nonexistent_rust_symbol_xyz"
        with self.assertRaisesRegex(ValueError, "symbol missing"):
            m3_manifest.validate_mappings(ROOT, self.inventory, bad_sym, m3_cases, self.m2_case_ids)

        bad_case = copy.deepcopy(self.mappings)
        bad_case["module_links"]["pkl:math"]["case_ids"].append("m3:does-not-exist")
        with self.assertRaisesRegex(ValueError, "unknown case_ids"):
            m3_manifest.validate_mappings(ROOT, self.inventory, bad_case, m3_cases, self.m2_case_ids)

        bad_renderer_case = copy.deepcopy(self.mappings)
        bad_renderer_case["renderer_links"]["json"].append("m3:bogus")
        with self.assertRaisesRegex(ValueError, "unknown renderer_links"):
            m3_manifest.validate_mappings(ROOT, self.inventory, bad_renderer_case, m3_cases, self.m2_case_ids)

    def test_evaluate_census_passing_and_deferred_accounting(self) -> None:
        census = build_complete_synthetic_census(self.inventory, self.mappings)
        verdict = m3_manifest.evaluate_census(self.inventory, census, self.mappings, ROOT)
        self.assertTrue(verdict["passed"], verdict["failures"][:5])
        self.assertEqual(verdict["counts"]["failure_count"], 0)
        self.assertEqual(verdict["counts"]["vendored_stdlib_files"], {"deferred": 1, "exact_match": 23})
        self.assertEqual(verdict["counts"]["stdlib"], {"deferred": 8, "implemented": 1214})
        self.assertEqual(verdict["counts"]["builtin"], {"implemented": 542})
        self.assertEqual(verdict["counts"]["renderer"], {"implemented": 18})

    def test_evaluate_census_negative_mutations_fail_closed(self) -> None:
        base = build_complete_synthetic_census(self.inventory, self.mappings)

        # 1. Omitted vendored stdlib file
        c = copy.deepcopy(base)
        c["vendored_stdlib_files"] = [r for r in c["vendored_stdlib_files"] if r["filename"] != "base.pkl"]
        v = m3_manifest.evaluate_census(self.inventory, c, self.mappings, ROOT)
        self.assertFalse(v["passed"])
        self.assertTrue(any("omitted from census: stdlib/base.pkl" in f for f in v["failures"]))

        # 2. Byte-mismatched vendored stdlib file
        c = copy.deepcopy(base)
        for r in c["vendored_stdlib_files"]:
            if r["filename"] == "math.pkl":
                r["vendored_sha256"] = "f" * 64
                r["status"] = "mismatch"
        v = m3_manifest.evaluate_census(self.inventory, c, self.mappings, ROOT)
        self.assertFalse(v["passed"])
        self.assertTrue(any("mismatch/missing" in f and "math.pkl" in f for f in v["failures"]))

        # 3. Upstream hash drift vs inventory.json
        c = copy.deepcopy(base)
        c["vendored_stdlib_files"][0]["upstream_sha256"] = "0" * 64
        v = m3_manifest.evaluate_census(self.inventory, c, self.mappings, ROOT)
        self.assertFalse(v["passed"])
        self.assertTrue(any("upstream stdlib sha256 disagrees" in f for f in v["failures"]))

        # 4. Omitted builtin
        c = copy.deepcopy(base)
        first_bid = next(iter(c["builtins"]))
        del c["builtins"][first_bid]
        v = m3_manifest.evaluate_census(self.inventory, c, self.mappings, ROOT)
        self.assertFalse(v["passed"])
        self.assertEqual(v["counts"]["builtin"].get("omitted"), 1)

        # 5. Builtin stub returning unsupported
        c = copy.deepcopy(base)
        c["builtins"][first_bid]["status"] = "unsupported"
        c["builtins"][first_bid]["detail"] = "stubbed"
        v = m3_manifest.evaluate_census(self.inventory, c, self.mappings, ROOT)
        self.assertFalse(v["passed"])
        self.assertEqual(v["counts"]["builtin"].get("unsupported"), 1)

        # 6. Omitted pkl-binary renderer
        c = copy.deepcopy(base)
        del c["renderers"][m3_manifest.PKL_BINARY_FORMAT_ID]
        v = m3_manifest.evaluate_census(self.inventory, c, self.mappings, ROOT)
        self.assertFalse(v["passed"])
        self.assertEqual(v["counts"]["renderer"].get("omitted"), 1)

        # 7. Renderer marked unsupported
        c = copy.deepcopy(base)
        c["renderers"][m3_manifest.PKL_BINARY_FORMAT_ID]["status"] = "unsupported"
        v = m3_manifest.evaluate_census(self.inventory, c, self.mappings, ROOT)
        self.assertFalse(v["passed"])
        self.assertEqual(v["counts"]["renderer"].get("unsupported"), 1)

        # 8. Public stdlib member missing from live reflection
        c = copy.deepcopy(base)
        c["modules"]["pkl:math"]["reflected"]["properties"].remove("pi")
        v = m3_manifest.evaluate_census(self.inventory, c, self.mappings, ROOT)
        self.assertFalse(v["passed"])
        self.assertTrue(any("pkl.math#pi" in f for f in v["failures"]))

        # 9. Module failed to load
        c = copy.deepcopy(base)
        c["modules"]["pkl:Command"]["status"] = "load_error"
        c["modules"]["pkl:Command"]["reflected"] = None
        v = m3_manifest.evaluate_census(self.inventory, c, self.mappings, ROOT)
        self.assertFalse(v["passed"])
        self.assertTrue(any("pkl:Command" in f for f in v["failures"]))

        # 10. Local external builtin (`pkl.EvaluatorSettings#resolvePath`) unsupported
        c = copy.deepcopy(base)
        resolve_bid = next(
            item["id"]
            for item in m3_manifest.extract_m3_inventory(self.inventory)["builtin"]
            if item["symbol"] == "pkl.EvaluatorSettings#resolvePath"
        )
        c["builtins"][resolve_bid]["status"] = "unsupported"
        v = m3_manifest.evaluate_census(self.inventory, c, self.mappings, ROOT)
        self.assertFalse(v["passed"])
        self.assertTrue(any("local external stdlib member not implemented" in f for f in v["failures"]))

    def test_cli_write_check_emit_plan_and_verify_coverage(self) -> None:
        script = str(ROOT / "tools/m3_manifest.py")
        with tempfile.TemporaryDirectory() as td:
            td_path = Path(td)
            out_manifest = td_path / "m3-stdlib-output.json"
            plan_path = td_path / "plan.tsv"
            census_path = td_path / "census.json"

            # --emit-plan
            proc = subprocess.run([sys.executable, script, "--emit-plan", str(plan_path)], capture_output=True, text=True)
            self.assertEqual(proc.returncode, 0, proc.stderr)
            self.assertEqual(json.loads(proc.stdout)["plan_rows"], 560)
            self.assertEqual(len(plan_path.read_text(encoding="utf-8").splitlines()), 561)

            # --write & --check round-trip
            proc = subprocess.run([sys.executable, script, "--write", "--output", str(out_manifest)], capture_output=True, text=True)
            self.assertEqual(proc.returncode, 0, proc.stderr)
            proc = subprocess.run([sys.executable, script, "--check", "--output", str(out_manifest)], capture_output=True, text=True)
            self.assertEqual(proc.returncode, 0, proc.stderr)

            # Tamper 1 byte -> --check must fail
            raw = out_manifest.read_bytes()
            out_manifest.write_bytes(raw + b" ")
            proc = subprocess.run([sys.executable, script, "--check", "--output", str(out_manifest)], capture_output=True, text=True)
            self.assertEqual(proc.returncode, 1)

            # --verify-coverage passing vs failing census
            census = build_complete_synthetic_census(self.inventory, self.mappings)
            census_path.write_bytes(m3_manifest.encoded(census))
            proc = subprocess.run([sys.executable, script, "--verify-coverage", str(census_path)], capture_output=True, text=True)
            self.assertEqual(proc.returncode, 0, proc.stderr)

            first_bid = next(iter(census["builtins"]))
            census["builtins"][first_bid]["status"] = "missing"
            census_path.write_bytes(m3_manifest.encoded(census))
            proc = subprocess.run([sys.executable, script, "--verify-coverage", str(census_path)], capture_output=True, text=True)
            self.assertEqual(proc.returncode, 1)


if __name__ == "__main__":
    unittest.main()
