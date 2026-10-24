import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import parameterized_object_fixtures as fixtures


def refresh(root):
    path = root / 'manifest.json'
    manifest = json.loads(path.read_text())
    manifest['files'] = {
        p.relative_to(root).as_posix(): {
            'sha256': hashlib.sha256(p.read_bytes()).hexdigest(), 'bytes': p.stat().st_size}
        for p in root.rglob('*') if p.is_file() and p != path
    }
    path.write_text(json.dumps(manifest))


class ParameterizedObjectFixtureTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name) / 'fixture'
        shutil.copytree(fixtures.DEFAULT, self.root)

    def edit(self, name, callback):
        path = self.root / name
        value = json.loads(path.read_text())
        callback(value)
        path.write_text(json.dumps(value))
        refresh(self.root)

    def test_copied_fixture_verifies_without_original_target_or_java(self):
        result = fixtures.verify(self.root)
        self.assertEqual(result['ordinary_cases'], 85)
        self.assertEqual(result['ordinary_java_processes'], 170)
        self.assertEqual(result['oracle_error_cases'], 7)
        self.assertEqual(result['compatibility_passes_claimed'], 0)

    def test_self_consistent_raw_replacement_cannot_replace_java_authority(self):
        cases = json.loads((self.root / 'cases.json').read_text())
        row = next(c for c in cases if c['observations'][0]['exit_code'] == 0)
        raw = self.root / row['observations'][0]['channels']['stdout']['path']
        raw.write_bytes(b'forged output')
        for observation in row['observations']:
            observation['channels']['stdout'].update(sha256=hashlib.sha256(raw.read_bytes()).hexdigest(), bytes=raw.stat().st_size)
        (self.root / 'cases.json').write_text(json.dumps(cases))
        refresh(self.root)
        with self.assertRaisesRegex(ValueError, 'raw Java channel differs'):
            fixtures.verify(self.root)

    def test_source_replacement_rejected_after_all_local_hashes_are_updated(self):
        cases = json.loads((self.root / 'cases.json').read_text())
        source = cases[0]['source']
        path = self.root / source
        path.write_bytes(path.read_bytes() + b'\n// replacement\n')
        for row in cases:
            if row['source'] == source:
                row['source_sha256'] = hashlib.sha256(path.read_bytes()).hexdigest()
        (self.root / 'cases.json').write_text(json.dumps(cases))
        refresh(self.root)
        with self.assertRaisesRegex(ValueError, 'source bytes differ from Java'):
            fixtures.verify(self.root)

    def test_dropped_ordinary_case_rejected_after_manifest_refresh(self):
        self.edit('cases.json', lambda rows: rows.pop())
        with self.assertRaisesRegex(ValueError, 'ordinary Java selection differs'):
            fixtures.verify(self.root)

    def test_internal_error_cannot_be_promoted_to_ordinary(self):
        boundaries = json.loads((self.root / 'oracle-boundaries.json').read_text())
        boundaries[0]['classification'] = 'ordinary'
        self.edit('cases.json', lambda rows: rows.append(boundaries[0]))
        with self.assertRaisesRegex(ValueError, 'ordinary Java selection differs'):
            fixtures.verify(self.root)

    def test_authority_report_cannot_be_replaced_with_a_new_manifest(self):
        self.edit('provenance/original-reports/03.json', lambda report: report.update(baseline='forged'))
        with self.assertRaisesRegex(ValueError, 'independent Java authority differs'):
            fixtures.verify(self.root)

    def test_diagnostic_uri_cannot_be_rewritten(self):
        self.edit('cases.json', lambda rows: rows[0].update(source_uri='file:///rewritten.pkl'))
        with self.assertRaisesRegex(ValueError, 'source/expression/classification differs'):
            fixtures.verify(self.root)

    def test_resource_replacement_rejected_after_manifest_refresh(self):
        (self.root / 'inputs/05/gate.txt').write_bytes(b'stop')
        refresh(self.root)
        with self.assertRaisesRegex(ValueError, 'Java resource bytes differ'):
            fixtures.verify(self.root)

    def test_extra_file_rejected_even_when_listed(self):
        (self.root / 'extra.bin').write_bytes(b'extra')
        refresh(self.root)
        with self.assertRaisesRegex(ValueError, 'fixture file coverage differs'):
            fixtures.verify(self.root)

    def test_symlink_is_rejected_before_following_it(self):
        (self.root / 'extra-link').symlink_to(self.root / 'inputs', target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'symlink'):
            fixtures.verify(self.root)


if __name__ == '__main__':
    unittest.main()
