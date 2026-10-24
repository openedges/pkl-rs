"""Tamper checks for independent additive Java DataSize/Bytes authority."""
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
import data_size_supplement_fixtures as fixture

class FixtureTests(unittest.TestCase):
 def copy(self):
  temp=tempfile.TemporaryDirectory();self.addCleanup(temp.cleanup)
  root=Path(temp.name)/'fixture';shutil.copytree(fixture.DEFAULT,root)
  return root
 def rehash(self,root):
  files={p.relative_to(root).as_posix():fixture.digest(p) for p in root.rglob('*') if p.is_file() and p.name!='manifest.json'}
  (root/'manifest.json').write_text(json.dumps({'schema':1,'kind':fixture.KIND,'baseline':'Pkl 0.32.0','files':files}))
 def test_offline_split_identity_and_incomplete_history(self):
  with patch('subprocess.Popen',side_effect=AssertionError('offline verification started Java')):
   result=fixture.verify()
  self.assertEqual(result['kind'],fixture.KIND)
  self.assertEqual(result['common_string_cohorts']['05']['string_calls_per_session'],44)
  self.assertEqual(result['common_string_cohorts']['05']['export_calls_per_session'],49)
  self.assertEqual(result['public_identity_06']['public_export_calls_per_session'],16)
  self.assertIs(result['preserved_incomplete_history_04']['complete_acquisition'],False)
  self.assertEqual(result['candidate_compatibility_passes_claimed'],0)
 def test_rehashed_archive_fake_success_rejected_by_independent_pin(self):
  root=self.copy();p=root/'oracle-evidence.tar.gz';p.write_bytes(p.read_bytes()+b'fake success');self.rehash(root)
  with self.assertRaisesRegex(ValueError,'independent supplement archive SHA differs'):fixture.verify(root)
 def test_rehashed_source_query_input_drift_rejected(self):
  root=self.copy();p=root/'inputs/05/queries.tsv';p.write_bytes(p.read_bytes()+b'fake\t1.b\n');self.rehash(root)
  with self.assertRaisesRegex(ValueError,'authored input differs'):fixture.verify(root)
 def test_extra_fixture_channel_rejected(self):
  root=self.copy();(root/'candidate-pass.json').write_text('{"passes":44}')
  with self.assertRaisesRegex(ValueError,'manifest differs'):fixture.verify(root)
 def test_swapped_fixture_kind_rejected(self):
  root=self.copy();manifest=fixture.read(root/'manifest.json');manifest['kind']='pkl-java-data-size-split-v1';(root/'manifest.json').write_text(json.dumps(manifest))
  with self.assertRaisesRegex(ValueError,'manifest differs'):fixture.verify(root)
 def test_symlinked_input_cannot_substitute_payload(self):
  root=self.copy();path=root/'inputs/05/module.pkl';path.unlink();path.symlink_to(fixture.DEFAULT/'inputs/05/module.pkl')
  with self.assertRaisesRegex(ValueError,'symlink differs'):fixture.verify(root)

class RawTests(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory();cls.root=Path(cls.temp.name)
  with tarfile.open(fixture.DEFAULT/'oracle-evidence.tar.gz','r:gz') as stream:stream.extractall(cls.root,filter='data')
  cls.sources=cls.root/'sources'
 @classmethod
 def tearDownClass(cls):cls.temp.cleanup()
 def altered(self,path,raw,verify):
  original=path.read_bytes()
  try:
   path.write_bytes(raw)
   with self.assertRaises((ValueError,FileNotFoundError,UnicodeError)):verify()
  finally:path.write_bytes(original)
 def test_source_before_after_forgery_rejected(self):
  path=self.root/'capture-05/provenance.json';report=json.loads(path.read_bytes())
  key='external/pkl/stdlib/base.pkl';report['source_before'][key]=report['source_after'][key]='0'*64
  self.altered(path,json.dumps(report).encode(),lambda:fixture.verify_split(self.root/'capture-05','05',self.sources))
 def test_command_mutation_rejected(self):
  path=self.root/'capture-07/provenance.json';report=json.loads(path.read_bytes())
  report['sessions'][0]['command'][2]='-Xmx1g'
  self.altered(path,json.dumps(report).encode(),lambda:fixture.verify_split(self.root/'capture-07','07',self.sources))
 def test_missing_channel_rejected(self):
  path=self.root/'capture-05/string/repeat-0/000-round-call/result.stderr.bin';original=path.read_bytes()
  try:
   path.unlink()
   with self.assertRaises((ValueError,FileNotFoundError)):fixture.verify_split(self.root/'capture-05','05',self.sources)
  finally:path.write_bytes(original)
 def test_extra_channel_rejected(self):
  path=self.root/'capture-05/string/repeat-0/000-round-call/fake-success.bin'
  try:
   path.write_bytes(b'x')
   with self.assertRaises(ValueError):fixture.verify_split(self.root/'capture-05','05',self.sources)
  finally:path.unlink()
 def test_status_mismatch_rejected(self):
  path=self.root/'capture-05/string/repeat-0/000-round-call/result.status.bin'
  self.altered(path,b'success\n',lambda:fixture.verify_split(self.root/'capture-05','05',self.sources))
 def test_process_protocol_corruption_rejected(self):
  path=self.root/'capture-08/string/repeat-0/process.stdout.bin';original=path.read_bytes()
  self.altered(path,original.replace(b'call-enter=string-0-this-call-overflow',b'call-enter=string-0-fake',1),lambda:fixture.verify_split(self.root/'capture-08','08',self.sources))
 def test_utf16_corruption_rejected(self):
  path=self.root/'capture-09/string/repeat-0/000-lexical-inside/result.stdout.code-units.bin'
  self.altered(path,b'\x00z',lambda:fixture.verify_split(self.root/'capture-09','09',self.sources))
 def test_public_identity_producer_drift_rejected(self):
  path=self.root/'capture-06/repeat-0/000-bytes-empty/comparison.reference.bin'
  self.altered(path,b'false',lambda:fixture.verify_identity(self.root/'capture-06',self.sources))
 def test_swapped_histories_rejected(self):
  with self.assertRaises(ValueError):fixture.verify_split(self.root/'capture-08','07',self.sources)
 def test_incomplete_capture_cannot_be_relabelled_complete(self):
  path=self.root/'capture-04/provenance.json';report=json.loads(path.read_bytes());report['complete']=True
  self.altered(path,json.dumps(report).encode(),lambda:fixture.verify_split(self.root/'capture-04','04',self.sources))
 def test_forged_compatibility_pass_claim_rejected(self):
  path=self.root/'capture-05/audit.json';report=json.loads(path.read_bytes());report['compatibility_passes_claimed']=44
  self.altered(path,json.dumps(report).encode(),lambda:fixture.verify_split(self.root/'capture-05','05',self.sources))
 def test_optimized_python_rejects_corrupted_raw_status(self):
  path=self.root/'capture-05/string/repeat-0/000-round-call/result.status.bin';original=path.read_bytes()
  try:
   path.write_bytes(b'success\n')
   code='import sys;from pathlib import Path;sys.path.insert(0,sys.argv[1]);import data_size_supplement_fixtures as f;f.verify_split(Path(sys.argv[2]),"05",Path(sys.argv[3]))'
   process=subprocess.run([sys.executable,'-O','-c',code,str(Path(__file__).resolve().parents[1]),str(self.root/'capture-05'),str(self.sources)],capture_output=True,timeout=60)
   self.assertNotEqual(process.returncode,0)
   self.assertIn(b'ValueError',process.stderr)
  finally:path.write_bytes(original)

if __name__=='__main__':unittest.main()
