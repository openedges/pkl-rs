"""Pinned separate-module Java 0.32.0 union-marker evidence regressions."""
import json
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import union_marker_fixtures as fixture
from collection_conversions_fixtures import ROW_CHANNELS, validate_row_channels


class UnionMarkerFixtureTests(unittest.TestCase):
    def test_offline_sealed_capture(self):
        with patch('subprocess.Popen', side_effect=AssertionError('offline verifier launched Java')):
            result = fixture.verify()
        self.assertEqual(result['programs'], 40)
        self.assertEqual(result['statuses'], {'success': 9, 'error': 31})
        self.assertEqual(result['candidate_compatibility_passes_claimed'], 0)

    def test_first_error_order_and_utf16_source_highlights(self):
        programs = json.loads((fixture.DEFAULT / 'programs.json').read_bytes())['programs']
        names = {row['id']: index for index, row in enumerate(programs)}
        with tarfile.open(fixture.DEFAULT / 'oracle-evidence.tar.gz', 'r:gz') as archive:
            def read(identifier, channel):
                index = names[identifier]
                name = (f'{fixture.ARCHIVE_PREFIX}/programs/{index:02d}-{identifier}/'
                        f'repeat-0/000-{identifier}/{channel}.bin')
                return archive.extractfile(name).read()
            self.assertEqual(read('nonunion-int', 'result.status'), b'error\n')
            self.assertIn(b'Only type unions can have a default marker (*).',
                          read('nonunion-int', 'result.stderr'))
            self.assertIn(b'^' * 13, read('nonunion-comment-gap', 'result.stderr'))
            self.assertIn(b'typealias T = *"', read('nonunion-emoji', 'result.stderr'))
            self.assertIn(b'A type union cannot have more than one default type.',
                          read('duplicate-before-bad-generic', 'result.stderr'))
            self.assertIn(b'Unexpected token `>`. Expected a type.',
                          read('first-bad-generic', 'result.stderr'))
            self.assertIn(b'Only type unions can have a default marker (*).',
                          read('notunion-before-later-error', 'result.stderr'))
            self.assertEqual(read('union-first', 'result.status'), b'success\n')
            self.assertEqual(read('union-first', 'result.stdout'), b'1')

    def test_exact_distinct_module_bytes_and_order(self):
        programs = json.loads((fixture.DEFAULT / 'programs.json').read_bytes())['programs']
        self.assertEqual(len({row['id'] for row in programs}), 40)
        for index, row in enumerate(programs):
            self.assertEqual(row['module_file'], f"modules/{index:02d}-{row['id']}.pkl")
            self.assertEqual(row['query'], 'result')
            raw = (fixture.DEFAULT / row['module_file']).read_bytes()
            self.assertEqual(fixture.sha(raw), row['module_sha256'])
            self.assertEqual(len(raw), row['module_bytes'])

    def test_missing_utf16_channel_rejected(self):
        folder = (f'{fixture.ARCHIVE_PREFIX}/programs/00-nonunion-int/'
                  'repeat-0/000-nonunion-int/')
        files = {folder + name: b'' for name in ROW_CHANNELS}
        validate_row_channels(files, folder)
        files.pop(folder + 'result.stderr.code-units.bin')
        with self.assertRaisesRegex(ValueError, 'row channel set differs'):
            validate_row_channels(files, folder)

    def test_undeclared_path_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / 'fixture'
            shutil.copytree(fixture.DEFAULT, root)
            (root / 'extra').mkdir()
            with self.assertRaisesRegex(ValueError, 'undeclared or nonregular'):
                fixture.verify(root)

    def test_mutated_module_even_with_rewritten_manifest_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / 'fixture'
            shutil.copytree(fixture.DEFAULT, root)
            module = root / 'modules/00-nonunion-int.pkl'
            module.write_bytes(module.read_bytes() + b'other = 1\n')
            hashes = {p.relative_to(root).as_posix(): fixture.sha(p.read_bytes())
                      for p in root.rglob('*') if p.is_file() and p.name != 'manifest.json'}
            (root / 'manifest.json').write_text(json.dumps(
                {'schema': 1, 'baseline': 'Pkl 0.32.0', 'files': hashes}))
            with self.assertRaisesRegex(ValueError, 'authored module bytes/order differ'):
                fixture.verify(root)

if __name__ == '__main__': unittest.main()
