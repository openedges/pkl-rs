"""Authority substitution and complete raw Class/public-provider evidence checks."""
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import class_values_fixtures as fixtures
import class_values_oracle as oracle
import class_values_exports as exports


class FixtureTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name) / 'fixture'
        shutil.copytree(fixtures.DEFAULT, self.root)

    def refresh(self):
        records = oracle.inventory(self.root)
        records.pop('manifest.json', None)
        oracle.put(self.root / 'manifest.json', dict(schema=1, baseline='Pkl 0.32.0', files=records))

    def edit(self, name, change):
        path = self.root / name
        value = json.loads(path.read_bytes())
        change(value)
        oracle.put(path, value)
        self.refresh()

    def test_offline_copy_retains_all_nonpasses_provider_and_public_export_boundaries(self):
        with patch('subprocess.Popen', side_effect=AssertionError('offline verifier launched a process')):
            result = fixtures.verify(self.root)
        self.assertEqual((result['sessions'], result['java_processes'], result['repeated_raw_equal']), (44, 88, 44))
        self.assertEqual(result['public_calls'], 558)
        self.assertEqual(result['call_status_counts'], {'success': 462, 'error': 86, 'oracle-error': 6, 'api-exception': 4})
        self.assertEqual(result['compatibility_passes_claimed'], 0)
        self.assertGreater(result['provider_supplement']['callback_counts']['create'], 0)
        self.assertGreater(result['provider_supplement']['callback_counts']['resolve'], 0)
        self.assertIn('unexecuted/required', result['provider_supplement']['scope'])
        export = result['public_export_supplement']
        self.assertEqual(export['public_calls'], 14)
        self.assertEqual(export['call_status_counts'], {'success': 12, 'api-exception': 2})
        self.assertEqual(export['observations']['different-evaluator']['reference'], 'false')
        self.assertEqual([r['sessions'] for r in result['preserved_history']], [42, 43, 44])

    def test_rehashed_replacement_archive_cannot_replace_independent_java_authority(self):
        path = self.root / 'oracle-evidence.tar.gz'
        path.write_bytes(path.read_bytes() + b'changed reference')
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'independent Java Class authority'):
            fixtures.verify(self.root)

    def test_rehashed_query_whitespace_cannot_replace_actual_java_input(self):
        self.edit('cases.json', lambda m: m['cases'][0]['expressions'].__setitem__(0, ' Any.simpleName'))
        with self.assertRaisesRegex(ValueError, 'main Class selection differs'):
            fixtures.verify(self.root)

    def test_rehashed_support_source_cannot_replace_executed_module_bytes(self):
        path = self.root / 'inputs/plain-module.pkl'
        path.write_bytes(path.read_bytes() + b'\n// changed original input\n')
        def update(m):
            for case in m['cases']:
                for source in case['support_modules']:
                    if source['source'] == 'inputs/plain-module.pkl':
                        source['source_sha256'] = oracle.sha(path)
        self.edit('cases.json', update)
        with self.assertRaisesRegex(ValueError, 'main Class selection differs'):
            fixtures.verify(self.root)

    def test_public_export_supplement_cannot_be_replaced_by_refreshing_metadata(self):
        self.edit('export-cases.json', lambda m: m['cases'][0].__setitem__('other_uri', m['cases'][0]['uri']))
        with self.assertRaisesRegex(ValueError, 'public-export selection differs'):
            fixtures.verify(self.root)

    def test_extra_undeclared_payload_cannot_be_admitted_by_manifest_refresh(self):
        (self.root / 'extra.bin').write_bytes(b'extra')
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'file-set identity differs'):
            fixtures.verify(self.root)

    def test_boolean_callback_threshold_is_not_integer_zero(self):
        self.edit('cases.json', lambda m: m['cases'][0]['logger'].__setitem__('callback_after', False))
        with self.assertRaisesRegex(ValueError, 'invalid Class logger profile'):
            oracle.load_cases(self.root / 'cases.json', self.root)


class RawObservationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.authority = Path(cls.temporary.name)
        with tarfile.open(fixtures.DEFAULT / 'oracle-evidence.tar.gz', 'r:gz') as stream:
            stream.extractall(cls.authority, filter='data')
        cls.manifest = json.loads((cls.authority / 'main/sources/tests/fixtures/m2-class-values/cases.json').read_bytes())

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def copy_case(self, name):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        target = Path(temporary.name) / name
        shutil.copytree(self.authority / 'main/cases' / name / '0', target)
        return target, next(c for c in self.manifest['cases'] if c['id'] == name)

    def test_real_nested_logger_reentry_failure_observation_is_complete(self):
        root, case = self.copy_case('annotation-log-reentry-throw')
        result = oracle.audit_session(root, case)
        self.assertEqual(result['logger_callbacks'], 1)
        self.assertEqual(result['call_statuses']['log-callback-0'], 'success')
        self.assertEqual(result['call_statuses']['outer-0'], 'oracle-error')
        self.assertEqual(result['call_statuses']['outer-3'], 'error')

    def test_changed_java_utf8_cannot_hide_unchanged_utf16(self):
        root, case = self.copy_case('unicode-class-name-and-java-string')
        (root / 'outer-3.stdout.bin').write_bytes(b'replacement')
        with self.assertRaisesRegex(ValueError, 'Java UTF8 differs'):
            oracle.audit_session(root, case)

    def test_missing_provider_event_is_not_a_complete_session(self):
        root, case = self.copy_case('module-extends-amends-class')
        (root / 'provider/event-0.kind.bin').unlink()
        with self.assertRaisesRegex(ValueError, 'provider callback coverage'):
            oracle.audit_session(root, case)

    def test_provider_match_is_bound_to_authored_uri_inputs(self):
        root, case = self.copy_case('module-extends-amends-class')
        (root / 'provider/event-0.matched.bin').write_bytes(b'true')
        with self.assertRaisesRegex(ValueError, 'provider match differs'):
            oracle.audit_session(root, case)

    def test_provider_extra_ghost_channel_is_rejected(self):
        root, case = self.copy_case('module-extends-amends-class')
        (root / 'provider/event-999.kind.bin').write_bytes(b'create')
        with self.assertRaisesRegex(ValueError, 'provider callback coverage'):
            oracle.audit_session(root, case)

    def test_extra_ghost_common_call_metadata_is_rejected(self):
        root, case = self.copy_case('explicit-typed')
        (root / 'outer-999.error-class.bin').write_bytes(b'forged provenance')
        with self.assertRaisesRegex(ValueError, 'extra/missing common Class channel'):
            oracle.audit_session(root, case)

    def test_protocol_order_is_checked_after_process_bytes_are_changed_together(self):
        root, case = self.copy_case('annotation-log-reentry')
        protocol = (root / 'protocol.bin').read_bytes().replace(b'log-enter=0 level=trace depth=0', b'log-enter=0 level=trace depth=7')
        (root / 'protocol.bin').write_bytes(protocol)
        (root / 'process/stdout.bin').write_bytes(protocol)
        with self.assertRaisesRegex(ValueError, 'logger entry order/depth'):
            oracle.audit_session(root, case)

    def test_public_export_api_exception_is_not_changed_to_success(self):
        root = self.authority / 'exports/cases/public-export-identity/0'
        result = exports.audit(root)
        self.assertEqual(result['call_statuses']['post-close-0'], 'api-exception')
        self.assertEqual(result['identity_comparisons']['same-uri-changed-source'], {'reference': 'true', 'equals': 'true'})


if __name__ == '__main__':
    unittest.main()
