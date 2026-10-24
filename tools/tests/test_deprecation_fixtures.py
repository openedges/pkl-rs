import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import deprecation_fixtures as fixtures
import m2_deprecation


class DeprecationFixtureTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name) / 'fixture'
        shutil.copytree(fixtures.DEFAULT, self.root)

    def refresh(self):
        files = m2_deprecation.inventory(self.root)
        files.pop('manifest.json', None)
        (self.root / 'manifest.json').write_text(json.dumps(dict(schema=1, baseline='Pkl 0.32.0', files=files)))

    def test_copied_fixture_verifies_without_java_or_original_target(self):
        result = fixtures.verify(self.root)
        self.assertEqual(result['java_processes'], result['sessions'] * 2)
        self.assertEqual(result['repeated_raw_equal'], result['sessions'])
        self.assertGreaterEqual(result['oracle_error_calls'], 10)
        self.assertEqual(result['compatibility_passes_claimed'], 0)

    def test_self_consistent_archive_replacement_cannot_replace_java_authority(self):
        path = self.root / 'oracle-evidence.tar.gz'
        path.write_bytes(path.read_bytes() + b'replacement')
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'independent Java authority'):
            fixtures.verify(self.root)

    def test_query_replacement_rejected_after_local_manifest_refresh(self):
        path = self.root / 'cases.json'
        cases = json.loads(path.read_bytes())
        cases['cases'][0]['expressions'][0] = '42'
        path.write_text(json.dumps(cases))
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'selection differs from Java authority'):
            fixtures.verify(self.root)

    def test_source_replacement_rejected_after_all_local_hashes_updated(self):
        cases_path = self.root / 'cases.json'
        cases = json.loads(cases_path.read_bytes())
        name = cases['cases'][0]['source']
        source = self.root / name
        source.write_bytes(source.read_bytes() + b'\n// replacement\n')
        for row in cases['cases']:
            if row['source'] == name:
                row['source_sha256'] = hashlib.sha256(source.read_bytes()).hexdigest()
        cases_path.write_text(json.dumps(cases))
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'selection differs from Java authority'):
            fixtures.verify(self.root)

    def test_extra_file_rejected_even_when_listed(self):
        (self.root / 'extra.bin').write_bytes(b'extra')
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'file coverage differs'):
            fixtures.verify(self.root)

    def test_extra_root_report_cannot_hide_in_acquisition_inventory_exception(self):
        (self.root / 'report.json').write_bytes(b'{}')
        with self.assertRaisesRegex(ValueError, 'file coverage differs'):
            fixtures.verify(self.root)

    def test_symlink_is_rejected_before_traversal(self):
        (self.root / 'loop').symlink_to(self.root, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'symlink'):
            fixtures.verify(self.root)

    def test_missing_both_outer_channels_cannot_establish_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            with self.assertRaisesRegex(ValueError, 'missing outer call'):
                m2_deprecation.audit_session(path, dict(expressions=['retained(1)']))

    def test_java_surrogate_encoding_stays_distinct_from_node_replacement(self):
        raw = {'message.bin': b'?', 'message.code-units.bin': b'\xd8\x00'}
        m2_deprecation.check_string(raw, 'message')
        raw['message.bin'] = b'\xef\xbf\xbd'
        with self.assertRaisesRegex(ValueError, 'UTF-8 differs'):
            m2_deprecation.check_string(raw, 'message')

    def test_error_line_endings_are_not_normalized(self):
        raw = {'error.bin': b'one\ntwo', 'error.code-units.bin': 'one\r\ntwo'.encode('utf-16-be')}
        with self.assertRaisesRegex(ValueError, 'UTF-8 differs'):
            m2_deprecation.check_string(raw, 'error')


if __name__ == '__main__':
    unittest.main()
