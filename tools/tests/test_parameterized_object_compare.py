import copy
import json
from pathlib import Path
import sys
import unittest

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import parameterized_object_compare as compare
import parameterized_object_fixtures as fixtures


class NativeComparisonTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rows = json.loads((fixtures.DEFAULT / 'cases.json').read_text())

    def capture(self, row, disposition=None, constructed=True):
        expected = row['observations'][0]
        code = expected['exit_code']
        stdout = (fixtures.DEFAULT / expected['channels']['stdout']['path']).read_bytes()
        stderr = (fixtures.DEFAULT / expected['channels']['stderr']['path']).read_bytes()
        payload = stdout if code == 0 else stderr
        fields = {'profile': 'arbitrary-expression-noop-logger-power-off-v1',
                  'source_utf8_hex': (fixtures.DEFAULT / row['source']).read_bytes().hex(),
                  'source_uri_utf8_hex': row['source_uri'].encode().hex(),
                  'module_name_utf8_hex': row['module_name'].encode().hex(),
                  'expression_utf8_hex': row['expression'].encode().hex(),
                  'session_constructed': str(constructed).lower(),
                  'disposition': disposition if disposition is not None else 'language-error' if code == 1 else '',
                  'guest_exit_code': str(code)}
        if constructed:
            fields.update(terminal_slots='0', terminal_depth='0')
        channel = ('value_utf16be_hex' if code == 0 else 'error_utf16be_hex') if constructed else 'constructor_error_utf16be_hex'
        fields[channel] = payload.decode('utf-8').encode('utf-16-be').hex()
        trace = ''.join(key + '\t' + value + '\n' for key, value in fields.items()).encode()
        return code, stdout, stderr, trace

    def test_complete_raw_value_and_language_error_are_admitted(self):
        for code in [0, 1]:
            row = next(row for row in self.rows if row['observations'][0]['exit_code'] == code)
            result = compare.compare_native(row, fixtures.DEFAULT, *self.capture(row))
            self.assertEqual(result['status'], 'pass')

    def test_unsupported_cannot_pass_with_identical_error_bytes(self):
        row = next(row for row in self.rows if row['observations'][0]['exit_code'] == 1)
        for disposition in ['unsupported', 'limit-exceeded', 'internal-failure', '']:
            self.assertEqual(compare.compare_native(row, fixtures.DEFAULT, *self.capture(row, disposition))['status'], 'non-language-failure')

    def test_raw_whitespace_and_entire_channel_are_compared(self):
        row = self.rows[0]
        code, stdout, stderr, trace = self.capture(row)
        self.assertNotEqual(compare.compare_native(row, fixtures.DEFAULT, code, stdout + b'\n', stderr, trace)['status'], 'pass')

    def test_trace_source_identity_cannot_be_replaced(self):
        row = self.rows[0]
        code, stdout, stderr, trace = self.capture(row)
        trace = trace.replace(row['expression'].encode().hex().encode(), b'00')
        self.assertNotEqual(compare.compare_native(row, fixtures.DEFAULT, code, stdout, stderr, trace)['status'], 'pass')

    def test_missing_trace_never_passes_even_with_identical_bytes(self):
        row = self.rows[0]
        code, stdout, stderr, _ = self.capture(row)
        self.assertEqual(compare.compare_native(row, fixtures.DEFAULT, code, stdout, stderr, b'')['status'], 'acquisition-failed')

    def test_leaked_context_is_a_nonpass(self):
        row = self.rows[0]
        code, stdout, stderr, trace = self.capture(row)
        trace = trace.replace(b'terminal_depth\t0\n', b'terminal_depth\t1\n')
        self.assertNotEqual(compare.compare_native(row, fixtures.DEFAULT, code, stdout, stderr, trace)['status'], 'pass')

    def test_classified_constructor_error_has_explicit_no_session_path(self):
        row = next(row for row in self.rows if row['observations'][0]['exit_code'] == 1)
        self.assertEqual(compare.compare_native(row, fixtures.DEFAULT, *self.capture(row, constructed=False))['status'], 'pass')

    def test_no_session_constructor_shape_cannot_claim_a_success(self):
        row = self.rows[0]
        self.assertNotEqual(compare.compare_native(row, fixtures.DEFAULT, *self.capture(row, constructed=False))['status'], 'pass')

    def test_truncated_trace_is_not_complete_lifecycle_evidence(self):
        row = self.rows[0]
        code, stdout, stderr, trace = self.capture(row)
        self.assertEqual(compare.compare_native(row, fixtures.DEFAULT, code, stdout, stderr, trace[:-1])['status'], 'acquisition-failed')

    def test_timeout_never_passes_even_with_complete_matching_payload(self):
        row = self.rows[0]
        self.assertEqual(compare.compare_native(row, fixtures.DEFAULT, *self.capture(row), timed_out=True)['status'], 'candidate-timeout')

    def test_native_command_keeps_exact_expression_uri_and_resource_table(self):
        row = next(row for row in self.rows if row['requires_host'])
        command = compare.native_command(Path('/candidate'), row, fixtures.DEFAULT)
        self.assertEqual(command[2:5], [row['source_uri'], row['module_name'], row['expression']])
        self.assertEqual(command[5], row['resources'][0]['source_uri'])
        self.assertEqual(command[6], str(fixtures.DEFAULT / row['resources'][0]['source']))

    def test_power_assertion_override_rejected_before_native_admission(self):
        row = copy.deepcopy(self.rows[0])
        row['power_assertions'] = True
        with self.assertRaisesRegex(ValueError, 'power assertions'):
            compare.native_command(Path('/candidate'), row, fixtures.DEFAULT)

    def test_portable_report_cannot_override_failed_or_timed_out_process(self):
        for code, timed_out in [(1, False), (None, False), (0, True)]:
            report = dict(portable_report=dict(status='passed'), portable_process=dict(exit_code=code, timed_out=timed_out))
            self.assertFalse(compare.portable_process_passed(report))
        self.assertTrue(compare.portable_process_passed(dict(portable_report=dict(status='passed'), portable_process=dict(exit_code=0, timed_out=False))))


if __name__ == '__main__':
    unittest.main()
