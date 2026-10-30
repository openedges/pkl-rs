import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

test_directory = Path(__file__).resolve().parent
module_directory = test_directory if (test_directory/'workspace_storage.py').is_file() else test_directory.parent
sys.path.insert(0, str(module_directory))
import workspace_storage as storage


def fake_metadata(path):
    s = path.stat()
    return {'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'bytes': s.st_size,
            'mode': s.st_mode & 0o7777, 'uid': s.st_uid, 'gid': s.st_gid,
            'inode': s.st_ino, 'nlink': s.st_nlink, 'blocks': s.st_blocks,
            'flags': 0x20 if '.pkl-storage-' in path.name else 0, 'xattrs': {}, 'acl_sha256': 'same-acl'}


def fake_copy(source, destination, timeout, target, floor):
    shutil.copy2(source, destination)
    return {'exit_code': 0}


class WorkspaceStorageTests(unittest.TestCase):
    def test_storage_floor_validation_and_default(self):
        self.assertEqual(storage.storage_floor({}), 1024*1024**2)
        self.assertEqual(storage.storage_floor({'PKL_STORAGE_MIN_FREE_MIB': '768'}), 768*1024**2)
        for value in ['', '511', '0', '-1', '65537', '1.5', ' 768', '９００']:
            with self.subTest(value=value), self.assertRaises(ValueError):
                storage.storage_floor({'PKL_STORAGE_MIN_FREE_MIB': value})

    def test_override_floor_reaches_copy_and_receipt(self):
        class Child:
            stdout = io.BufferedReader(io.BytesIO())
            stderr = io.BufferedReader(io.BytesIO(
                b'{"$message_type":"artifact","artifact":"/unused/test","emit":"link"}\n'))
            def wait(self): return 0
        class Stream:
            def __init__(self): self.buffer = io.BytesIO()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); target = root/'target'; target.mkdir(); receipts = root/'receipts'; receipts.mkdir()
            env = {'PKL_STORAGE_TARGET': str(target), 'PKL_STORAGE_RECEIPTS': str(receipts),
                   'PKL_STORAGE_MIN_FREE_MIB': '768'}
            with patch.object(storage.sys, 'stdout', Stream()), patch.object(storage.sys, 'stderr', Stream()), \
                 patch.object(storage.shutil, 'which', return_value=sys.executable), \
                 patch.object(storage, 'digest', side_effect=lambda path: hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else 'mock-tool'), \
                 patch.object(storage.subprocess, 'Popen', return_value=Child()), \
                 patch.object(storage, 'compress_product', return_value={'action': 'compressed'}) as product:
                self.assertEqual(storage.run(['wrapper', 'rustc', '--test', '--emit=link'], env), 0)
                product.assert_called_once_with('/unused/test', target, 120, 768*1024**2)
            receipt = json.loads(next(receipts.glob('*.json')).read_bytes())
            self.assertEqual(receipt['minimum_free_bytes'], 768*1024**2)

    def test_override_floor_propagates_through_real_copy_decision(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory).resolve(); binary = target/'test'; binary.write_bytes(b'bytes'); binary.chmod(0o700)
            floor = 768*1024**2
            with patch.object(storage, 'metadata', side_effect=fake_metadata), \
                 patch.object(storage, 'macho_executable', return_value=True), \
                 patch.object(storage, 'codesign', return_value={'exit_code': 0}), \
                 patch.object(storage.shutil, 'disk_usage', return_value=types.SimpleNamespace(free=floor+10000)), \
                 patch.object(storage, 'copy_compressed', side_effect=fake_copy) as copy:
                self.assertEqual(storage.compress_product(binary, target, 5, floor)['action'], 'compressed')
                self.assertEqual(copy.call_args.args[-1], floor)

    def test_artifact_notifications_are_exact_and_deduplicated(self):
        data = (b'not json\n'
                b'{"$message_type":"artifact","artifact":"/tmp/a","emit":"link"}\n'
                b'{"$message_type":"artifact","artifact":"/tmp/a","emit":"link"}\n'
                b'{"$message_type":"artifact","artifact":"/tmp/b","emit":"metadata"}\n')
        self.assertEqual(storage.artifact_lines(data), ['/tmp/a'])
        with self.assertRaises(ValueError):
            storage.artifact_lines(b'{"$message_type":"artifact","emit":"link"}\n')

    def test_macho_header_identifies_only_executables(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'binary'
            for filetype, expected in [(2, True), (6, False)]:
                path.write_bytes(b'\xcf\xfa\xed\xfe'+b'\0'*8+struct.pack('<I', filetype))
                self.assertEqual(storage.macho_executable(path), expected)
            path.write_bytes(b'!<arch>\n')
            self.assertFalse(storage.macho_executable(path))

    def test_test_link_classification_excludes_probes(self):
        self.assertTrue(storage.needs_executable_link(['--test', '--emit=dep-info,link']))
        self.assertTrue(storage.needs_executable_link(['--test', '--emit', 'dep-info,link']))
        self.assertTrue(storage.needs_executable_link(['--test']))
        self.assertTrue(storage.needs_executable_link(['--crate-type', 'bin', '--emit=link']))
        self.assertTrue(storage.needs_executable_link(['--crate-type=bin,rlib', '--emit=link']))
        self.assertFalse(storage.needs_executable_link(['--crate-type=lib', '--emit=link']))
        self.assertFalse(storage.needs_executable_link(['--test', '--emit=dep-info,metadata']))
        self.assertFalse(storage.needs_executable_link(['--test', '--print=cfg']))
        self.assertFalse(storage.needs_executable_link(['--crate-type=bin', '-vV']))

    def test_atomic_replace_keeps_content_and_rejects_changed_copy(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory).resolve()/'target'; target.mkdir()
            binary = target/'test-executable'; binary.write_bytes(b'original'); binary.chmod(0o700)
            disk = types.SimpleNamespace(free=storage.FLOOR+100000)
            with patch.object(storage, 'metadata', side_effect=fake_metadata), \
                 patch.object(storage, 'macho_executable', return_value=True), \
                 patch.object(storage, 'codesign', return_value={'exit_code': 0}), \
                 patch.object(storage.shutil, 'disk_usage', return_value=disk), \
                 patch.object(storage, 'copy_compressed', side_effect=fake_copy):
                result = storage.compress_product(binary, target, 5)
                self.assertEqual(result['action'], 'compressed')
                self.assertEqual(binary.read_bytes(), b'original')
                self.assertEqual(binary.stat().st_mode & 0o7777, 0o700)
                def corrupt(source, destination, timeout, root, floor):
                    fake_copy(source, destination, timeout, root, floor)
                    destination.write_bytes(b'changed')
                    return {'exit_code': 0}
                with patch.object(storage, 'copy_compressed', side_effect=corrupt):
                    with self.assertRaises(ValueError):
                        storage.compress_product(binary, target, 5)
                self.assertEqual(binary.read_bytes(), b'original')
                self.assertEqual([p.name for p in target.iterdir()], ['test-executable'])

    def test_refuses_outside_target_symlinks_and_hardlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); target = root/'target'; target.mkdir()
            other = root/'other'; other.write_bytes(b'bytes')
            with self.assertRaises(ValueError):
                storage.compress_product(other, target, 5)
            hard = target/'hard'; os.link(other, hard)
            with self.assertRaises(ValueError):
                storage.regular(hard)
            link = target/'link'; link.symlink_to(other)
            with self.assertRaises(ValueError):
                storage.regular(link)

    def test_exclusions_are_rejected_before_any_metadata(self):
        for name in ['stage65-b-native-regressions-evidence.tar.gz.partial', 'stage86-anything', 'private8-data']:
            with self.subTest(name=name), patch.object(Path, 'lstat', side_effect=AssertionError('metadata accessed')):
                with self.assertRaisesRegex(ValueError, 'excluded path'):
                    storage.plain_path(Path('/tmp')/name/'file')

    def test_xattr_hashes_binary_values_and_fails_closed(self):
        def result(command, **kwargs):
            if len(command) == 2:
                return types.SimpleNamespace(stdout=b'com.apple.decmpfs\ncom.apple.provenance\n')
            self.assertEqual(command[1:3], ['-px', 'com.apple.provenance'])
            return types.SimpleNamespace(stdout=b'00 FF\n 10 20\n')
        with patch.object(storage.subprocess, 'run', side_effect=result):
            self.assertEqual(storage.xattrs(Path('/tmp/file')),
                             {'com.apple.provenance': hashlib.sha256(b'\x00\xff\x10\x20').hexdigest()})
        for error in [subprocess.CalledProcessError(1, ['xattr']), ValueError('invalid hex')]:
            def bad(command, **kwargs):
                if len(command) == 2:
                    return types.SimpleNamespace(stdout=b'com.apple.provenance\n')
                if isinstance(error, subprocess.CalledProcessError):
                    raise error
                return types.SimpleNamespace(stdout=b'not hex')
            with patch.object(storage.subprocess, 'run', side_effect=bad):
                with self.assertRaises((subprocess.CalledProcessError, ValueError)):
                    storage.xattrs(Path('/tmp/file'))

    def test_non_executable_is_not_compressed(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory).resolve(); artifact = target/'library.rlib'; artifact.write_bytes(b'archive')
            with patch.object(storage, 'metadata', side_effect=fake_metadata), \
                 patch.object(storage, 'codesign') as sign:
                result = storage.compress_product(artifact, target, 5)
                self.assertEqual(result['action'], 'skip-non-executable')
                sign.assert_not_called()

    def test_floor_failure_preserves_original(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory).resolve(); artifact = target/'test'; artifact.write_bytes(b'bytes')
            with patch.object(storage, 'metadata', side_effect=fake_metadata), \
                 patch.object(storage, 'macho_executable', return_value=True), \
                 patch.object(storage, 'codesign', return_value={'exit_code': 0}), \
                 patch.object(storage.shutil, 'disk_usage', return_value=types.SimpleNamespace(free=storage.FLOOR)):
                with self.assertRaises(OSError): storage.compress_product(artifact, target, 5)
            self.assertEqual(artifact.read_bytes(), b'bytes')

    def test_compiler_output_forwarding_and_failure_receipts(self):
        class FakeChild:
            def __init__(self, code, stdout, stderr):
                self.returncode = code
                self.stdout = io.BufferedReader(io.BytesIO(stdout))
                self.stderr = io.BufferedReader(io.BytesIO(stderr))
            def wait(self): return self.returncode
        class Stream:
            def __init__(self): self.buffer = io.BytesIO()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); target = root/'target'; target.mkdir(); receipts = root/'receipts'; receipts.mkdir()
            env = {'PKL_STORAGE_TARGET': str(target), 'PKL_STORAGE_RECEIPTS': str(receipts)}
            link = str(target/'some-test')
            diagnostic = b'warning bytes\n'+json.dumps({'$message_type':'artifact','artifact':link,'emit':'link'}).encode()+b'\n'
            out, err = Stream(), Stream()
            with patch.object(storage.sys, 'stdout', out), patch.object(storage.sys, 'stderr', err), \
                 patch.object(storage.shutil, 'which', return_value=sys.executable), \
                 patch.object(storage, 'digest', side_effect=lambda path: hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else 'mock-tool'), \
                 patch.object(storage.subprocess, 'Popen', return_value=FakeChild(0, b'raw\x00out', diagnostic)), \
                 patch.object(storage, 'compress_product', return_value={'action':'compressed'}) as compress:
                self.assertEqual(storage.run(['wrapper', 'rustc', '--test', '--emit=link'], env), 0)
                compress.assert_called_once_with(link, target, 120, storage.FLOOR)
            self.assertEqual(out.buffer.getvalue(), b'raw\x00out')
            self.assertEqual(err.buffer.getvalue(), diagnostic)
            first = json.loads(next(receipts.glob('*.json')).read_bytes())
            self.assertEqual(first['compiler_exit_code'], 0)
            self.assertEqual(first['stderr']['sha256'], hashlib.sha256(diagnostic).hexdigest())
            with patch.object(storage.sys, 'stdout', Stream()), patch.object(storage.sys, 'stderr', Stream()), \
                 patch.object(storage.shutil, 'which', return_value=sys.executable), \
                 patch.object(storage, 'digest', side_effect=lambda path: hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else 'mock-tool'), \
                 patch.object(storage.subprocess, 'Popen', return_value=FakeChild(23, b'', b'failure')), \
                 patch.object(storage, 'compress_product') as compress:
                self.assertEqual(storage.run(['wrapper', 'rustc', '--test', '--emit=link'], env), 23)
                compress.assert_not_called()
            self.assertEqual(len(list(receipts.glob('*.json'))), 2)

    def test_missing_test_artifact_fails_closed(self):
        class Child:
            stdout = io.BufferedReader(io.BytesIO()); stderr = io.BufferedReader(io.BytesIO())
            def wait(self): return 0
        class Stream:
            def __init__(self): self.buffer = io.BytesIO()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); target = root/'target'; target.mkdir(); receipts = root/'receipts'; receipts.mkdir()
            env = {'PKL_STORAGE_TARGET': str(target), 'PKL_STORAGE_RECEIPTS': str(receipts)}
            with patch.object(storage.sys, 'stdout', Stream()), patch.object(storage.sys, 'stderr', Stream()), \
                 patch.object(storage.shutil, 'which', return_value=sys.executable), \
                 patch.object(storage, 'digest', side_effect=lambda path: hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else 'mock-tool'), \
                 patch.object(storage.subprocess, 'Popen', return_value=Child()):
                self.assertEqual(storage.run(['wrapper', 'rustc', '--test', '--emit=link'], env), 1)
            receipt = json.loads(next(receipts.glob('*.json')).read_bytes())
            self.assertIn('lacked rustc JSON artifact', receipt['errors'][0])

    def test_compiler_probe_succeeds_without_artifact_notifications(self):
        class Child:
            stdout = io.BufferedReader(io.BytesIO(b'x86_64\n'))
            stderr = io.BufferedReader(io.BytesIO())
            def wait(self): return 0
        class Stream:
            def __init__(self): self.buffer = io.BytesIO()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); target = root/'target'; target.mkdir(); receipts = root/'receipts'; receipts.mkdir()
            env = {'PKL_STORAGE_TARGET': str(target), 'PKL_STORAGE_RECEIPTS': str(receipts)}
            with patch.object(storage.sys, 'stdout', Stream()), patch.object(storage.sys, 'stderr', Stream()), \
                 patch.object(storage.shutil, 'which', return_value=sys.executable), \
                 patch.object(storage, 'digest', side_effect=lambda path: hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else 'mock-tool'), \
                 patch.object(storage.subprocess, 'Popen', return_value=Child()), \
                 patch.object(storage, 'compress_product') as compress:
                self.assertEqual(storage.run(['wrapper', 'rustc', '--print=cfg'], env), 0)
                compress.assert_not_called()

    def test_pump_failure_retains_known_exit_and_partial_raw_stream_pins(self):
        class Child:
            def __init__(self):
                self.stdout = io.BufferedReader(io.BytesIO(b'compiler stdout'))
                self.stderr = io.BufferedReader(io.BytesIO(b'compiler stderr'))
            def wait(self): return 0
            def poll(self): return 0
        class BrokenStream:
            @property
            def buffer(self): return self
            def write(self, data): raise BrokenPipeError('consumer gone')
            def flush(self): pass
        class Stream:
            def __init__(self): self.buffer = io.BytesIO()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); target = root/'target'; target.mkdir(); receipts = root/'receipts'; receipts.mkdir()
            env = {'PKL_STORAGE_TARGET': str(target), 'PKL_STORAGE_RECEIPTS': str(receipts)}
            with patch.object(storage.sys, 'stdout', BrokenStream()), patch.object(storage.sys, 'stderr', Stream()), \
                 patch.object(storage.shutil, 'which', return_value=sys.executable), \
                 patch.object(storage, 'digest', side_effect=lambda path: hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else 'mock-tool'), \
                 patch.object(storage.subprocess, 'Popen', return_value=Child()):
                self.assertEqual(storage.run(['wrapper', 'rustc', '--print=cfg'], env), 1)
            receipt = json.loads(next(receipts.glob('*.json')).read_bytes())
            self.assertEqual(receipt['compiler_exit_code'], 0)
            self.assertFalse(receipt['complete'])
            self.assertIn('stream forwarding failed', receipt['errors'][0])
            for name, expected in [('stdout', b'compiler stdout'), ('stderr', b'compiler stderr')]:
                self.assertEqual(Path(receipt[name]['path']).read_bytes(), expected)
                self.assertEqual(receipt[name]['sha256'], hashlib.sha256(expected).hexdigest())

    def test_failed_compiler_start_receipts_empty_created_streams(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); target = root/'target'; target.mkdir(); receipts = root/'receipts'; receipts.mkdir()
            env = {'PKL_STORAGE_TARGET': str(target), 'PKL_STORAGE_RECEIPTS': str(receipts)}
            with patch.object(storage.shutil, 'which', return_value=sys.executable), \
                 patch.object(storage, 'digest', side_effect=lambda path: hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else 'mock-tool'), \
                 patch.object(storage.subprocess, 'Popen', side_effect=OSError('start failed')):
                self.assertEqual(storage.run(['wrapper', 'rustc', '--print=cfg'], env), 1)
            receipt = json.loads(next(receipts.glob('*.json')).read_bytes())
            self.assertIsNone(receipt['compiler_exit_code'])
            for name in ['stdout', 'stderr']:
                self.assertEqual(receipt[name]['bytes'], 0)
                self.assertEqual(receipt[name]['sha256'], hashlib.sha256(b'').hexdigest())

    def test_binary_link_without_notification_fails_closed(self):
        class Child:
            stdout = io.BufferedReader(io.BytesIO()); stderr = io.BufferedReader(io.BytesIO())
            def wait(self): return 0
        class Stream:
            def __init__(self): self.buffer = io.BytesIO()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); target = root/'target'; target.mkdir(); receipts = root/'receipts'; receipts.mkdir()
            env = {'PKL_STORAGE_TARGET': str(target), 'PKL_STORAGE_RECEIPTS': str(receipts)}
            with patch.object(storage.sys, 'stdout', Stream()), patch.object(storage.sys, 'stderr', Stream()), \
                 patch.object(storage.shutil, 'which', return_value=sys.executable), \
                 patch.object(storage, 'digest', side_effect=lambda path: hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else 'mock-tool'), \
                 patch.object(storage.subprocess, 'Popen', return_value=Child()):
                self.assertEqual(storage.run(['wrapper', 'rustc', '--crate-type=bin', '--emit=link'], env), 1)
            receipt = json.loads(next(receipts.glob('*.json')).read_bytes())
            self.assertEqual(receipt['compiler_exit_code'], 0)
            self.assertIn('lacked rustc JSON artifact', receipt['errors'][0])


if __name__ == '__main__':
    unittest.main()
