"""Independent Java authority and lossless full-frame evidence regression checks."""
import copy
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import frame_transformer_fixtures as fixtures
import frame_transformer_oracle as oracle


class FixtureTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name) / 'fixture'
        shutil.copytree(fixtures.DEFAULT, self.root)

    def refresh(self):
        files = oracle.inventory(self.root)
        files.pop('manifest.json', None)
        (self.root / 'manifest.json').write_text(json.dumps(dict(schema=1, baseline='Pkl 0.32.0', files=files)))

    def edit(self, name, change):
        path = self.root / name
        data = json.loads(path.read_bytes())
        change(data)
        path.write_text(json.dumps(data))
        self.refresh()

    def test_copied_fixture_verifies_without_java_original_target_or_subprocess(self):
        with patch('subprocess.Popen', side_effect=AssertionError('offline verifier launched a process')):
            result = fixtures.verify(self.root)
        self.assertEqual(result['sessions'], 48)
        self.assertEqual(result['java_processes'], 96)
        self.assertEqual(result['repeated_raw_equal'], 48)
        self.assertEqual(result['buffer_supplement']['sessions'], 7)
        self.assertEqual(result['buffer_supplement']['java_processes'], 14)
        self.assertEqual(result['compatibility_passes_claimed'], 0)
        self.assertGreater(result['oracle_error_calls'], 0)
        self.assertGreater(result['api_exception_calls'], 0)

    def test_resealed_replacement_archive_cannot_replace_independent_java_authority(self):
        path = self.root / 'oracle-evidence.tar.gz'
        path.write_bytes(path.read_bytes() + b'forged replacement')
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'independent Java frame authority'):
            fixtures.verify(self.root)

    def test_replacement_frame_edit_rejected_after_manifest_refresh(self):
        def change(data):
            next(row for row in data['cases'] if row['replacement'] is not None)['replacement']['memberName'] = None
        self.edit('cases.json', change)
        with self.assertRaisesRegex(ValueError, 'main frame selection differs'):
            fixtures.verify(self.root)

    def test_source_edit_with_updated_local_hashes_cannot_replace_java_input(self):
        cases_path = self.root / 'cases.json'
        cases = json.loads(cases_path.read_bytes())
        source = cases['cases'][0]['source']
        path = self.root / source
        path.write_bytes(path.read_bytes() + b'\n// replacement\n')
        for row in cases['cases']:
            if row['source'] == source:
                row['source_sha256'] = hashlib.sha256(path.read_bytes()).hexdigest()
        cases_path.write_text(json.dumps(cases))
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'main frame selection differs'):
            fixtures.verify(self.root)

    def test_java_only_buffer_supplement_cannot_be_dropped(self):
        self.edit('buffer-cases.json', lambda data: data['cases'].pop())
        with self.assertRaisesRegex(ValueError, 'buffer supplement differs'):
            fixtures.verify(self.root)

    def test_extra_file_cannot_be_admitted_by_updating_manifest(self):
        (self.root / 'extra.bin').write_bytes(b'extra')
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'file-set identity differs'):
            fixtures.verify(self.root)

    def test_root_report_cannot_hide_in_generic_inventory_exception(self):
        (self.root / 'report.json').write_bytes(b'{}')
        with self.assertRaisesRegex(ValueError, 'unexpected root report'):
            fixtures.verify(self.root)

    def test_symlink_is_rejected_before_following_it(self):
        (self.root / 'loop').symlink_to(self.root, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'symlink'):
            fixtures.verify(self.root)


class FrameChannelsTests(unittest.TestCase):
    def test_lone_surrogate_uses_java_utf8_without_losing_canonical_units(self):
        data = {'uri.bin': b'?', 'uri.code-units.bin': b'\xd8\x00'}
        oracle.check_string(data, 'uri')
        data['uri.bin'] = b'\xef\xbf\xbd'
        with self.assertRaisesRegex(ValueError, 'Java UTF8 differs'):
            oracle.check_string(data, 'uri')

    def test_line_endings_are_not_normalized(self):
        data = {'source.bin': b'one\ntwo', 'source.code-units.bin': 'one\r\ntwo'.encode('utf-16-be')}
        with self.assertRaisesRegex(ValueError, 'Java UTF8 differs'):
            oracle.check_string(data, 'source')

    def test_null_and_empty_member_remain_distinct_replacement_inputs(self):
        cases = oracle.load_cases(fixtures.DEFAULT / 'cases.json')['cases']
        null = next(row for row in cases if row['id'] == 'authored-warn-member-null')
        empty = next(row for row in cases if row['id'] == 'authored-warn-member-empty')
        left, right = oracle.input_files(null, b''), oracle.input_files(empty, b'')
        self.assertEqual(left['replacement/member.u16be'], right['replacement/member.u16be'])
        self.assertEqual(left['replacement/member-null.txt'], b'true')
        self.assertEqual(right['replacement/member-null.txt'], b'false')

    def test_boolean_and_float_values_cannot_become_frame_units_or_positions(self):
        case = next(row for row in oracle.load_cases(fixtures.DEFAULT / 'cases.json')['cases'] if row['replacement'])
        for value in [True, 1.0, -1, 65536]:
            with self.subTest(unit=value), self.assertRaises(ValueError):
                oracle.unit_bytes([value])
        frame = copy.deepcopy(case['replacement'])
        frame['startLine'] = True
        with self.assertRaisesRegex(ValueError, 'invalid frame position'):
            oracle.check_frame_config(frame)

    def test_empty_source_lines_are_preserved_as_a_real_public_frame(self):
        raw = {'x.uri.bin': b'pkl:base', 'x.uri.code-units.bin': 'pkl:base'.encode('utf-16-be'),
               'x.member.bin': b'', 'x.member.code-units.bin': b'', 'x.member.null.bin': b'true',
               'x.position.bin': b'1:1:1:1', 'x.source-count.bin': b'0'}
        oracle.audit_frame(raw, 'x')
        raw.update({'x.source-0.bin': b'', 'x.source-0.code-units.bin': b''})
        with self.assertRaisesRegex(ValueError, 'source line coverage differs'):
            oracle.audit_frame(raw, 'x')

    def test_missing_public_calls_cannot_be_treated_as_a_successful_empty_capture(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'process').mkdir()
            (root / 'process/stdout.bin').write_bytes(b'closed transforms=0 logs=0 warnings=0\n')
            with self.assertRaisesRegex(ValueError, 'missing public outer call'):
                oracle.audit_session(root, dict(operation='expression', expressions=['answer']))


if __name__ == '__main__':
    unittest.main()
