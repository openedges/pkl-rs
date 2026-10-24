"""Thin native separate-module replay uses sealed Java, not Rust-generated expectations."""
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import union_marker_compare as compare
from data_size_compare import COMMON, EVENT_METADATA


class NativeUnionMarkerTests(unittest.TestCase):
    def test_java_authority_has_eighty_distinct_fresh_sessions(self):
        rows, files, provenance, verdict = compare.java_authority(compare.FIXTURE)
        self.assertEqual(len(rows), 40)
        self.assertEqual(len(provenance['sessions']), 80)
        self.assertEqual(len({row['pid'] for row in provenance['sessions']}), 80)
        self.assertEqual(verdict['statuses'], {'success': 9, 'error': 31})
        self.assertEqual(files['programs/00-nonunion-int/repeat-0/000-nonunion-int/result.status.bin'], b'error\n')

    def test_exact_source_bytes_are_copied_separately_with_one_query(self):
        programs = compare.java_authority(compare.FIXTURE)[0]
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            shutil.copytree(compare.FIXTURE, root / 'fixture')
            inputs = compare.input_files(root, compare.FIXTURE, programs)
            self.assertEqual(len(inputs), 40)
            self.assertEqual(inputs[0].joinpath('queries.tsv').read_bytes(), b'string\tnonunion-int\tresult\n')
            self.assertEqual(inputs[0].joinpath('module.pkl').read_bytes(),
                             (compare.FIXTURE / programs[0]['module_file']).read_bytes())
            self.assertIn(b'\r\n', inputs[39].joinpath('module.pkl').read_bytes())
            self.assertEqual(compare.input_files_check(root, programs),
                             {p.relative_to(root).as_posix() for p in (root / 'inputs').rglob('*') if p.is_file()})

    def test_all_seven_raw_channels_and_metadata_are_required(self):
        java = compare.java_authority(compare.FIXTURE)[1]
        prefix = 'programs/00-nonunion-int/repeat-0/000-nonunion-int/'
        reference = {name: java[prefix + name] for name in COMMON}
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for name, raw in reference.items(): (root / name).write_bytes(raw)
            for name in EVENT_METADATA:
                (root / name).write_bytes(b'language-error' if name == 'candidate.disposition.bin' else b'')
            self.assertEqual(compare.compare_row(reference, root)['status'], 'pass')
            (root / 'result.stderr.code-units.bin').write_bytes(b'')
            result = compare.compare_row(reference, root)
            self.assertEqual(result['status'], 'mismatch')
            self.assertEqual(result['different_channels'], ['result.stderr.code-units.bin'])
            (root / 'result.stderr.code-units.bin').write_bytes(reference['result.stderr.code-units.bin'])
            (root / 'candidate.disposition.bin').write_bytes(b'candidate-error')
            self.assertIn('ordinary error disposition differs', compare.compare_row(reference, root)['issues'])
            (root / 'candidate.disposition.bin').unlink()
            self.assertEqual(compare.compare_row(reference, root)['status'], 'mismatch')

    def test_transitive_tool_paths_are_finite_and_present(self):
        self.assertEqual(len(compare.TOOLS), len(set(compare.TOOLS)))
        self.assertTrue(all((compare.ROOT / name).is_file() for name in compare.TOOLS))
        self.assertIn('tools/union_marker_fixtures.py', compare.TOOLS)
        self.assertIn('tools/collection_conversion_profiles.py', compare.TOOLS)

    def test_eighty_sessions_are_globally_ordered(self):
        sessions = [{'started_epoch': float(i), 'finished_epoch': float(i) + 0.5}
                    for i in range(80)]
        compare.verify_global_order(sessions)
        sessions[2]['started_epoch'] = 1.25
        with self.assertRaisesRegex(ValueError, 'ordered native module sessions overlap'):
            compare.verify_global_order(sessions)
        with self.assertRaisesRegex(ValueError, 'native process coverage differs'):
            compare.verify_global_order(sessions[:-1])

if __name__ == '__main__': unittest.main()
