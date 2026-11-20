"""Unit and negative tests for tools/m4_manifest.py."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m4_differential
import m4_manifest
import oracle

ROOT = m4_manifest.ROOT


class M4ManifestTests(unittest.TestCase):
    def test_build_manifest_matches_committed_file_and_counts(self) -> None:
        manifest = m4_manifest.build_manifest(ROOT)
        counts = manifest["counts"]
        self.assertEqual(counts["inventory_resource_total"], 27)
        self.assertEqual(
            counts["inventory_resource_dispositions"],
            {"deferred:M5": 17, "m4-required": 3, "non-applicable": 4, "shared-core": 3},
        )
        self.assertEqual(counts["inventory_protocol_tag_total"], 19)
        self.assertEqual(counts["inventory_protocol_tag_dispositions"], {"deferred:M5": 6, "m4-required": 13})
        self.assertEqual(counts["inventory_protocol_field_total"], 119)
        self.assertEqual(counts["inventory_protocol_field_dispositions"], {"deferred:M5": 82, "m4-required": 37})
        self.assertEqual(counts["inventory_cli_command_total"], 17)
        self.assertEqual(
            counts["inventory_cli_command_dispositions"],
            {"deferred:M5": 7, "excluded": 2, "m4-exercised": 8},
        )
        self.assertEqual(counts["inventory_cli_option_total"], 86)
        self.assertEqual(counts["inventory_cli_option_dispositions"]["m4-required"], 31)
        self.assertEqual(counts["functional_stdlib_items"], 104)
        self.assertTrue(counts["bundled_ca_roots_exact_match"])
        self.assertEqual(counts["m4_requirement_groups"], 16)
        for rid, req in manifest["requirements"].items():
            self.assertTrue(req["implementation_links"], f"missing implementation_links for {rid}")
        self.assertEqual(counts["m4_static_cases"], 86)
        self.assertEqual(counts["pantry_verified_offline_remote_packages"], 7)
        self.assertEqual(counts["pantry_dev_cases"], 126)
        self.assertEqual(counts["pantry_packaged_cases"], 217)
        self.assertEqual(counts["m4_total_5lane_cases"], 429)
        self.assertIsNone(manifest["coverage_gate"])

        committed = (ROOT / m4_manifest.OUTPUT_PATH).read_bytes()
        self.assertEqual(committed, m4_manifest.encoded(manifest))

    def test_validate_mappings_rejects_unmapped_static_case(self) -> None:
        mappings = m4_manifest.load_json(ROOT / m4_manifest.MAPPINGS_PATH)
        cases_doc = m4_manifest.load_json(ROOT / m4_manifest.M4_CASES_PATH)
        static_cases = m4_differential.validate_static_cases(ROOT, cases_doc)
        extra_cases = dict(static_cases)
        extra_cases["m4:http:unmapped-case"] = {"id": "m4:http:unmapped-case", "lane": "host-io-http-security"}
        with self.assertRaisesRegex(ValueError, "unmapped static M4 cases"):
            m4_manifest.validate_mappings(ROOT, mappings, extra_cases)

    def test_validate_mappings_rejects_unknown_case_id(self) -> None:
        mappings = copy.deepcopy(m4_manifest.load_json(ROOT / m4_manifest.MAPPINGS_PATH))
        cases_doc = m4_manifest.load_json(ROOT / m4_manifest.M4_CASES_PATH)
        static_cases = m4_differential.validate_static_cases(ROOT, cases_doc)
        mappings["requirements"]["m4:host:http-basic-and-resources"]["case_ids"].append("m4:does:not-exist")
        with self.assertRaisesRegex(ValueError, "unknown static case_id"):
            m4_manifest.validate_mappings(ROOT, mappings, static_cases)

    def test_validate_mappings_rejects_missing_implementation_symbol(self) -> None:
        mappings = copy.deepcopy(m4_manifest.load_json(ROOT / m4_manifest.MAPPINGS_PATH))
        cases_doc = m4_manifest.load_json(ROOT / m4_manifest.M4_CASES_PATH)
        static_cases = m4_differential.validate_static_cases(ROOT, cases_doc)
        mappings["requirements"]["m4:host:http-basic-and-resources"]["implementation_links"] = [
            {"path": "crates/pkl-runtime/src/lib.rs", "symbol": "DefinitelyNonExistentM4SymbolXYZ"}
        ]
        with self.assertRaisesRegex(ValueError, "implementation link symbol missing"):
            m4_manifest.validate_mappings(ROOT, mappings, static_cases)

    def test_validate_mappings_rejects_empty_implementation_links(self) -> None:
        mappings = copy.deepcopy(m4_manifest.load_json(ROOT / m4_manifest.MAPPINGS_PATH))
        cases_doc = m4_manifest.load_json(ROOT / m4_manifest.M4_CASES_PATH)
        static_cases = m4_differential.validate_static_cases(ROOT, cases_doc)
        mappings["requirements"]["m4:host:http-basic-and-resources"]["implementation_links"] = []
        with self.assertRaisesRegex(ValueError, "empty implementation_links"):
            m4_manifest.validate_mappings(ROOT, mappings, static_cases)

    def test_validate_static_cases_rejects_tampered_input_sha(self) -> None:
        cases_doc = copy.deepcopy(m4_manifest.load_json(ROOT / m4_manifest.M4_CASES_PATH))
        cases_doc["cases"][0]["inputs"][0]["sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "sha256 mismatch"):
            m4_differential.validate_static_cases(ROOT, cases_doc)


if __name__ == "__main__":
    unittest.main()
