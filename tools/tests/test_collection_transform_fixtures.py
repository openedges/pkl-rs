"""Hermetic raw/counter/effect/provenance forgeries, including resigned manifests."""
from contextlib import contextmanager
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('collection_transform_fixtures', ROOT / 'tools/collection_transform_fixtures.py')
tool = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tool)
GOLDEN = Path(os.environ.get('COLLECTION_TRANSFORM_GOLDEN', ROOT / 'tests/fixtures/m2-collection-transforms')).absolute()


class CollectionFixtureIntegrity(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = json.loads((GOLDEN / 'manifest.json').read_bytes())
        cls.data = json.loads((GOLDEN / 'fixtures.json').read_bytes())
        private = ROOT / 'target/m2/stage51-collection-fixture-tests-private'
        private.mkdir(parents=True, exist_ok=True)
        cls.temporary = tempfile.TemporaryDirectory(dir=private)
        cls.copy = Path(cls.temporary.name) / 'fixture'
        for rel in [*cls.manifest['files'], 'manifest.json']:
            p = cls.copy / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            os.link(GOLDEN / rel, p)
        cls.before = {rel: row['sha256'] for rel, row in cls.manifest['files'].items()}

    @classmethod
    def tearDownClass(cls):
        # Test mutations replace clone files, never shared hardlink contents.
        for rel, expected in cls.before.items():
            actual = hashlib.sha256((GOLDEN / rel).read_bytes()).hexdigest()
            if actual != expected:
                raise AssertionError('original golden changed: ' + rel)
        cls.temporary.cleanup()

    @contextmanager
    def changed(self, relative, data, resign=True):
        path = self.copy / relative
        original_manifest = (self.copy / 'manifest.json').read_bytes()
        path.unlink()
        path.write_bytes(data)
        if resign:
            manifest = json.loads(original_manifest)
            manifest['files'][relative] = {'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}
            p = self.copy / 'manifest.json'
            p.unlink()
            p.write_bytes(tool.json_bytes(manifest))
        try:
            yield
        finally:
            path.unlink()
            os.link(GOLDEN / relative, path)
            if relative != 'manifest.json' and resign:
                p = self.copy / 'manifest.json'
                p.unlink()
                p.write_bytes(original_manifest)

    def forge_fixture(self, change):
        value = json.loads((GOLDEN / 'fixtures.json').read_bytes())
        change(value)
        return self.changed('fixtures.json', tool.json_bytes(value))

    def rejected(self):
        with self.assertRaises((ValueError, OSError, KeyError)):
            tool.verify(self.copy)

    def test_real_fixture_all_raw_rows_and_scopes(self):
        result = tool.verify(self.copy)
        self.assertEqual(result['sessions'], 152)
        self.assertEqual(result['javaApiCalls'], 2108)
        self.assertEqual(result['scopeLedger'], {'ordinary': 2000, 'intent-not-reached': 96, 'public-api-boundary': 12})
        self.assertFalse(result['fullJavaAdministrativeLifecyclePassed'])

    def test_no_external_canonical_payload_reads(self):
        original = Path.read_bytes
        def local(path):
            if not path.absolute().is_relative_to(self.copy):
                raise AssertionError('non-hermetic read: ' + str(path))
            return original(path)
        with patch.object(Path, 'read_bytes', local):
            self.assertEqual(tool.verify(self.copy)['javaApiCalls'], 2108)

    def test_resigned_expected_stdout_units(self):
        def change(data):
            data['fixtures'][0]['expected'][0]['stdoutUnits'][0] ^= 1
        with self.forge_fixture(change):
            self.rejected()

    def test_resigned_expected_java_utf8(self):
        def change(data):
            data['fixtures'][0]['expected'][0]['stdoutHex'] += '00'
        with self.forge_fixture(change):
            self.rejected()

    def test_resigned_status_newline_is_not_trimmed(self):
        def change(data):
            data['fixtures'][0]['expected'][0]['statusRawHex'] = b'success'.hex()
        with self.forge_fixture(change):
            self.rejected()

    def test_resigned_reads_and_logs_counters(self):
        def change(data):
            f = next(f for f in data['fixtures'] if f['readerRules'])
            f['expected'][1]['readsAfter'] += 1
            f['expected'][1]['logsAfter'] += 1
        with self.forge_fixture(change):
            self.rejected()

    def test_resigned_effect_order(self):
        def change(data):
            f = next(f for f in data['fixtures'] if len(f['expectedEffects']) >= 2)
            f['expectedEffects'][0], f['expectedEffects'][1] = f['expectedEffects'][1], f['expectedEffects'][0]
        with self.forge_fixture(change):
            self.rejected()

    def test_resigned_trace_nullable_member_and_source_line(self):
        def change(data):
            frame = next(e['frame'] for f in data['fixtures'] for e in f['expectedEffects'] if e['kind'] == 'log')
            frame['memberNameNull'] = not frame['memberNameNull']
            frame['sourceLines'][0]['units'].append(0)
        with self.forge_fixture(change):
            self.rejected()

    def test_resigned_reader_rule_cannot_steer_host(self):
        def change(data):
            next(f for f in data['fixtures'] if f['readerRules'])['readerRules'][0]['value'] = 'steer-host'
        with self.forge_fixture(change):
            self.rejected()

    def test_resigned_intent_cannot_become_ordinary(self):
        def change(data):
            c = next(c for f in data['fixtures'] for c in f['expected'] if c['scope'] == 'intent-not-reached')
            c['scope'] = 'ordinary'
        with self.forge_fixture(change):
            self.rejected()

    def test_resigned_post_close_cannot_become_language_error(self):
        def change(data):
            c = next(c for f in data['fixtures'] for c in f['expected'] if c['scope'] == 'public-api-boundary')
            c['scope'] = 'ordinary'
            c['oracleErrorClass'] = 'org.pkl.core.PklException'
            c['status'] = 'error'
        with self.forge_fixture(change):
            self.rejected()

    def test_null_guard_error_cannot_be_hidden_as_intent_failure(self):
        def change(data):
            f = next(f for f in data['fixtures'] if f['oracle'] == 'java04' and f['caseId'] == 'null-argument-guard')
            f['expected'][8]['scope'] = 'intent-not-reached'
        with self.forge_fixture(change):
            self.rejected()

    def test_duration_allowlist_misreach_cannot_be_ordinary(self):
        def change(data):
            f = next(f for f in data['fixtures'] if f['oracle'] == 'java07' and f['caseId'] == 'duration-type-render')
            f['expected'][9]['scope'] = 'ordinary'
        with self.forge_fixture(change):
            self.rejected()

    def test_duration_identity_error_hint_cannot_be_erased(self):
        def change(data):
            f = next(f for f in data['fixtures'] if f['oracle'] == 'java08' and f['caseId'] == 'duration-unit-literal-different-tostring')
            c = f['expected'][1]
            text = bytes.fromhex(c['stderrHex']).decode('utf-8')
            self.assertIn('This value was converted', text)
            c['stderrHex'] = text.split('This value was converted')[0].encode().hex()
            c['stderrUnits'] = tool.units(bytes.fromhex(c['stderrHex']).decode().encode('utf-16-be'))
        with self.forge_fixture(change):
            self.rejected()

    def test_duration_ordinary_bad_type_cannot_be_filtered(self):
        def change(data):
            f = next(f for f in data['fixtures'] if f['oracle'] == 'java07' and f['caseId'] == 'duration-type-render')
            self.assertEqual(f['expected'][11]['status'], 'error')
            f['expected'][11]['scope'] = 'intent-not-reached'
        with self.forge_fixture(change):
            self.rejected()

    def test_resigned_raw_payload_still_pinned(self):
        rel = 'provenance/java01/cases/list-core/0/raw/session-0/call-0.stdout.bin'
        with self.changed(rel, b'8'):
            self.rejected()

    def test_resigned_raw_counter_still_pinned(self):
        rel = 'provenance/java01/cases/list-core/0/raw/session-0/call-0.counts-after.bin'
        with self.changed(rel, b'0\t0\t999'):
            self.rejected()

    def test_resigned_report_counters_still_literal_pinned(self):
        rel = 'provenance/java01/report.json'
        value = json.loads((GOLDEN / rel).read_bytes())
        value['processes'][0]['calls'][0]['countsAfter'][0] += 1
        with self.changed(rel, tool.json_bytes(value)):
            self.rejected()

    def test_resigned_inventory_fails_before_scandir(self):
        rel = 'provenance/java01/inventory.json'
        value = json.loads((GOLDEN / rel).read_bytes())
        next(iter(value['files'].values()))['sha256'] = '0' * 64
        with self.changed(rel, tool.json_bytes(value)):
            with patch.object(tool.os, 'scandir', side_effect=AssertionError('walked before inventory authentication')):
                self.rejected()

    def test_undeclared_directory_rejected_before_descent(self):
        extra = self.copy / 'cargo-target'
        extra.mkdir()
        (extra / 'not-evidence').write_bytes(b'unlisted')
        original = os.scandir
        def guard(path):
            if Path(path) == extra:
                raise AssertionError('descended undeclared cache-like directory')
            return original(path)
        try:
            with patch.object(tool.os, 'scandir', guard):
                self.rejected()
        finally:
            shutil.rmtree(extra)

    def test_even_empty_undeclared_directory_rejected(self):
        extra = self.copy / 'empty-unlisted'
        extra.mkdir()
        try:
            self.rejected()
        finally:
            extra.rmdir()

    def test_resigned_manifest_extra_does_not_authorize_payload(self):
        rel = 'undeclared.bin'
        p = self.copy / rel
        p.write_bytes(b'bad')
        m = json.loads((GOLDEN / 'manifest.json').read_bytes())
        m['files'][rel] = {'sha256': hashlib.sha256(b'bad').hexdigest(), 'bytes': 3}
        try:
            with self.changed('manifest.json', tool.json_bytes(m), resign=False):
                self.rejected()
        finally:
            p.unlink()

    def test_manifest_schema_bool_is_rejected(self):
        value = json.loads((GOLDEN / 'manifest.json').read_bytes())
        value['schema'] = True
        with self.changed('manifest.json', tool.json_bytes(value), resign=False):
            self.rejected()

    def test_manifest_unknown_row_field_is_rejected(self):
        value = json.loads((GOLDEN / 'manifest.json').read_bytes())
        next(iter(value['files'].values()))['untrustedSource'] = '/tmp/foreign-payload'
        with self.changed('manifest.json', tool.json_bytes(value), resign=False):
            self.rejected()

    def test_missing_required_file(self):
        rel = 'provenance/java01/cases/list-core/0/raw/session-0/call-0.stdout.bin'
        p = self.copy / rel
        p.unlink()
        try:
            self.rejected()
        finally:
            os.link(GOLDEN / rel, p)

    def test_symlink_declared_payload_rejected(self):
        rel = 'provenance/java01/cases/list-core/0/raw/session-0/call-0.stdout.bin'
        p = self.copy / rel
        p.unlink()
        p.symlink_to(GOLDEN / rel)
        try:
            self.rejected()
        finally:
            p.unlink()
            os.link(GOLDEN / rel, p)

    def test_provenance_map_join_forgery(self):
        value = json.loads((GOLDEN / 'provenance-map.json').read_bytes())
        next(iter(value.values()))['rawSession'] = 'provenance/java02/forged'
        with self.changed('provenance-map.json', tool.json_bytes(value)):
            self.rejected()

    def test_duplicate_json_key_rejected(self):
        with self.assertRaises(ValueError):
            tool.parse(b'{"a":1,"a":2}')


if __name__ == '__main__':
    unittest.main()
