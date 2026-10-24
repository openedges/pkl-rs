"""Tamper checks for actual split Java DataSize evidence; never runs Java."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import data_size_fixtures as fixtures
import data_size_oracle as oracle

class FixtureIdentityTests(unittest.TestCase):
    def test_offline_authority_and_separate_boundaries(self):
        with patch('subprocess.Popen',side_effect=AssertionError('offline verifier launched Java')):
            result=fixtures.verify()
        self.assertEqual(result['canonical']['string_calls_per_session'],255)
        self.assertEqual(result['canonical']['export_calls_per_session'],389)
        self.assertEqual(result['canonical']['intentional_error_rows'],22)
        self.assertEqual(result['compatibility_passes_claimed'],0)
        self.assertIn('required',result['canonical']['public_export_boundary'])

    def test_root_query_drift_and_manifest_refresh_still_fail_fixed_input_pin(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);shutil.copytree(fixtures.DEFAULT,root/'fixture')
            q=root/'fixture/queries.tsv';q.write_bytes(q.read_bytes()+b'fake\t1.b\n')
            manifest=fixtures.read(root/'fixture/manifest.json')
            manifest['files']['queries.tsv']=fixtures.sha(q.read_bytes())
            (root/'fixture/manifest.json').write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError,'canonical input differs'): fixtures.verify(root/'fixture')

    def test_archive_bytes_and_fake_success_claim_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);shutil.copytree(fixtures.DEFAULT,root/'fixture')
            archive=root/'fixture/oracle-evidence.tar.gz';archive.write_bytes(archive.read_bytes()+b'fake-success')
            manifest=fixtures.read(root/'fixture/manifest.json');manifest['files']['oracle-evidence.tar.gz']=fixtures.sha(archive.read_bytes())
            (root/'fixture/manifest.json').write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError,'independent archive SHA differs'): fixtures.verify(root/'fixture')

    def test_extra_root_file_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);shutil.copytree(fixtures.DEFAULT,root/'fixture')
            (root/'fixture/fake-pass.json').write_text('{"passes":255}')
            with self.assertRaisesRegex(ValueError,'fixture manifest/files differ'): fixtures.verify(root/'fixture')

class RawProtocolTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp=tempfile.TemporaryDirectory();cls.root=Path(cls.temp.name)
        with tarfile.open(fixtures.DEFAULT/'oracle-evidence.tar.gz','r:gz') as stream:
            stream.extractall(cls.root,filter='data')
        cls.capture=cls.root/'capture-03';cls.sources=cls.root/'sources'

    @classmethod
    def tearDownClass(cls): cls.temp.cleanup()

    def altered(self,path,raw,pattern):
        original=path.read_bytes()
        try:
            path.write_bytes(raw)
            with self.assertRaises((AssertionError,FileNotFoundError,UnicodeError,ValueError)) as raised:
                oracle.audit_split(self.capture,self.sources)
            self.assertIn(pattern,str(raised.exception) or pattern)
        finally: path.write_bytes(original)

    def test_missing_call_channel(self):
        path=self.capture/'string/repeat-0/000-int-b/result.stdout.bin'
        original=path.read_bytes()
        try:
            path.unlink()
            with self.assertRaises((ValueError,FileNotFoundError)):oracle.audit_split(self.capture,self.sources)
        finally:path.write_bytes(original)

    def test_extra_call_channel(self):
        path=self.capture/'string/repeat-0/000-int-b/fake-success.bin'
        try:
            path.write_bytes(b'fake')
            with self.assertRaises(ValueError):oracle.audit_split(self.capture,self.sources)
        finally:path.unlink()

    def test_status_mismatch_and_raw_repetition(self):
        self.altered(self.capture/'string/repeat-0/000-int-b/result.status.bin',b'error\n','')

    def test_protocol_corruption(self):
        path=self.capture/'string/repeat-0/process.stdout.bin'
        self.altered(path,path.read_bytes().replace(b'call-enter=string-0-int-b',b'call-enter=string-0-fake',1),'')

    def test_utf16_corruption(self):
        path=self.capture/'string/repeat-0/000-int-b/result.stdout.code-units.bin'
        self.altered(path,b'\x00z','')

    def test_export_producer_drift(self):
        path=self.capture/'export/repeat-0/000-int-b/call-0.raw-bits-hex.bin'
        self.altered(path,b'0000000000000000','')

    def test_swapped_history_rejected(self):
        with self.assertRaisesRegex(ValueError,'historical row count changed'):
            fixtures.verify_history(self.root/'capture-02','01',self.sources)

    def test_resigned_source_before_after_cannot_change_snapshot(self):
        path=self.capture/'provenance.json';original=path.read_bytes()
        try:
            report=json.loads(original)
            name='external/pkl/stdlib/base.pkl'
            report['source_before'][name]=report['source_after'][name]='0'*64
            path.write_text(json.dumps(report))
            with self.assertRaises(ValueError):oracle.audit_split(self.capture,self.sources)
        finally:path.write_bytes(original)

    def test_resigned_command_cannot_change_actual_public_call(self):
        path=self.capture/'provenance.json';original=path.read_bytes()
        try:
            report=json.loads(original)
            report['sessions'][0]['command'][2]='-Xmx1g'
            path.write_text(json.dumps(report))
            with self.assertRaises(ValueError):oracle.audit_split(self.capture,self.sources)
        finally:path.write_bytes(original)

    def test_forged_success_audit_rejected_after_raw_reconstruction(self):
        path=self.capture/'audit.json';original=path.read_bytes()
        try:
            report=json.loads(original)
            report['compatibility_passes_claimed']=255
            path.write_text(json.dumps(report))
            with self.assertRaises(ValueError):oracle.audit_split(self.capture,self.sources)
        finally:path.write_bytes(original)

    def test_optimized_python_rejects_corrupted_capture(self):
        path=self.capture/'provenance.json';original=path.read_bytes()
        try:
            report=json.loads(original)
            name='external/pkl/stdlib/base.pkl'
            report['source_before'][name]=report['source_after'][name]='0'*64
            path.write_text(json.dumps(report))
            code='import sys; sys.path.insert(0,sys.argv[1]); import data_size_oracle as d; d.audit_split(sys.argv[2],sys.argv[3])'
            result=subprocess.run([sys.executable,'-O','-c',code,str(Path(__file__).resolve().parents[1]),str(self.capture),str(self.sources)],capture_output=True,timeout=60)
            self.assertNotEqual(result.returncode,0)
            self.assertIn(b'Java DataSize audit check',result.stderr)
        finally:path.write_bytes(original)

    def test_repeated_raw_cross_session_is_exact(self):
        result=oracle.audit_split(self.capture,self.sources)
        self.assertEqual([s['public_evaluator_calls'] for s in result['sessions']],[255,255,389,389])
        self.assertEqual([s['raw_files'] for s in result['sessions']],[2297,2297,8975,8975])
        self.assertEqual(result['raw_repetition_equal'],{'string':True,'export':True})

if __name__=='__main__':unittest.main()
