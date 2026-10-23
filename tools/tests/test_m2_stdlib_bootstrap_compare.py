import tempfile
from pathlib import Path
import sys
import unittest
TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import m2_stdlib_bootstrap_compare as compare

class RawComparisonTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.actual = self.root / 'native'
        self.actual.mkdir()
        self.expected = {'prefix': 'outer-0', 'rawPrefix': 'java/outer-0', 'status': 'success\n', 'errorClass': ''}
        (self.root / 'java').mkdir()
        for prefix in [self.root / 'java/outer-0', self.actual / 'outer-0']:
            for channel, data in [('stdout.bin', b'value\n'), ('stderr.bin', b''), ('stdout.code-units.bin', 'value\n'.encode('utf-16-be')), ('stderr.code-units.bin', b''), ('status.bin', b'success\n')]:
                prefix.with_name(prefix.name + '.' + channel).write_bytes(data)
    def result(self, **kwargs):
        return compare.compare_call(self.root, self.expected, self.actual, **kwargs)
    def test_all_raw_channels_and_status_required(self):
        self.assertEqual(self.result()['status'], 'pass')
        (self.actual / 'outer-0.stdout.bin').write_bytes(b'value')
        self.assertEqual(self.result()['status'], 'mismatch')
        self.assertIn('stdout.bin', self.result()['differentChannels'])
    def test_equal_utf8_distinct_surrogate_units_cannot_pass(self):
        for prefix in [self.root / 'java/outer-0', self.actual / 'outer-0']:
            prefix.with_name(prefix.name + '.stdout.bin').write_bytes(b'?')
        (self.root / 'java/outer-0.stdout.code-units.bin').write_bytes(b'\xd8\x00')
        (self.actual / 'outer-0.stdout.code-units.bin').write_bytes(b'\xdc\x00')
        self.assertEqual(self.result()['status'], 'mismatch')
    def test_unsupported_never_becomes_ordinary_pass(self):
        (self.actual / 'outer-0.disposition.bin').write_bytes(b'unsupported')
        self.assertEqual(self.result()['status'], 'unsupported')
    def test_java_internal_error_remains_explicit_boundary(self):
        self.expected['errorClass'] = 'org.pkl.core.PklBugException'
        self.assertEqual(self.result()['status'], 'java-internal-error-boundary')
    def test_constructor_matching_text_does_not_prove_api_timing(self):
        (self.actual / 'constructor.status.bin').write_bytes(b'constructor-error\n')
        for channel in ['stdout.bin', 'stderr.bin', 'stdout.code-units.bin', 'stderr.code-units.bin']:
            (self.actual / ('constructor.' + channel)).write_bytes((self.actual / ('outer-0.' + channel)).read_bytes())
        result = self.result()
        self.assertEqual(result['status'], 'constructor-diagnostic-boundary')
        self.assertTrue(result['constructorTextMatch'])
    def test_independent_followup_never_proves_shared_evaluator(self):
        result = self.result(followup_boundary=True)
        self.assertEqual(result['status'], 'isolated-followup-control')
        self.assertTrue(result['rawMatch'])
        self.assertFalse(result['sameEvaluatorApiParityVerified'])
    def test_missing_completion_is_unexecuted(self):
        (self.actual / 'outer-0.status.bin').unlink()
        self.assertEqual(self.result()['status'], 'unexecuted')

if __name__ == '__main__':
    unittest.main()
