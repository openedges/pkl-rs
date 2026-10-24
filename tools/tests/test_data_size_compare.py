"""Regression checks for evidence tampering and incomplete native sessions."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import data_size_compare as compare


class RawComparisonTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.java = self.root / 'java'
        self.candidate = self.root / 'candidate'
        self.java.mkdir()
        self.candidate.mkdir()
        self.row(self.java, False)
        self.row(self.candidate, True)

    def row(self, folder, candidate, status=b'success\n', value=b'1.mb', error=b''):
        folder.mkdir(parents=True, exist_ok=True)
        payloads = {'expression.bin': b'1.mb', 'expression.code-units.bin': '1.mb'.encode('utf-16-be'),
                    'result.status.bin': status, 'result.stdout.bin': value,
                    'result.stdout.code-units.bin': value.decode().encode('utf-16-be'),
                    'result.stderr.bin': error, 'result.stderr.code-units.bin': error.decode().encode('utf-16-be')}
        if candidate:
            payloads.update({name: b'' for name in compare.EVENT_METADATA})
            if status == b'error\n':
                payloads['candidate.disposition.bin'] = b'language-error'
        else:
            payloads.update({name: b'' for name in compare.JAVA_ONLY})
        for name, data in payloads.items():
            (folder / name).write_bytes(data)

    def result(self):
        return compare.classify_row(self.java, self.candidate)

    def test_equal_common_bytes_ignore_only_recorded_java_producer_metadata(self):
        (self.java / 'result.error-class.bin').write_bytes(b'org.pkl.core.PklException')
        self.assertEqual(self.result()['status'], 'pass')
        (self.candidate / 'result.error-class.bin').write_bytes(b'org.pkl.core.PklException')
        self.assertEqual(self.result()['status'], 'mismatch')

    def test_trailing_whitespace_and_newline_are_not_normalized(self):
        for changed in [b'1.mb ', b'1.mb\n', b'1.mb\r\n']:
            with self.subTest(changed=changed):
                (self.candidate / 'result.stdout.bin').write_bytes(changed)
                self.assertEqual(self.result()['status'], 'mismatch')

    def test_equal_lossy_utf8_cannot_hide_distinct_utf16_surrogates(self):
        for folder in [self.java, self.candidate]:
            (folder / 'result.stdout.bin').write_bytes(b'?')
        (self.java / 'result.stdout.code-units.bin').write_bytes(b'\xd8\x00')
        (self.candidate / 'result.stdout.code-units.bin').write_bytes(b'\xd8\x01')
        self.assertEqual([d['channel'] for d in self.result()['differences']], ['result.stdout.code-units.bin'])

    def test_query_drift_is_not_hidden_by_equal_result(self):
        (self.candidate / 'expression.bin').write_bytes(b'1000.kb')
        self.assertEqual(self.result()['status'], 'mismatch')

    def test_missing_error_channel_is_not_empty_success(self):
        (self.candidate / 'result.stderr.bin').unlink()
        result = self.result()
        self.assertEqual(result['status'], 'mismatch')
        self.assertIn('result.stderr.bin', result['layout_issues'][0]['missing'])

    def test_unimplemented_error_cannot_pass_as_language_error(self):
        self.row(self.java, False, b'error\n', b'', b'error')
        self.row(self.candidate, True, b'candidate-error\n', b'', b'error')
        (self.candidate / 'candidate.disposition.bin').write_bytes(b'unsupported')
        self.assertEqual(self.result()['status'], 'mismatch')

    def test_ordinary_error_status_cannot_hide_unsupported_disposition(self):
        self.row(self.java, False, b'error\n', b'', b'error')
        self.row(self.candidate, True, b'error\n', b'', b'error')
        (self.candidate / 'candidate.disposition.bin').write_bytes(b'unsupported')
        result = self.result()
        self.assertEqual(result['status'], 'mismatch')
        self.assertEqual(result['differences'], [])

    def test_extra_channel_is_not_silently_ignored(self):
        (self.candidate / 'candidate.expected-result.bin').write_bytes(b'1.mb')
        self.assertEqual(self.result()['status'], 'mismatch')

    def test_error_disposition_cannot_be_promoted_to_success(self):
        (self.candidate / 'candidate.disposition.bin').write_bytes(b'unsupported')
        self.assertEqual(self.result()['status'], 'mismatch')

    def session(self, swapped=False):
        cases = {'rows': [{'id': 'first'}, {'id': 'second'}]}
        processes = []
        for repeat in range(2):
            java = self.java / 'string' / f'repeat-{repeat}'
            native = self.candidate / 'native' / f'repeat-{repeat}'
            java.mkdir(parents=True)
            native.mkdir(parents=True)
            for index, name in enumerate(['first', 'second']):
                status = b'error\n' if index == 0 else b'success\n'
                self.row(java / f'{index:03}-{name}', False, status,
                         b'' if index == 0 else b'1.mb', b'error' if index == 0 else b'')
                other = 1 - index if swapped else index
                self.row(native / f'{index:03}-{name}', True,
                         b'error\n' if other == 0 else b'success\n',
                         b'' if other == 0 else b'1.mb', b'error' if other == 0 else b'')
            # Deliberately equal aggregate footer: per-row checks must still reject swapped statuses.
            for directory in (java, native):
                (directory / 'process.stdout.bin').write_bytes(b'success=1 errors=1\n')
                (directory / 'process.stderr.bin').write_bytes(b'')
            (native / 'candidate-trace.tsv').write_bytes(b'terminal-heap-slots\t0\nterminal-context-depth\t0\n')
            processes.append({'repeat': repeat, 'exit_code': 0, 'timed_out': False, 'launch_error': None,
                              'stdout': compare.identity(native / 'process.stdout.bin'),
                              'stderr': compare.identity(native / 'process.stderr.bin')})
        return cases, processes

    def test_balanced_status_swaps_fail_even_when_footer_counts_match(self):
        cases, processes = self.session(swapped=True)
        result = compare.compare_sessions(self.java, self.candidate, cases, processes)
        self.assertEqual(result['counts'], {'mismatch': 4})
        self.assertFalse(result['strict_string_passed'])

    def test_protocol_change_fails_even_when_all_rows_match(self):
        cases, processes = self.session()
        path = self.candidate / 'native/repeat-0/process.stdout.bin'
        path.write_bytes(b'extra-call\nsuccess=1 errors=1\n')
        processes[0]['stdout'] = compare.identity(path)
        result = compare.compare_sessions(self.java, self.candidate, cases, processes)
        self.assertEqual(result['counts'], {'pass': 4})
        self.assertFalse(result['strict_string_passed'])

    def test_missing_or_duplicate_repetition_is_rejected(self):
        cases, processes = self.session()
        for broken in [processes[:1], [processes[0], processes[0]],
                       [{**processes[0], 'repeat': False}, {**processes[1], 'repeat': True}]]:
            with self.subTest(count=len(broken)), self.assertRaises(ValueError):
                compare.compare_sessions(self.java, self.candidate, cases, broken)

    def test_nonzero_exit_is_not_success_despite_complete_payloads(self):
        cases, processes = self.session()
        processes[0]['exit_code'] = 2
        self.assertFalse(compare.compare_sessions(self.java, self.candidate, cases, processes)['strict_string_passed'])

    def test_terminal_leak_is_not_hidden_by_matching_payloads(self):
        cases, processes = self.session()
        (self.candidate / 'native/repeat-0/candidate-trace.tsv').write_bytes(b'terminal-heap-slots\t1\nterminal-context-depth\t0\n')
        self.assertFalse(compare.compare_sessions(self.java, self.candidate, cases, processes)['strict_string_passed'])

    def test_build_input_closure_requires_embedded_stdlib(self):
        build, sources = self.root / 'build', self.root / 'sources'
        build.mkdir()
        paths = {'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'crates/pkl-runtime/Cargo.toml',
                 'crates/pkl-runtime/examples/data_size_fixture_probe.rs',
                 'crates/pkl-runtime/src/stdlib/base.pkl', 'crates/pkl-runtime/src/stdlib/ref.pkl',
                 'crates/pkl-runtime/src/stdlib/json.pkl', 'crates/pkl-runtime/src/stdlib/test.pkl',
                 'crates/pkl-runtime/src/stdlib/modules.txt'}
        records = {}
        for name in paths:
            path = sources / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b'fixed bytes')
            records[name] = compare.identity(path)
        self.assertTrue(compare.required_build_inputs(sources).issubset(records))
        del records['crates/pkl-runtime/src/stdlib/base.pkl']
        for name in ['source-before.json', 'source-after.json']:
            (build / name).write_text(json.dumps(records))
        with self.assertRaisesRegex(ValueError, 'closure incomplete'):
            compare.verify_build(build, sources)
        # Removing both the physical input and its record must not shrink the contract.
        (sources / 'crates/pkl-runtime/src/stdlib/base.pkl').unlink()
        with self.assertRaisesRegex(ValueError, 'closure incomplete'):
            compare.verify_build(build, sources)

    def test_reused_process_and_overlapping_sessions_are_rejected(self):
        processes = [{'repeat': 0, 'pid': 100, 'launch_error': None, 'started_epoch': 1, 'finished_epoch': 2},
                     {'repeat': 1, 'pid': 101, 'launch_error': None, 'started_epoch': 3, 'finished_epoch': 4}]
        compare.verify_process_boundaries(processes)
        with self.assertRaisesRegex(ValueError, 'not distinct'):
            compare.verify_process_boundaries([processes[0], {**processes[1], 'pid': 100}])
        with self.assertRaisesRegex(ValueError, 'overlap'):
            compare.verify_process_boundaries([processes[0], {**processes[1], 'started_epoch': 1.5}])


if __name__ == '__main__':
    unittest.main()
