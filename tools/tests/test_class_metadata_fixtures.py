"""Independent authority dispatch and complete additive Java raw evidence checks."""
import io
import json
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import class_metadata_fixtures as fixtures
import class_metadata_oracle as metadata
import class_values_fixtures as dispatch
import class_values_oracle as common


class FixtureTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name) / 'fixture'
        shutil.copytree(fixtures.DEFAULT, self.root)

    def refresh(self, kind=metadata.KIND):
        records = common.inventory(self.root)
        records.pop('manifest.json', None)
        common.put(self.root / 'manifest.json', dict(schema=1, kind=kind, baseline='Pkl 0.32.0', files=records))

    def test_actual_offline_dispatch_retains_errors_history_and_unacquired_exports(self):
        with patch('subprocess.Popen', side_effect=AssertionError('offline verifier launched a process')):
            result = dispatch.verify(self.root)
        self.assertEqual((result['sessions'], result['java_processes'], result['repeated_raw_equal']), (10, 20, 10))
        self.assertEqual(result['public_calls'], 244)
        self.assertEqual(result['call_status_counts'], {'success': 204, 'error': 40})
        self.assertEqual(result['preserved_history'][0]['call_status_counts'], {'success': 184, 'error': 60})
        self.assertEqual(result['compatibility_passes_claimed'], 0)
        self.assertGreater(result['provider_supplement']['callback_counts']['create'], 0)
        self.assertIn('unexecuted/required', result['provider_supplement']['scope'])
        self.assertEqual(result['public_export_supplement']['status'], 'not-acquired-required')
        self.assertIs(result['public_export_supplement']['required'], True)
        self.assertEqual(result['public_export_supplement']['public_calls'], 0)

    def test_unknown_authority_kind_is_rejected(self):
        self.refresh('pkl-java-invented-pass-kind')
        with self.assertRaisesRegex(ValueError, 'unknown Class fixture authority kind'):
            dispatch.verify(self.root)

    def test_removing_kind_cannot_admit_supplement_as_original_authority(self):
        manifest = json.loads((self.root / 'manifest.json').read_bytes())
        del manifest['kind']
        common.put(self.root / 'manifest.json', manifest)
        with self.assertRaisesRegex(ValueError, 'file-set identity differs'):
            dispatch.verify(self.root)

    def test_forged_known_kind_cannot_admit_original_fixture_as_supplement(self):
        original = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, original)
        shutil.copytree(dispatch.DEFAULT, original / 'fixture')
        manifest = json.loads((original / 'fixture/manifest.json').read_bytes())
        manifest['kind'] = metadata.KIND
        common.put(original / 'fixture/manifest.json', manifest)
        with self.assertRaisesRegex(ValueError, 'metadata fixture file-set identity differs'):
            dispatch.verify(original / 'fixture')

    def test_rehashed_replacement_archive_cannot_replace_authority(self):
        archive = self.root / 'oracle-evidence.tar.gz'
        archive.write_bytes(archive.read_bytes() + b'forged authority')
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'independent Java metadata authority differs'):
            dispatch.verify(self.root)

    def test_rehashed_query_change_does_not_replace_executed_java_query(self):
        path = self.root / 'cases.json'
        manifest = json.loads(path.read_bytes())
        manifest['cases'][0]['expressions'][0] += ' '
        common.put(path, manifest)
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'metadata selection differs from Java authority'):
            fixtures.verify(self.root)

    def test_rehashed_source_change_does_not_replace_executed_java_source(self):
        path = self.root / 'inputs/metadata.pkl'
        path.write_bytes(path.read_bytes() + b'\n// replacement\n')
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'metadata authored input differs'):
            fixtures.verify(self.root)

    def test_extra_file_cannot_be_admitted_by_manifest_refresh(self):
        (self.root / 'invented.bin').write_bytes(b'extra')
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'metadata fixture file-set identity differs'):
            fixtures.verify(self.root)

    def test_boolean_schema_is_not_numeric_one(self):
        manifest = json.loads((self.root / 'manifest.json').read_bytes())
        manifest['schema'] = True
        common.put(self.root / 'manifest.json', manifest)
        with self.assertRaisesRegex(ValueError, 'metadata fixture file bytes/kind differ'):
            fixtures.verify(self.root)

    def test_symlink_cannot_replace_payload(self):
        path = self.root / 'LICENSE'
        path.unlink()
        path.symlink_to(fixtures.DEFAULT / 'LICENSE')
        with self.assertRaisesRegex(ValueError, 'symlink'):
            fixtures.verify(self.root)

    def test_archive_paths_and_duplicate_members_checked_before_extraction(self):
        for names, error in [(['../escape'], 'unsafe Java metadata'), (['same', 'same'], 'duplicate Java metadata')]:
            with self.subTest(names=names):
                archive = self.root / 'oracle-evidence.tar.gz'
                with tarfile.open(archive, 'w:gz') as stream:
                    for name in names:
                        info = tarfile.TarInfo(name)
                        info.size = 1
                        stream.addfile(info, io.BytesIO(b'x'))
                self.refresh()
                with patch.object(fixtures, 'AUTHORITY_SHA256', common.sha(archive)):
                    with self.assertRaisesRegex(ValueError, error):
                        fixtures.verify(self.root)


class RawTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.authority = Path(cls.temporary.name)
        with tarfile.open(fixtures.DEFAULT / 'oracle-evidence.tar.gz', 'r:gz') as stream:
            stream.extractall(cls.authority, filter='data')
        cls.manifest = metadata.load_inputs(cls.authority / 'main/sources/experiment-inputs')

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def copy_case(self, name):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name) / name
        shutil.copytree(self.authority / 'main/cases' / name / '0', root)
        return root, next(case for case in self.manifest['cases'] if case['id'] == name)

    def test_actual_ordinary_errors_retain_exception_provenance_and_utf16(self):
        root, case = self.copy_case('ordinary-type-constraints')
        result = common.audit_session(root, case)
        self.assertEqual(result['call_statuses']['outer-0'], 'error')
        self.assertEqual(result['call_statuses']['outer-9'], 'success')
        self.assertEqual((root / 'outer-0.error-class.bin').read_bytes(), b'org.pkl.core.PklException')
        raw = common.payloads(root)
        common.wire.check_string(raw, 'outer-0.stderr')

    def test_supplementary_plane_value_cannot_be_replaced_with_java_utf8_only(self):
        root, case = self.copy_case('ordinary-valid-values')
        self.assertEqual((root / 'outer-3.stdout.code-units.bin').read_bytes(), 'a𐐀'.encode('utf-16-be'))
        (root / 'outer-3.stdout.bin').write_bytes(b'replacement')
        with self.assertRaisesRegex(ValueError, 'Java UTF8 differs'):
            common.audit_session(root, case)

    def test_provider_observations_cannot_be_silently_removed(self):
        root, case = self.copy_case('metadata-yamlrenderer')
        (root / 'provider/event-0.kind.bin').unlink()
        with self.assertRaisesRegex(ValueError, 'provider callback coverage'):
            common.audit_session(root, case)

    def test_ghost_common_error_metadata_is_not_an_observation(self):
        root, case = self.copy_case('ordinary-lazy-defaults')
        (root / 'outer-999.error-class.bin').write_bytes(b'fake')
        with self.assertRaisesRegex(ValueError, 'extra/missing common Class channel'):
            common.audit_session(root, case)

    def test_protocol_does_not_allow_promoting_real_java_error_to_success(self):
        root, case = self.copy_case('descriptor-prototype-separation')
        protocol = (root / 'protocol.bin').read_bytes().replace(b'outer-0 status=error', b'outer-0 status=success')
        (root / 'protocol.bin').write_bytes(protocol)
        (root / 'process/stdout.bin').write_bytes(protocol)
        with self.assertRaisesRegex(ValueError, 'Class call completion/status differs'):
            common.audit_session(root, case)


if __name__ == '__main__':
    unittest.main()
