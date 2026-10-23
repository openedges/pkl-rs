import unittest,json,shutil,tempfile,importlib.util
from pathlib import Path
TOOLS=Path(__file__).parents[1];P=TOOLS/'logical_capability_fixtures.py';spec=importlib.util.spec_from_file_location('check',P);check=importlib.util.module_from_spec(spec);spec.loader.exec_module(check);FIXTURE=TOOLS.parent/'tests/fixtures/m2-logical-capability'
class Integrity(unittest.TestCase):
 def setUp(self):self.temp=tempfile.TemporaryDirectory();self.root=Path(self.temp.name)/'fixture';shutil.copytree(FIXTURE,self.root)
 def tearDown(self):self.temp.cleanup()
 def replace(self,name,data):
  p=self.root/name;p.write_bytes(data);m=json.loads((self.root/'manifest.json').read_bytes());m['files'][name]={'sha256':check.sha(p),'bytes':p.stat().st_size};(self.root/'manifest.json').write_text(json.dumps(m)+'\n')
 def test_valid_fullraw_and_tsv(self):self.assertEqual(check.verify(self.root)['selectedJavaApiCalls'],36)
 def test_reblessed_tsv_count_rejected(self):
  p=self.root/'expected.tsv';self.replace('expected.tsv',p.read_bytes().replace(b'\t0\t0\n',b'\t0\t999\n',1))
  with self.assertRaisesRegex(ValueError,'TSV/raw-verified JSON'):check.verify(self.root)
 def test_reblessed_expected_count_rejected(self):
  p=json.loads((self.root/'fixtures.json').read_bytes());p['cases'][0]['expected'][0]['readsAfter']=999;self.replace('fixtures.json',(json.dumps(p)+'\n').encode())
  with self.assertRaisesRegex(ValueError,'raw expected outputs/counts'):check.verify(self.root)
 def test_reblessed_inventory_rejected(self):
  name='provenance/logical/inventory.json';p=json.loads((self.root/name).read_bytes());p['files'][0]['sha256']='0'*64;self.replace(name,(json.dumps(p)+'\n').encode());p=json.loads((self.root/'provenance.json').read_bytes())
  for row in p['originalPaths'].values():
   if row['path']==name:row.update(sha256=check.sha(self.root/name),bytes=(self.root/name).stat().st_size)
  self.replace('provenance.json',(json.dumps(p)+'\n').encode())
  with self.assertRaisesRegex(ValueError,'pinned Java report/inventory/closure'):check.verify(self.root)
 def test_reblessed_extra_file_rejected(self):
  self.replace('extra.bin',b'extra')
  with self.assertRaisesRegex(ValueError,'manifest/provenance coverage'):check.verify(self.root)
if __name__=='__main__':unittest.main()
