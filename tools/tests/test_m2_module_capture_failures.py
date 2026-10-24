"""Failed original-command acquisitions preserve evidence and restore inputs."""
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compat
import m2_module_eval as module


class CaptureFailureTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.workspace = self.root / 'workspace'
        self.workspace.mkdir()
        self.source = b'value = 1\n'
        for base in (self.root, self.workspace):
            (base / 'input.pkl').write_bytes(self.source)
        self.case = dict(id='original:failure', input='input.pkl', input_sha256=compat.digest(self.source),
                         upstream_case=dict(id='original:failure', command=['eval', 'input.pkl'], cwd='.',
                                            environment={'LANG': 'C.UTF-8', 'NO_COLOR': '1'}, resources=[]))
        self.environment = {'PATH': os.defpath, 'LANG': 'C', 'TZ': 'UTC', 'UNRECORDED': 'not logged'}
        self.directory = self.root / 'evidence'
        self.before = compat.snapshot(self.workspace)

    def capture(self):
        return module.capture_process(self.root, self.workspace, self.case, ['missing-runner'],
                                      self.environment, self.directory)

    def mutate(self, command, cwd, environment, timeout, generated):
        (cwd / 'input.pkl').write_bytes(b'mutated\xff\r\n')
        (cwd / 'new-file.bin').write_bytes(b'new\x00bytes')
        (generated / 'output.bin').write_bytes(b'generated\xff')
        (Path(environment['HOME']) / 'cache.bin').write_bytes(b'private\x00')

    def launch_failure(self):
        error = OSError(5, 'synthetic launch failure')
        error.stdout = b'partial stdout\xff\r\n'
        error.stderr = b'partial stderr\x00'

        def fail(*arguments):
            self.mutate(*arguments)
            raise error

        return error, fail

    def failure_record(self):
        self.assertFalse((self.directory / 'process.json').exists())
        self.assertFalse((self.directory / 'result.json').exists())
        record = json.loads((self.directory / 'acquisition-failure.json').read_bytes())
        self.assertEqual(record['status'], 'acquisition-failure')
        self.assertEqual(record['command'], ['missing-runner', 'eval', 'input.pkl'])
        self.assertEqual(record['cwd'], str(self.workspace.resolve()))
        self.assertEqual(record['environment']['LANG'], 'C.UTF-8')
        self.assertNotIn('UNRECORDED', record['environment'])
        self.assertEqual(record['upstream_case_sha256'], compat.digest(compat.encoded(self.case['upstream_case'])))
        for channel, entry in record['raw_channels'].items():
            if entry is not None:
                self.assertEqual(compat.digest((self.directory / entry['path']).read_bytes()), entry['sha256'])
        for name, digest in record['files_sha256'].items():
            self.assertEqual(compat.digest((self.directory / 'failure-files' / name).read_bytes()), digest)
        return record

    def test_launch_exception_restores_inputs_and_retains_raw_failure(self):
        error, fail = self.launch_failure()
        with patch.object(compat, 'capture', side_effect=fail), self.assertRaises(OSError) as caught:
            self.capture()
        self.assertIs(caught.exception, error)
        self.assertEqual(compat.snapshot(self.workspace), self.before)
        record = self.failure_record()
        self.assertTrue(record['workspace_restored'])
        self.assertIsNone(record['restoration_error'])
        self.assertEqual(record['capture_error']['message'], str(error))
        self.assertIn('synthetic launch failure', record['capture_error']['traceback'])
        self.assertFalse(record['capture_result_available'])
        self.assertEqual((self.directory / 'failure-stdout.bin').read_bytes(), error.stdout)
        self.assertEqual((self.directory / 'failure-stderr.bin').read_bytes(), error.stderr)
        self.assertEqual((self.directory / 'failure-files/workspace/input.pkl').read_bytes(), b'mutated\xff\r\n')
        self.assertEqual((self.directory / 'failure-files/workspace/new-file.bin').read_bytes(), b'new\x00bytes')
        self.assertEqual((self.directory / 'failure-files/generated/output.bin').read_bytes(), b'generated\xff')
        self.assertEqual((self.directory / 'failure-files/private-state/home/cache.bin').read_bytes(), b'private\x00')

    def test_cleanup_failure_does_not_mask_launch_error_or_erase_changed_bytes(self):
        error, fail = self.launch_failure()
        cleanup_error = OSError(13, 'synthetic restoration failure')

        def fail_cleanup(*arguments):
            (self.workspace / 'input.pkl').unlink()
            raise cleanup_error

        with patch.object(compat, 'capture', side_effect=fail), \
                patch.object(compat, 'restore_changes', side_effect=fail_cleanup), \
                self.assertRaises(OSError) as caught:
            self.capture()
        self.assertIs(caught.exception, error)
        record = self.failure_record()
        self.assertFalse(record['workspace_restored'])
        self.assertEqual(record['capture_error']['message'], str(error))
        self.assertEqual(record['restoration_error']['message'], str(cleanup_error))
        self.assertIn('synthetic restoration failure', '\n'.join(error.__notes__))
        self.assertFalse((self.workspace / 'input.pkl').exists())
        self.assertEqual((self.directory / 'failure-files/workspace/input.pkl').read_bytes(), b'mutated\xff\r\n')

    def test_restoration_failure_after_process_exit_retains_complete_process_bytes(self):
        result = dict(stdout=b'stdout\xff', stderr=b'stderr\x00', exit_code=7, timed_out=False,
                      elapsed_ns=1, files={'output.bin': b'generated\xff'})

        def complete(*arguments):
            self.mutate(*arguments)
            return result

        cleanup_error = ValueError('synthetic cleanup error')
        with patch.object(compat, 'capture', side_effect=complete), \
                patch.object(compat, 'restore_changes', side_effect=cleanup_error), \
                self.assertRaises(ValueError) as caught:
            self.capture()
        self.assertIs(caught.exception, cleanup_error)
        record = self.failure_record()
        self.assertIsNone(record['capture_error'])
        self.assertTrue(record['capture_result_available'])
        self.assertEqual(record['process'], dict(exit_code=7, timed_out=False, elapsed_ns=1))
        self.assertEqual((self.directory / 'failure-stdout.bin').read_bytes(), result['stdout'])
        self.assertEqual((self.directory / 'failure-stderr.bin').read_bytes(), result['stderr'])

    def test_normal_and_timeout_observations_keep_existing_capture_contract(self):
        for timed_out in (False, True):
            with self.subTest(timed_out=timed_out):
                self.directory = self.root / f'evidence-{timed_out}'
                result = dict(stdout=b'stdout\r\n', stderr=b'stderr\xff', exit_code=-9 if timed_out else 1,
                              timed_out=timed_out, elapsed_ns=12, files={'output.bin': b'generated\xff'})

                def complete(*arguments):
                    self.mutate(*arguments)
                    if timed_out:
                        raise compat.CaptureTimeout(60, result)
                    return result

                with patch.object(compat, 'capture', side_effect=complete):
                    process, captured = self.capture()
                self.assertEqual(compat.snapshot(self.workspace), self.before)
                self.assertEqual(process['timed_out'], timed_out)
                self.assertEqual(process['exit_code'], -9 if timed_out else 1)
                self.assertEqual(captured['stdout'], b'stdout\r\n')
                self.assertEqual(captured['stderr'], b'stderr\xff')
                self.assertEqual(captured['files']['workspace/input.pkl'], b'mutated\xff\r\n')
                self.assertEqual(captured['files']['generated/output.bin'], b'generated\xff')
                self.assertEqual(compat.differences(captured, compat.load_capture(self.directory)), [])
                self.assertFalse((self.directory / 'acquisition-failure.json').exists())

    def test_failure_record_reuse_is_rejected_before_private_state_is_reset(self):
        error, fail = self.launch_failure()
        with patch.object(compat, 'capture', side_effect=fail), self.assertRaises(OSError):
            self.capture()
        saved = (self.directory / 'acquisition-failure.json').read_bytes()
        with patch.object(compat, 'case_execution') as execution, self.assertRaisesRegex(ValueError, 'new output directory'):
            self.capture()
        execution.assert_not_called()
        self.assertEqual((self.directory / 'acquisition-failure.json').read_bytes(), saved)

    def test_real_unavailable_runner_keeps_exception_cause_without_invented_channels(self):
        with self.assertRaisesRegex(RuntimeError, 'Runner unavailable') as caught:
            self.capture()
        self.assertIsInstance(caught.exception.__cause__, OSError)
        self.assertEqual(compat.snapshot(self.workspace), self.before)
        record = self.failure_record()
        self.assertIn('FileNotFoundError', record['capture_error']['traceback'])
        self.assertEqual(record['raw_channels'], dict(stdout=None, stderr=None))
        self.assertFalse(record['capture_result_available'])


if __name__ == '__main__':
    unittest.main()
