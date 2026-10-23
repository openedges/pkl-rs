import importlib.util,unittest,subprocess,sys,json,os,tempfile,shutil,hashlib
from pathlib import Path
TOOLS=Path(__file__).parents[1]
def load(name):
 spec=importlib.util.spec_from_file_location(name,TOOLS/(name+'.py'));m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
export=load('export_resource_portable');check=load('resource_fixtures')
GOLDEN=Path(os.environ.get('RESOURCE_PORTABLE_GOLDEN',str(TOOLS.parent/'tests/fixtures/m2-resource-portable')))
class Contract(unittest.TestCase):
 def test_exact_success_status(self):self.assertEqual(export.parse_status(b'success\n'),'success')
 def test_exact_error_status(self):self.assertEqual(export.parse_status(b'error\n'),'error')
 def test_no_status_trim(self):
  for raw in [b'success',b' success\n',b'success\n\n',b'error\r\n',b'unsupported\n']:
   with self.assertRaises(ValueError):export.parse_status(raw)
 def test_java_unpaired_high_surrogate(self):self.assertEqual(export.java_utf8([0xD800]),b'?')
 def test_java_unpaired_low_surrogate(self):self.assertEqual(export.java_utf8([0xDC00]),b'?')
 def test_java_surrogate_pair(self):self.assertEqual(export.java_utf8([0xD83D,0xDE00]),b'\xf0\x9f\x98\x80')
 def test_invalid_unit_not_wrapped(self):
  with self.assertRaises(ValueError):export.java_utf8([65536])
 def test_half_utf16_byte_rejected(self):
  with self.assertRaises(ValueError):export.units(b'\x00')
 def test_missing_input_rule(self):self.assertEqual(check.derive('resource',{'workspace':'.'},dict(uri='res:/missing',attempt=1,total=2),Path('.'),{}),{'kind':'missing'})
 def test_typed_io_first_attempt(self):self.assertEqual(check.derive('resource',{'workspace':'.'},dict(uri='res:/retry',attempt=1,total=3),Path('.'),{}),{'kind':'io','message':'reader-first'})
 def test_authority_returns_actual_spelling(self):self.assertEqual(check.derive('authority',{'workspace':'.'},dict(uri='mem://HOST:080/item',attempt=1,total=1),Path('.'),{}),{'kind':'resource','resourceUri':'mem://HOST:080/item','bytes':list(b'payload-1')})
 def test_unknown_reader_not_defaulted(self):
  with self.assertRaises(ValueError):check.derive('resource',{'workspace':'.'},dict(uri='other:/unexpected',attempt=1,total=1),Path('.'),{})
 def test_portable_full_fixture_cli(self):
  result=subprocess.run([sys.executable,str(TOOLS/'resource_fixtures.py'),'--fixtures',str(GOLDEN)],capture_output=True);self.assertEqual(result.returncode,0,result.stderr)
class ReblessedForgeries(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory();cls.root=Path(cls.temp.name)/'fixture';shutil.copytree(GOLDEN,cls.root,copy_function=os.link)
  cls.original_manifest=(GOLDEN/'manifest.json').read_bytes();cls.original_hashes=json.loads(cls.original_manifest)['files'];cls.baseline_provenance=json.loads((GOLDEN/'provenance.json').read_bytes())
 @classmethod
 def tearDownClass(cls):
  if (GOLDEN/'manifest.json').read_bytes()!=cls.original_manifest:raise AssertionError('original golden manifest changed')
  for name,record in cls.original_hashes.items():
   if check.sha(GOLDEN/name)!=record['sha256']:raise AssertionError('original golden payload changed: '+name)
  cls.temp.cleanup()
 def setUp(self):self.changed=set();self.added_dirs=[]
 def replace(self,name,data):
  path=self.root/name;path.parent.mkdir(parents=True,exist_ok=True);tmp=path.with_name(path.name+'.replacement');tmp.write_bytes(data);os.replace(tmp,path);self.changed.add(name)
 def json(self,name):return json.loads((self.root/name).read_bytes())
 def write_json(self,name,value):self.replace(name,(json.dumps(value,indent=2)+'\n').encode())
 def rebless(self):
  manifest=self.json('manifest.json')
  for name in self.changed-{'manifest.json'}:
   path=self.root/name
   if path.exists():manifest['files'][name]={'sha256':check.sha(path),'bytes':path.stat().st_size}
   else:manifest['files'].pop(name,None)
  self.write_json('manifest.json',manifest)
 def mutate_bound(self,name,data):
  self.replace(name,data);p=self.json('provenance.json')
  for record in p['originalPaths'].values():
   if record['path']==name:record.update(sha256=check.sha(self.root/name),bytes=len(data))
  self.write_json('provenance.json',p)
 def rejected(self,pattern):
  self.rebless()
  with self.assertRaisesRegex(ValueError,pattern):check.verify(self.root)
 def tearDown(self):
  for name in self.changed:
   path=self.root/name
   if path.exists():path.unlink()
   if (GOLDEN/name).exists():os.link(GOLDEN/name,path)
  for directory in reversed(self.added_dirs):
   path=self.root/directory
   if path.exists():shutil.rmtree(path)
 def test_forged_expected_read_count_reblessed(self):
  d=self.json('fixtures.json');d['fixtures'][0]['expected'][0]['readsAfter']=999;self.write_json('fixtures.json',d);self.rejected('expected raw outputs/counts')
 def test_forged_raw_count_expected_and_binding_reblessed(self):
  d=self.json('fixtures.json');d['fixtures'][0]['expected'][0]['readsAfter']=999;self.write_json('fixtures.json',d);name=self.baseline_provenance['sessions'][0]['selectedRawJava'][0]['rawLocalPrefix']+'.reads-after.bin';self.mutate_bound(name,b'999');self.rejected('inventory binding')
 def test_forged_raw_counters_and_binding_reblessed(self):
  name=self.baseline_provenance['sessions'][0]['selectedRawJava'][0]['rawLocalPrefix']+'.counters-after.bin';self.mutate_bound(name,(self.root/name).read_bytes().replace(b'read=1\n',b'read=999\n'));self.rejected('inventory binding')
 def test_forged_report_counters_reblessed(self):
  name='provenance/resource/report.json';r=self.json(name);r['observations'][0]['calls'][0]['countersAfter']=r['observations'][0]['calls'][0]['countersAfter'].replace('read=1\n','read=999\n');self.mutate_bound(name,(json.dumps(r)+'\n').encode());self.rejected('report pin')
 def test_forged_inventory_raw_hash_reblessed(self):
  name='provenance/resource/inventory.json';v=self.json(name);v['files'][0]['sha256']='0'*64;self.mutate_bound(name,(json.dumps(v)+'\n').encode());self.rejected('inventory/closure pin')
 def test_removed_inventory_raw_entry_reblessed(self):
  name='provenance/resource/inventory.json';v=self.json(name);v['files'].pop();self.mutate_bound(name,(json.dumps(v)+'\n').encode());self.rejected('inventory/closure pin')
 def test_forged_provenance_raw_call_identity_reblessed(self):
  p=self.json('provenance.json');p['sessions'][0]['selectedRawJava'][0]=p['sessions'][0]['selectedRawJava'][1];self.write_json('provenance.json',p);self.rejected('raw call provenance')
 def test_forged_provenance_reader_session_reblessed(self):
  p=self.json('provenance.json');p['sessions'][0]['readerTrace'][0]['rawPath']=p['sessions'][1]['readerTrace'][0]['rawPath'];self.write_json('provenance.json',p);self.rejected('reader event original path')
 def test_added_manifest_listed_file_reblessed(self):
  self.replace('unexpected.bin',b'extra');self.rejected('manifest/provenance file coverage')
 def test_added_unlisted_regular_file(self):
  self.replace('unexpected.bin',b'extra')
  with self.assertRaisesRegex(ValueError,'actual regular-file coverage'):check.verify(self.root)
 def test_missing_regular_file(self):
  name=self.baseline_provenance['sessions'][0]['selectedRawJava'][0]['rawLocalPrefix']+'.stdout.bin';(self.root/name).unlink();self.changed.add(name)
  with self.assertRaisesRegex(ValueError,'actual regular-file coverage'):check.verify(self.root)
 def test_removed_raw_binding_manifest_and_file_reblessed(self):
  p=self.json('provenance.json');original=next(k for k,v in p['originalPaths'].items()if v['kind']=='Java API raw');name=p['originalPaths'].pop(original)['path'];self.write_json('provenance.json',p);(self.root/name).unlink();self.changed.add(name);self.rejected('original raw/input path coverage')
 def test_forged_cache_path_rejected_before_walk(self):
  from unittest.mock import patch
  p=self.json('provenance.json');original=next(k for k,v in p['originalPaths'].items()if v['kind']=='Java API raw');record=p['originalPaths'][original];old=record['path'];record['path']='cargo-target/copied.bin';self.write_json('provenance.json',p)
  directory=self.root/'cargo-target';directory.mkdir();self.added_dirs.append('cargo-target');self.replace('cargo-target/copied.bin',(self.root/old).read_bytes());(self.root/old).unlink();self.changed.add(old);self.rebless()
  def no_walk(*args,**kwargs):raise AssertionError('unauthenticated layout was walked')
  with patch.object(check.os,'scandir',side_effect=no_walk):
   with self.assertRaisesRegex(ValueError,'canonical original/local path'):check.verify(self.root)
 def test_unexpected_cache_directory_rejected_before_descent(self):
  directory=self.root/'cargo-target';directory.mkdir();self.added_dirs.append('cargo-target');(directory/'sentinel').write_bytes(b'never read')
  with self.assertRaisesRegex(ValueError,'unexpected fixture directory'):check.verify(self.root)
if __name__=='__main__':unittest.main()
