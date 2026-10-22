"""Regression checks for complete raw logger evidence comparisons."""
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m2_trace


class TraceComparisonTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.expected = Path(self.temp.name) / 'java'
        self.actual = Path(self.temp.name) / 'rust'
        self.expected.mkdir()
        self.actual.mkdir()

    def pair(self, name, expected, actual=None):
        (self.expected / name).write_bytes(expected)
        (self.actual / name).write_bytes(expected if actual is None else actual)

    def test_raw_crlf_and_lone_surrogate_units_are_not_normalized(self):
        self.pair('log-0.source-0.bin', b'one\r\ntwo', b'one\ntwo')
        self.pair('log-0.message.code-units.bin', b'\xd8\x00', b'\xff\xfd')
        result = m2_trace.compare_payloads(self.expected, self.actual)
        self.assertTrue(result['exact_coverage'])
        self.assertFalse(result['raw_equal'])
        self.assertTrue(all(not row['raw_equal'] for row in result['comparisons']))

    def test_extra_or_missing_callback_cannot_pass(self):
        self.pair('outer-0.status.bin', b'success\n')
        (self.actual / 'log-callback-1.stdout.bin').write_bytes(b'1')
        result = m2_trace.compare_payloads(self.expected, self.actual)
        self.assertFalse(result['exact_coverage'])
        self.assertFalse(result['raw_equal'])
        self.assertIsNone(result['comparisons'][0]['expected_sha256'])

    def test_matching_oracle_internal_error_remains_distinct(self):
        self.pair('outer-0.status.bin', b'oracle-error\n')
        result = m2_trace.compare_payloads(self.expected, self.actual)
        self.assertTrue(result['raw_equal'])
        self.assertEqual(result['oracle_errors'], ['outer-0.status.bin'])

    def test_jvm_class_name_is_provenance_and_full_error_is_compared(self):
        self.pair('outer-0.error-class.bin', b'org.pkl.core.PklException', b'')
        self.pair('outer-0.stderr.bin', b'complete error\n', b'complete error')
        result = m2_trace.compare_payloads(self.expected, self.actual)
        self.assertEqual(len(result['comparisons']), 1)
        self.assertFalse(result['raw_equal'])

    def test_both_missing_calls_do_not_establish_coverage(self):
        self.pair('protocol.bin', b'logs=0 reads=0\n')
        self.assertEqual(m2_trace.missing_calls(self.expected, ['first', 'second']), [
            'outer-0.status.bin', 'outer-0.stderr.bin', 'outer-0.stdout.bin',
            'outer-1.status.bin', 'outer-1.stderr.bin', 'outer-1.stdout.bin'])

    def test_callback_status_requires_complete_payload(self):
        (self.expected / 'log-callback-0.status.bin').write_bytes(b'error\n')
        self.assertEqual(m2_trace.missing_calls(self.expected, []), [
            'log-callback-0.stderr.bin', 'log-callback-0.stdout.bin'])

    def test_checked_in_manifest_preserves_original_sources(self):
        manifest = m2_trace.load_manifest(m2_trace.ROOT / 'compatibility/m2-trace.json')
        self.assertEqual(len(manifest['cases']), 42)
        self.assertTrue({'edge-compact-limit', 'compile-grouped-contextual-new',
                         'compile-empty-trace-parse', 'read-same-pending-property'}
                        <= {case['id'] for case in manifest['cases']})


if __name__ == '__main__':
    unittest.main()
