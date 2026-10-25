"""Adversarial sealed fixture checks; no Java or Rust guest evaluation."""
import json
from pathlib import Path
import shutil
import tarfile
import tempfile
import unittest
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from string_base64_fixtures import DEFAULT, verify

class StringBase64FixtureTests(unittest.TestCase):
    def test_offline_archive_and_raw_protocol(self):
        result=verify()
        self.assertEqual(result['statuses'],{'success':67,'error':22})
        self.assertEqual(result['members'],3833)
        self.assertTrue(result['first_attempt_preserved_as_failure'])

    def test_java_utf8_and_decoder_boundaries_are_raw(self):
        with tarfile.open(DEFAULT/'oracle-evidence.tar.gz','r:gz') as stream:
            def raw(name): return stream.extractfile('capture-02/'+name).read()
            self.assertEqual(raw('repeat-0/007-encode-high/result.stdout.bin'),b'Pw==')
            self.assertEqual(raw('repeat-0/043-decode-invalid-utf8-ff/result.stdout.code-units.bin'),
                             '\ufffd'.encode('utf-16-be'))
            self.assertEqual(raw('repeat-0/055-decode-bytes-ff/result.stdout.bin'),b'Bytes(255)')
            first=json.loads(stream.extractfile('failed-launch-01/report.json').read())
            self.assertEqual(first['exit_code'],1)
            self.assertIsNone(first['audit'])

    def test_reject_undeclared_fixture_root_file(self):
        with tempfile.TemporaryDirectory() as temp:
            folder=Path(temp)/'fixture';shutil.copytree(DEFAULT,folder)
            (folder/'unexpected').write_bytes(b'x')
            with self.assertRaisesRegex(ValueError,'fixture root differs'):
                verify(folder)

    def test_reject_changed_authored_input(self):
        with tempfile.TemporaryDirectory() as temp:
            folder=Path(temp)/'fixture';shutil.copytree(DEFAULT,folder)
            (folder/'queries.tsv').write_bytes((folder/'queries.tsv').read_bytes()+b'\n')
            with self.assertRaisesRegex(ValueError,'fixture root manifest differs'):
                verify(folder)

if __name__=='__main__': unittest.main()
