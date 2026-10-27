"""Execution mapping and parent/child integrity; no Java needed for these checks."""
import copy
import json
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compat
import m2_execution_profiles as profiles
import m2_jvm_classpath as jvm
import m2_historical as historical
import m2_language as language
import m2_manifest as manifest


class ProfileMappingTests(unittest.TestCase):
    def test_same_original_id_is_selected_once_with_explicit_profile(self):
        current = manifest.current_manifest()
        rows = [row for row in current['cases'] if row['id'] == jvm.CASE_ID]
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]['adapter'], jvm.ADAPTER)
        self.assertEqual(rows[0]['upstream_case']['execution']['status'], 'blocked')
        self.assertEqual(rows[0]['upstream_case']['expected_exit'], [0])
        self.assertEqual(len(language.selected_cases(current)), current['counts']['cases_by_execution']['ready'])
        self.assertEqual(len(language.selected_cases(current, [jvm.ADAPTER])), 1)
        self.assertIn('required case incomplete: ' + jvm.CASE_ID, manifest.full_gate_failures(current, []))
        failures = manifest.full_gate_failures(current, [dict(id=jvm.CASE_ID, status='pass')])
        self.assertNotIn('required case incomplete: ' + jvm.CASE_ID, failures)
        self.assertTrue(failures)

    def test_original_corpus_order_scope_and_all_other_cli_states_are_preserved(self):
        current = manifest.current_manifest()
        corpus = manifest.load(Path('compatibility/corpus.json'))
        originals = [row for row in corpus['cases'] if row['suite'] == 'language-snippet']
        # Historical admission also stores an upstream_case contract. Keep its
        # exact eight rows separate from the original 957 upstream snippets.
        all_original_contracts = [row for row in current['cases'] if 'upstream_case' in row]
        actual = [row for row in all_original_contracts if row['id'].startswith('pkl:language-snippet:')]
        history = historical.registry_cases(manifest.ROOT)
        self.assertEqual(len(history), 8)
        self.assertEqual([row['id'] for row in all_original_contracts],
                         [row['id'] for row in originals] + list(history))
        for row in all_original_contracts[len(originals):]:
            self.assertEqual(row['upstream_case'], history[row['id']]['contract'])
            self.assertEqual(row['adapter'], historical.ADAPTER)
            self.assertEqual(row['execution']['state'], 'ready')
        self.assertEqual([row['id'] for row in actual], [row['id'] for row in originals])
        self.assertEqual(len(actual), 957)
        self.assertEqual(current['counts']['cases'], 4527 + 18)
        self.assertEqual(current['counts']['requirements'], 475)
        self.assertEqual(current['counts']['cases_by_execution']['unimplemented'], 68 + 3)
        self.assertEqual(current['counts']['cases_by_execution']['unmapped'], 32)
        self.assertEqual(current['counts']['cases_by_execution']['ready'], 4442)
        import m2_module_eval
        registered = m2_module_eval.registry_cases(manifest.ROOT, corpus, manifest.load(manifest.MODULE_ADAPTERS))
        for original, case in zip(originals, actual):
            self.assertEqual(case['upstream_case'], original)
            self.assertEqual(case['input_sha256'], original['source']['sha256'])
            if case['id'] != jvm.CASE_ID:
                self.assertEqual(case['execution'], m2_module_eval.execution_state(original, case['id'] in registered))
                self.assertEqual(case['adapter'], m2_module_eval.ADAPTER if case['id'] in registered else None)

    def test_source_catalog_has_no_runner_import_cycle_in_any_entry_order(self):
        for order in ['m2_language,m2_manifest,m2_jvm_classpath',
                      'm2_jvm_classpath,m2_manifest,m2_language',
                      'm2_manifest,m2_jvm_classpath,m2_language']:
            result = subprocess.run([sys.executable, '-c', 'import ' + order],
                                    cwd=manifest.ROOT / 'tools', capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_execution_profile_hash_adapter_and_original_contract_forgery_is_rejected(self):
        original = manifest.current_manifest()
        for field in ('adapter', 'profile_sha256', 'upstream_case', 'input_sha256', 'execution'):
            current = copy.deepcopy(original)
            row = next(row for row in current['cases'] if row['id'] == jvm.CASE_ID)
            if field == 'profile_sha256': row['execution_profile'][field] = '0' * 64
            elif field == 'upstream_case': row[field]['expected_exit'] = [1]
            elif field == 'execution': row[field] = dict(state='unimplemented', reason='forged')
            else: row[field] = 'forged'
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'execution profile'):
                manifest.validate_manifest(current)

    def test_registry_duplicate_or_unreviewed_profile_cannot_promote_any_case(self):
        registry = profiles.expected_registry(manifest.ROOT)
        corpus = manifest.load(Path('compatibility/corpus.json'))
        for field in ('duplicate', 'profile_sha256', 'id', 'schema_boolean'):
            changed = copy.deepcopy(registry)
            if field == 'duplicate': changed['cases'].append(changed['cases'][0])
            elif field == 'schema_boolean': changed['schema'] = True
            else: changed['cases'][0][field] = 'forged'
            with (patch.object(profiles, 'expected_registry', return_value=registry),
                  patch.object(profiles.json, 'loads', return_value=changed)):
                with self.subTest(field=field), self.assertRaises(ValueError):
                    profiles.registry_cases(manifest.ROOT, corpus)


class ParentChildTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.output = Path(self.temporary.name) / 'parent'
        (self.output / 'jvm-classpath').mkdir(parents=True)
        report_path = self.output / 'jvm-classpath/report.json'
        report_path.write_bytes(b'child evidence\n')
        workspace = str(self.output.with_name('separate-workspace'))
        self.parent = dict(classpath_eval=dict(directory='jvm-classpath', workspace=workspace, parent_output=str(self.output),
            child_report_sha256=compat.digest(report_path.read_bytes())),
            classpath_candidate=dict(sha256='native'), source_sha256={'source': 'sha'}, oracle={'pinned': True})
        self.child = dict(workspace=workspace, output=str(self.output / 'jvm-classpath'), oracle={'pinned': True}, sources={'source': 'sha'},
            candidate=dict(source_binding='unbound-caller-supplied',
                           original_path=str(self.output / 'artifacts/jvm_snippet_probe'), sha256='native'))

    def test_child_is_bound_to_parent_artifact_sources_oracle_and_external_workspace(self):
        with patch.object(jvm, 'verify_report', return_value=self.child):
            self.assertEqual(language.verify_classpath_child(self.output, self.parent), self.child)
        for field in ('artifact', 'source', 'oracle', 'path', 'binding'):
            child = copy.deepcopy(self.child)
            if field == 'artifact': child['candidate']['sha256'] = 'forged'
            elif field == 'source': child['sources'] = {}
            elif field == 'oracle': child['oracle'] = {}
            elif field == 'path': child['candidate']['original_path'] = '/unrelated/artifact'
            else: child['candidate']['source_binding'] = 'recorded-build'
            with patch.object(jvm, 'verify_report', return_value=child):
                with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'parent artifact/source/oracle'):
                    language.verify_classpath_child(self.output, self.parent)
        self.parent['classpath_eval']['workspace'] = str(self.output / 'hidden-workspace')
        with self.assertRaisesRegex(ValueError, 'workspace boundary'):
            language.verify_classpath_child(self.output, self.parent)

    def test_parent_rejects_child_cargo_cache_before_any_walk(self):
        cache = self.output / 'jvm-classpath/build'
        cache.mkdir()
        (cache / 'not-evidence').write_bytes(b'must not be scanned')
        with self.assertRaisesRegex(ValueError, 'Cargo build cache'):
            list(language.evidence_paths(self.output))


if __name__ == '__main__':
    unittest.main()
