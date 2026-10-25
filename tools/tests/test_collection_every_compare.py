from __future__ import annotations
import importlib.util
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'collection_every_compare.py'
spec = importlib.util.spec_from_file_location('collection_every_compare', SCRIPT)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def utf16(value: str) -> bytes:
    return value.encode('utf-16-be', errors='surrogatepass')


def java_utf8(value: str) -> bytes:
    return value.encode('utf-8', errors='replace')


def put(root: Path, name: str, raw: bytes) -> None:
    (root / name).write_bytes(raw)


def put_pair(root: Path, name: str, value: str) -> None:
    put(root, name + '.bin', java_utf8(value))
    put(root, name + '.code-units.bin', utf16(value))


class LoggerComparison(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.java = self.root / 'java'
        self.candidate = self.root / 'candidate'
        for folder in (self.java, self.candidate):
            folder.mkdir()
        self.rows = [{'id': 'first', 'expression': 'List(1).every((x) -> trace(x) > 0)'}]
        self.make_row(0, 'first', self.rows[0]['expression'],
                      [{'level': 'trace', 'message': 'x = 🙂\ud800', 'module-uri': 'repl:text',
                        'member': 'null', 'position': '1:2-1:2'}])
        self.protocol()

    def make_row(self, index, id, expression, logs):
        for base in (self.java, self.candidate):
            folder = base / f'{index:03d}-{id}'
            folder.mkdir()
            put_pair(folder, 'expression', expression)
            put(folder, 'result.status.bin', b'success\n')
            put_pair(folder, 'result.stdout', 'true')
            put_pair(folder, 'result.stderr', '')
            put(folder, 'logs.count.bin', f'{len(logs)}\n'.encode())
            for log_index, record in enumerate(logs):
                for field in module.LOG_FIELDS:
                    put_pair(folder, f'log-{log_index:03d}.{field}', record[field])
            if base == self.java:
                put_pair(folder, 'mode', 'string')
                for name in ('result.error-class', 'result.exception-to-string',
                             'result.cause-class', 'result.cause-message',
                             'result.cause-to-string', 'result.java-type'):
                    put_pair(folder, name, '')
            else:
                for name in module.CANDIDATE_EVENT:
                    put(folder, name, b'')

    def protocol(self):
        lines = [f'session-open mode=string rows={len(self.rows)}']
        for i, row in enumerate(self.rows):
            lines.extend((f'call-enter=string-{i}-{row["id"]}',
                          f'call-exit=string-{i}-{row["id"]} status=success'))
        lines.append(f'session-closed mode=string rows={len(self.rows)} '
                     f'public-calls={len(self.rows)} success={len(self.rows)} errors=0')
        for base in (self.java, self.candidate):
            put(base, 'process.stdout.bin', ('\n'.join(lines) + '\n').encode())
            put(base, 'process.stderr.bin', b'')

    def compare(self):
        return module.compare_session(self.java, self.candidate, self.rows)

    def candidate_logs(self, records):
        folder = self.candidate / '000-first'
        for path in folder.glob('log-*'):
            path.unlink()
        put(folder, 'logs.count.bin', f'{len(records)}\n'.encode())
        for index, record in enumerate(records):
            for field in module.LOG_FIELDS:
                put_pair(folder, f'log-{index:03d}.{field}', record[field])

    def test_exact_unicode_and_empty_logger_rows(self):
        self.make_row(1, 'empty', 'List().every((x) -> false)', [])
        self.rows.append({'id': 'empty', 'expression': 'List().every((x) -> false)'})
        self.protocol()
        self.assertEqual(self.compare()['raw_matches'], 2)

    def test_unicode_code_unit_change_is_mismatch(self):
        put(self.candidate / '000-first', 'log-000.message.code-units.bin', utf16('x = 🙂?'))
        result = self.compare()
        self.assertEqual(result['raw_matches'], 0)
        self.assertEqual(result['rows'][0]['different_channels'], ['log-000.message.code-units.bin'])

    def test_missing_duplicate_or_extra_logger_fields_rejected(self):
        folder = self.candidate / '000-first'
        (folder / 'log-000.member.bin').unlink()
        with self.assertRaisesRegex(ValueError, 'logger channel'):
            self.compare()
        put_pair(folder, 'log-000.member', 'null')
        put_pair(folder, 'log-001.member', 'unexpected')
        with self.assertRaisesRegex(ValueError, 'logger channel'):
            self.compare()

    def test_reordered_log_events_are_raw_mismatch(self):
        record = {'level': 'trace', 'message': 'second', 'module-uri': 'repl:text',
                  'member': 'null', 'position': '1:3-1:3'}
        for base in (self.java, self.candidate):
            folder = base / '000-first'
            put(folder, 'logs.count.bin', b'2\n')
            for field in module.LOG_FIELDS:
                put_pair(folder, 'log-001.' + field, record[field])
        folder = self.candidate / '000-first'
        for field in module.LOG_FIELDS:
            for suffix in ('bin', 'code-units.bin'):
                one, two = folder / f'log-000.{field}.{suffix}', folder / f'log-001.{field}.{suffix}'
                first, second = one.read_bytes(), two.read_bytes()
                one.write_bytes(second); two.write_bytes(first)
        self.assertFalse(self.compare()['full_raw_match'])

    def test_count_status_and_process_protocol_rejected(self):
        folder = self.candidate / '000-first'
        put(folder, 'logs.count.bin', b'2\n')
        with self.assertRaisesRegex(ValueError, 'candidate logger channel'):
            self.compare()
        put(folder, 'logs.count.bin', b'1\n')
        put(folder, 'candidate.disposition.bin', b'language-error')
        with self.assertRaisesRegex(ValueError, 'status/disposition'):
            self.compare()
        put(folder, 'candidate.disposition.bin', b'')
        put(self.candidate, 'process.stdout.bin', b'wrong\n')
        with self.assertRaisesRegex(ValueError, 'protocol'):
            self.compare()

    def test_valid_candidate_status_difference_is_recorded(self):
        folder = self.candidate / '000-first'
        put(folder, 'result.status.bin', b'error\n')
        put(folder, 'candidate.disposition.bin', b'language-error')
        put(self.candidate, 'process.stdout.bin',
            b'session-open mode=string rows=1\n'
            b'call-enter=string-0-first\n'
            b'call-exit=string-0-first status=error\n'
            b'session-closed mode=string rows=1 public-calls=1 success=0 errors=1\n')
        result = self.compare()
        self.assertEqual(result['rows'][0]['different_channels'], ['result.status.bin'])
        self.assertEqual(result['different_process_channels'], ['stdout'])
        self.assertFalse(result['full_raw_match'])

    def test_missing_row_and_wrong_expression_rejected(self):
        (self.candidate / '000-first' / 'log-000.level.bin').unlink()
        with self.assertRaisesRegex(ValueError, 'logger channel'):
            self.compare()
        put(self.candidate / '000-first', 'log-000.level.bin', b'trace')
        self.rows[0]['expression'] = 'different()'
        with self.assertRaisesRegex(ValueError, 'Java expression'):
            self.compare()

    def test_complete_fewer_events_are_raw_mismatch(self):
        self.candidate_logs([])
        result = self.compare()['rows'][0]
        self.assertEqual(result['java_logs'], 1)
        self.assertEqual(result['candidate_logs'], 0)
        self.assertIn('logs.count.bin', result['different_channels'])
        self.assertIn('log-000.message.bin', result['different_channels'])
        self.assertFalse(result['raw_match'])

    def test_complete_extra_events_are_raw_mismatch(self):
        record = {'level': 'trace', 'message': 'extra', 'module-uri': 'repl:text',
                  'member': 'null', 'position': '1:3-1:3'}
        self.candidate_logs([
            {'level': 'trace', 'message': 'x = 🙂\ud800', 'module-uri': 'repl:text',
             'member': 'null', 'position': '1:2-1:2'}, record])
        result = self.compare()['rows'][0]
        self.assertEqual(result['java_logs'], 1)
        self.assertEqual(result['candidate_logs'], 2)
        self.assertIn('logs.count.bin', result['different_channels'])
        self.assertIn('log-001.position.code-units.bin', result['different_channels'])
        self.assertFalse(result['raw_match'])

    def test_native_and_portable_api_exception_metadata(self):
        folder = self.candidate / '000-first'
        for name in module.CANDIDATE_EVENT:
            (folder / name).unlink()
        put(folder, 'result.status.bin', b'api-exception\n')
        put(folder, 'candidate.api-error-kind.bin', b'Host')
        put(folder, 'candidate.callback-origin.bin', b'')
        result = module.compare_row(self.java / '000-first', folder)
        self.assertIn('result.status.bin', result['different_channels'])
        (folder / 'candidate.api-error-kind.bin').unlink()
        put(folder, 'candidate.api-error-name.bin', b'TypeError')
        module.compare_row(self.java / '000-first', folder)
        put(folder, 'candidate.api-error-kind.bin', b'Host')
        with self.assertRaisesRegex(ValueError, 'candidate row channel set'):
            module.compare_row(self.java / '000-first', folder)


if __name__ == '__main__':
    unittest.main()
