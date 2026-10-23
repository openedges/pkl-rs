import importlib.util,json,shutil,tempfile,unittest,hashlib
from pathlib import Path
spec=importlib.util.spec_from_file_location('fixtures',Path(__file__).parents[1]/'resource_oracle_fixtures.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
ORIGINAL=Path(__file__).parents[2]/'tests/fixtures/m2-resource-oracle'
class Integrity(unittest.TestCase):
 def setUp(self):self.temp=tempfile.TemporaryDirectory();self.root=Path(self.temp.name)/'fixture';shutil.copytree(ORIGINAL,self.root)
 def tearDown(self):self.temp.cleanup()
 def rows(self):return [line.split('\t')for line in(self.root/'expected.tsv').read_text().splitlines()]
 def save(self,rows):(self.root/'expected.tsv').write_text('\n'.join('\t'.join(r)for r in rows)+'\n')
 def reject(self):
  with self.assertRaises((ValueError,KeyError,StopIteration)):m.verify(self.root)
 def test_valid(self):self.assertEqual(m.verify(self.root)['selectedJavaApiCalls'],516)
 def test_missing_call(self):self.save(self.rows()[:-1]);self.reject()
 def test_reordered_calls(self):r=self.rows();r[0],r[1]=r[1],r[0];self.save(r);self.reject()
 def test_whitespace_payload_not_normalized(self):r=self.rows();r[0][4]+='20';self.save(r);self.reject()
 def test_utf16_units_independent(self):r=self.rows();r[0][6]+='0000';self.save(r);self.reject()
 def test_callback_counts_required(self):r=self.rows();r[0][9]='2';self.save(r);self.reject()
 def test_forged_source_manifest_hash(self):
  p=self.root/'manifest.json';d=json.loads(p.read_bytes());c=d['cases'][0];q=self.root/c['source'];q.write_bytes(q.read_bytes()+b'\n');c['sourceSha256']=hashlib.sha256(q.read_bytes()).hexdigest();p.write_text(json.dumps(d));self.reject()
 def test_payload_cannot_be_reblessed(self):
  p=self.root/'manifest.json';d=json.loads(p.read_bytes());r=d['cases'][1]['payloads'][9];q=self.root/r['path'];q.write_bytes(b'new');r['sha256']=hashlib.sha256(q.read_bytes()).hexdigest();r['bytes']=3;p.write_text(json.dumps(d));self.reject()
 def test_java_provenance_not_reserialized(self):
  p=self.root/'provenance/report.json';p.write_text(json.dumps(json.loads(p.read_bytes())));self.reject()
 def test_empty_selection_not_pass(self):
  p=self.root/'manifest.json';d=json.loads(p.read_bytes());d['cases']=[];p.write_text(json.dumps(d));self.reject()
if __name__=='__main__':unittest.main()
