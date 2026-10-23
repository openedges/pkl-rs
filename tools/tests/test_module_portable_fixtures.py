import unittest,tempfile,shutil,json,importlib.util,sys,subprocess
from pathlib import Path
from unittest.mock import patch
TOOLS=Path(__file__).parents[1];SCRIPT=TOOLS/'module_portable_fixtures.py';spec=importlib.util.spec_from_file_location('check',SCRIPT);check=importlib.util.module_from_spec(spec);spec.loader.exec_module(check);GOLDEN=TOOLS.parent/'tests/fixtures/m2-module-portable'
class Integrity(unittest.TestCase):
 def setUp(self):self.temp=tempfile.TemporaryDirectory();self.root=Path(self.temp.name)/'fixture';shutil.copytree(GOLDEN,self.root)
 def tearDown(self):self.temp.cleanup()
 def load(self,name):return json.loads((self.root/name).read_bytes())
 def save(self,name,value):
  path=self.root/name;path.write_text(json.dumps(value)+'\n');self.rebless(name)
 def rebless(self,name):
  p=self.root/name;m=self.load('manifest.json');m['files'][name]={'sha256':check.sha(p),'bytes':p.stat().st_size};(self.root/'manifest.json').write_text(json.dumps(m)+'\n')
 def mutate_raw(self,original,payload):
  p=self.load('provenance.json');record=p['originalPaths'][original];h=check.digest(payload);path='captured/'+h+'.bin';(self.root/path).write_bytes(payload);record.update(path=path,sha256=h,bytes=len(payload));self.rebless(path);self.save('provenance.json',p)
 def test_valid_full252_with_raw_event_hashes(self):self.assertEqual(check.verify(self.root)['selectedJavaApiCalls'],252)
 def test_cli_verifier(self):
  result=subprocess.run([sys.executable,'-B',str(SCRIPT),'--fixtures',str(self.root)],capture_output=True);self.assertEqual(result.returncode,0,result.stderr)
 def test_reblessed_provider_result_rejected(self):
  d=self.load('fixtures.json');d['fixtures'][0]['moduleProviders'][0]['result']['local']=False;self.save('fixtures.json',d)
  with self.assertRaisesRegex(ValueError,'fixture/provider/raw expectation'):check.verify(self.root)
 def test_reblessed_duplicate_request_key_rejected(self):
  d=self.load('fixtures.json');d['fixtures'][0]['moduleProviders'].append(d['fixtures'][0]['moduleProviders'][0]);self.save('fixtures.json',d)
  with self.assertRaisesRegex(ValueError,'fixture/provider/raw expectation'):check.verify(self.root)
 def test_reblessed_dropped_expected_observable_rejected(self):
  d=self.load('fixtures.json');d['fixtures'][0]['expected'][0]['observableEvents'].pop();self.save('fixtures.json',d)
  with self.assertRaisesRegex(ValueError,'fixture/provider/raw expectation'):check.verify(self.root)
 def test_reblessed_raw_counter_and_expected_rejected(self):
  d=self.load('fixtures.json');d['fixtures'][0]['expected'][0]['readsAfter']=999;self.save('fixtures.json',d);p=self.load('provenance.json');original=next(k for k in p['originalPaths']if '/file-tree-allow/0/calls/session-0/outer-0.reads-after.bin'in k);self.mutate_raw(original,b'999')
  with self.assertRaisesRegex(ValueError,'raw/pinned Java counters'):check.verify(self.root)
 def test_reblessed_actual_event_rejected_by_process_stdout(self):
  p=self.load('provenance.json');original=next(k for k in p['originalPaths']if '/file-tree-allow/0/calls/session-0/event-7.bin'in k);self.mutate_raw(original,b'policy-resolve-module file:///different-recorded-event')
  with self.assertRaisesRegex(ValueError,'original Java event order'):check.verify(self.root)
 def test_reblessed_report_rejected(self):
  p=self.load('provenance.json');original=next(k for k in p['originalPaths']if k.endswith('stage46-triple-dot-oracle-01/report.json'));record=p['originalPaths'][original];data=json.loads((self.root/record['path']).read_bytes());data['apiCalls']=999;self.mutate_raw(original,json.dumps(data).encode())
  with self.assertRaisesRegex(ValueError,'pinned raw/source bytes'):check.verify(self.root)
 def test_missing_original_raw_binding_rejected(self):
  p=self.load('provenance.json');original=next(k for k in p['originalPaths']if k.endswith('outer-0.status.bin'));p['originalPaths'].pop(original);self.save('provenance.json',p)
  with self.assertRaisesRegex(ValueError,'required raw/source provenance missing'):check.verify(self.root)
 def test_extra_manifest_regular_file_rejected(self):
  (self.root/'extra.bin').write_bytes(b'extra');self.rebless('extra.bin')
  with self.assertRaisesRegex(ValueError,'manifest semantic coverage'):check.verify(self.root)
 def test_forged_cache_path_rejected_before_walk(self):
  p=self.load('provenance.json');record=next(iter(p['originalPaths'].values()));record['path']='cargo-target/cache.bin';self.save('provenance.json',p)
  with patch.object(check.os,'scandir',side_effect=AssertionError('unauthenticated cachewalk')):
   with self.assertRaisesRegex(ValueError,'original/source pin mapping'):check.verify(self.root)
if __name__=='__main__':unittest.main()
