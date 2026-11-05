"""Language source snapshots retain inputs without reopening provenance bundles."""
from contextlib import contextmanager
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m2_source_inputs as subject


class ProvenanceEntry:
    def __init__(self, entry):
        self.path = entry.path
        self.name = entry.name

    def is_dir(self, **kwargs):
        raise AssertionError('provenance archive metadata accessed')

    def is_file(self, **kwargs):
        raise AssertionError('provenance archive metadata accessed')

    def stat(self, **kwargs):
        raise AssertionError('provenance archive metadata accessed')


class LanguageSourceInputsTests(unittest.TestCase):
    def test_exact_provenance_exclusions_precede_metadata_and_keep_binary_inputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)

            def write(name, data=b'input'):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)

            for index, name in enumerate(sorted(subject.PROVENANCE_ARCHIVES)):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                if index == 0:
                    write(name, b'closed synthetic provenance')
                elif index == 1:
                    # Even an unexpected directory must be excluded before
                    # metadata access or recursion into its contents.
                    write(name + '/unexpected.pkl')
                else:
                    path.symlink_to(root / 'missing-provenance', target_is_directory=index == 3)
            retained = {
                'tests/fixtures/m2/binary-input.bin',
                'tests/fixtures/m2/archive-input.zip',
                'tests/fixtures/m2-deprecation/resource.tar.gz',
                'tests/fixtures/m2-deprecation/inputs/source.pkl',
                'tests/fixtures/m2-class-values/manifest.json',
                'crates/pkl-runtime/src/generated.rs',
                'crates/pkl-runtime/src/stdlib/input.pkl',
                'crates/pkl-runtime/Cargo.toml',
                'crates/pkl-runtime/tests/fixtures/m2-glob-lossless-source/raw.bin',
            }
            for name in retained:
                write(name)
            resources = ['external/pkl/classpath/input.bin', 'tests/fixtures/historical/raw.pkl',
                         'tests/fixtures/selected/raw.pkl']
            write(subject.HISTORICAL, json.dumps({'resources': [{'path': resources[1]}]}).encode())
            write(subject.SELECTED_REGRESSIONS, json.dumps({'resources': [{'path': resources[2]}]}).encode())
            write(subject.PROFILE, json.dumps({'reference': {'sources': []},
                  'classpath_resources': [resources[0]], 'input': {'path': 'external/pkl/input.pkl'}}).encode())
            scandir = subject.os.scandir
            read_bytes = Path.read_bytes

            def guarded_scandir(path):
                iterator = scandir(path)

                @contextmanager
                def guarded_entries():
                    with iterator as entries:
                        # Reverse enumeration order; catalog order is stable.
                        yield iter([ProvenanceEntry(entry)
                                    if Path(entry.path).relative_to(root).as_posix() in subject.PROVENANCE_ARCHIVES
                                    else entry for entry in reversed(list(entries))])

                return guarded_entries()

            def guarded_read(path):
                if path.relative_to(root).as_posix() in subject.PROVENANCE_ARCHIVES:
                    raise AssertionError('provenance archive body accessed')
                return read_bytes(path)

            with patch.object(subject.os, 'scandir', guarded_scandir), \
                    patch.object(Path, 'read_bytes', guarded_read):
                observed = subject.source_paths(root)
                repeated = subject.source_paths(root)
            self.assertEqual(observed, sorted(set(observed)))
            self.assertEqual(observed, repeated)
            self.assertTrue(retained.union(resources) <= set(observed))
            self.assertTrue(subject.PROVENANCE_ARCHIVES.isdisjoint(observed))

    def test_source_directory_symlink_is_not_recursed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            outside = root / 'outside'
            outside.mkdir()
            (outside / 'not-a-source.pkl').write_bytes(b'outside')
            base = root / 'tests/fixtures/m2'
            base.mkdir(parents=True)
            (base / 'directory-link').symlink_to(outside, target_is_directory=True)
            (base / 'file-link.pkl').symlink_to(outside / 'not-a-source.pkl')
            self.assertEqual(list(subject.glob_source_paths(root, 'tests/fixtures/m2', '*')),
                             ['tests/fixtures/m2/file-link.pkl'])


if __name__ == '__main__':
    unittest.main()
