"""Hermeticity and re-signed tampering checks against the actual Java944 fixture."""
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

REPO = Path(__file__).resolve().parents[2]
GOLDEN = Path(os.environ.get('MODULEKEY_EXTENDED_FIXTURES', REPO / 'tests/fixtures/m2-modulekey-extended'))
spec = importlib.util.spec_from_file_location('modulekey_extended_fixtures', REPO / 'tools/modulekey_extended_fixtures.py')
tool = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tool)


class ExtendedFixtures(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        private = REPO / 'target/m2/stage50-modulekey-extended-tests-private'
        private.mkdir(parents=True, exist_ok=True)
        cls.temp = tempfile.TemporaryDirectory(dir=private)
        cls.root = Path(cls.temp.name) / 'fixture'
        shutil.copytree(GOLDEN, cls.root, copy_function=os.link)
        cls.manifest_bytes = (cls.root / 'manifest.json').read_bytes()
        cls.bundle = json.loads((cls.root / 'fixtures.json').read_bytes())
        cls.host = json.loads((cls.root / 'host-metadata.json').read_bytes())

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    @staticmethod
    def atomic(path, data):
        temporary = path.with_name(path.name + '.test-new')
        temporary.write_bytes(data)
        temporary.replace(path)

    @contextmanager
    def mutate(self, updates, resign=True):
        old = {rel: (self.root / rel).read_bytes() for rel in updates}
        try:
            manifest = json.loads(self.manifest_bytes)
            for rel, data in updates.items():
                self.atomic(self.root / rel, data)
                if rel in manifest['files']:
                    manifest['files'][rel] = {'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}
            if resign:
                self.atomic(self.root / 'manifest.json', tool.dump(manifest))
            yield
        finally:
            for rel, data in old.items():
                self.atomic(self.root / rel, data)
            self.atomic(self.root / 'manifest.json', self.manifest_bytes)

    def change_json(self, name, callback):
        value = json.loads((self.root / name).read_bytes())
        callback(value)
        return {name: tool.dump(value)}

    def test_authentic_scope_preserves_states_internal_and_intent_boundaries(self):
        result = tool.verify(self.root)
        self.assertEqual((result['javaApiCalls'], result['javaOrderedEvents'], result['sessions']), (944, 10124, 140))
        self.assertEqual(result['scopeLedger'], {'ordinary': 836, 'intent-not-reached-diagnostic': 100, 'internal-boundary': 8})
        synthetic = next(p for p in self.host['profiles'] if p['id'] == 'java06/actual-factory-synthetic-cached-false')
        self.assertEqual(synthetic['initial']['sourceState'], 'good')
        self.assertNotIn('good', synthetic['sources'])  # First sourceone operation precedes use.
        self.assertEqual(synthetic['customModules'][0]['sourceMode'], 'state-at-factory')

    def test_hermetic_guard_allows_only_copied_payload_reads(self):
        original = Path.read_bytes

        def guarded(path):
            if not path.is_relative_to(self.root):
                raise AssertionError('outside copied fixture: ' + str(path))
            return original(path)

        with patch.object(Path, 'read_bytes', guarded):
            self.assertEqual(tool.verify(self.root)['evaluations'], 0)

    def test_literal_authorities_cannot_be_self_signed(self):
        for name in ['report.json', 'inventory.json', 'closure.json']:
            rel = 'provenance/java01/' + name
            with self.subTest(name=name), self.mutate({rel: (self.root / rel).read_bytes() + b' '}):
                with self.assertRaisesRegex(ValueError, 'literal Java'):
                    tool.construct(self.root)

    def test_raw_api_channels_counters_and_error_class_reject_resigned_tampering(self):
        prefix = self.bundle['fixtures'][0]['expected'][0]['javaRawPrefix']
        for suffix in ['.status.bin', '.stdout.bin', '.stdout.code-units.bin', '.stderr.bin',
                       '.counters-before.bin', '.error-class.bin', '.message-null.bin']:
            rel = prefix + suffix
            with self.subTest(suffix=suffix), self.mutate({rel: (self.root / rel).read_bytes() + b'x'}):
                with self.assertRaisesRegex(ValueError, 'pinned original'):
                    tool.construct(self.root)

    def test_expected_counter_units_and_scope_cannot_hide_original(self):
        for field, value in [('stdoutUnits', [99]), ('stdoutHex', '63'), ('countersBefore', {'event': 0}),
                             ('scope', 'ordinary'), ('status', 'error')]:
            def change(bundle):
                index = next(i for i, e in enumerate(bundle['fixtures'][0]['expected'])
                             if e['scope'] == 'intent-not-reached-diagnostic') if field == 'scope' else 0
                bundle['fixtures'][0]['expected'][index][field] = value
            with self.subTest(field=field), self.mutate(self.change_json('fixtures.json', change)):
                with self.assertRaisesRegex(ValueError, 'literal fixture manifest|reconstructed'):
                    tool.verify(self.root)

    def test_host_profile_source_kind_cache_and_source_mode_cannot_change(self):
        for action in [lambda p: p['sourceProfiles']['main.pkl'].update(kind='contents', sourceId='main.pkl'),
                       lambda p: p['customModules'][0].update(sourceMode='state-at-factory'),
                       lambda p: p['customModules'][0].update(cacheMode='false'),
                       lambda p: p['customModules'][0].update(keyKind='synthetic'),
                       lambda p: p['customModules'][0].update(selectedUri='memo:/redirect.pkl')]:
            with self.subTest(action=action), self.mutate(self.change_json('host-metadata.json', lambda m: action(m['profiles'][0]))):
                with self.assertRaisesRegex(ValueError, 'literal fixture manifest|reconstructed'):
                    tool.verify(self.root)

    def test_override_capabilities_and_negative_default_are_source_bound(self):
        for key in ['uriOverrides', 'capabilities', 'elementDefaults']:
            def change(metadata):
                profile = next(p for p in metadata['profiles'] if p[key])
                profile[key].pop()
            with self.subTest(key=key), self.mutate(self.change_json('host-metadata.json', change)):
                with self.assertRaisesRegex(ValueError, 'literal fixture manifest|reconstructed'):
                    tool.verify(self.root)

    def test_policy_cause_and_io_encoding_provenance_are_distinct(self):
        for action in [lambda m: next(p for p in m['profiles'] if p['rawCauses'])['rawCauses'][0].update(message='formatted fake'),
                       lambda m: next(p for p in m['profiles'] if 'io' in p['sources'])['sources']['io'].update(encodingProvenance='independent-java-utf16')]:
            with self.subTest(action=action), self.mutate(self.change_json('host-metadata.json', action)):
                with self.assertRaisesRegex(ValueError, 'literal fixture manifest|reconstructed'):
                    tool.verify(self.root)

    def test_operations_reordering_is_not_a_new_expected_sequence(self):
        def change(bundle):
            fixture = next(f for f in bundle['fixtures'] if any(o['kind'] == 'source' for o in f['operations']))
            fixture['operations'].reverse()
        with self.mutate(self.change_json('fixtures.json', change)):
            with self.assertRaisesRegex(ValueError, 'literal fixture manifest|reconstructed'):
                tool.verify(self.root)

    def test_authored_source_and_inventory_cannot_be_reissued(self):
        rel = 'original/java01/workspace/inference-basename-implicit/child.pkl'
        inventory = json.loads((self.root / 'provenance/java01/inventory.json').read_bytes())
        payload = b'tag = "forged"\n'
        inventory['files'][rel.removeprefix('original/java01/')].update(sha256=hashlib.sha256(payload).hexdigest(), bytes=len(payload))
        with self.mutate({rel: payload, 'provenance/java01/inventory.json': tool.dump(inventory)}):
            with self.assertRaisesRegex(ValueError, 'literal Java'):
                tool.construct(self.root)

    def test_unknown_directory_rejected_before_descent(self):
        unknown = self.root / 'undeclared-cargo-target'
        unknown.mkdir()
        original = os.scandir
        entered = []

        def guarded(path):
            if Path(path) == unknown:
                entered.append(str(path))
                raise AssertionError('descended unknown directory')
            return original(path)

        try:
            with patch.object(os, 'scandir', guarded), self.assertRaisesRegex(ValueError, 'before descent'):
                tool.verify(self.root)
            self.assertEqual(entered, [])
        finally:
            unknown.rmdir()

    def test_extra_file_and_symlink_payload_are_rejected(self):
        extra = self.root / 'extra.bin'
        extra.write_bytes(b'not declared')
        try:
            with self.assertRaisesRegex(ValueError, 'coverage'):
                tool.verify(self.root)
        finally:
            extra.unlink()
        path = self.root / 'original/java01/AdmissionProbe.java'
        data = path.read_bytes(); path.unlink(); path.symlink_to(GOLDEN / 'original/java01/AdmissionProbe.java')
        try:
            with self.assertRaisesRegex(ValueError, 'regular.*payload'):
                tool.verify(self.root)
        finally:
            path.unlink(); path.write_bytes(data)


if __name__ == '__main__':
    unittest.main()
