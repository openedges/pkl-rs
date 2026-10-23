"""Real reblessed fixture tampering checks; never modifies the golden inputs."""
import sys
sys.dont_write_bytecode = True
from pathlib import Path
import copy
import hashlib
import json
import os
import shutil
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).parent if (Path(__file__).parent / 'renderer_fixtures.py').is_file() else Path(__file__).resolve().parents[1]))
from unittest import mock
import renderer_fixtures as tool

GOLDEN = Path(os.environ.get('RENDERER_PORTABLE_GOLDEN', str(Path(tool.__file__).resolve().parent.parent / 'tests/fixtures/m2-renderer-portable')))

class FixtureVerificationTests(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temporary = tempfile.TemporaryDirectory(prefix='renderer-negative-', dir=Path(__file__).parent)
  cls.root = Path(cls.temporary.name) / 'fixture'
  shutil.copytree(GOLDEN, cls.root, copy_function=os.link)
  cls.golden_files = {rel: hashlib.sha256((GOLDEN / rel).read_bytes()).hexdigest() for rel in tool.file_set(GOLDEN)}

 @classmethod
 def tearDownClass(cls):
  after = {rel: hashlib.sha256((GOLDEN / rel).read_bytes()).hexdigest() for rel in tool.file_set(GOLDEN)}
  cls.temporary.cleanup()
  if after != cls.golden_files:
   raise AssertionError('golden pathset or payload changed')

 def setUp(self):
  self.touched = set()

 def tearDown(self):
  for relative in self.touched:
   path = self.root / relative
   if path.exists() or path.is_symlink():
    path.unlink()
   original = GOLDEN / relative
   if original.is_file():
    os.link(original, path)

 def put(self, relative, data):
  self.touched.add(relative)
  path = self.root / relative
  if path.exists() or path.is_symlink():
   path.unlink()
  path.parent.mkdir(parents=True, exist_ok=True)
  path.write_bytes(data)

 def obj(self, relative):
  return json.loads((self.root / relative).read_bytes())

 def put_json(self, relative, value):
  self.put(relative, (json.dumps(value, indent=2) + '\n').encode())

 def bless(self):
  manifest = self.obj('manifest.json')
  manifest['files'] = {rel: {'sha256': tool.sha((self.root / rel).read_bytes()), 'bytes': (self.root / rel).stat().st_size} for rel in tool.file_set(self.root) if rel != 'manifest.json'}
  self.put_json('manifest.json', manifest)

 def reject(self):
  self.bless()
  with self.assertRaises((ValueError, KeyError, FileNotFoundError)):
   tool.verify(self.root)

 def fixture_mutation(self, mutate):
  bundle = self.obj('fixtures.json')
  mutate(bundle)
  self.put_json('fixtures.json', bundle)
  self.reject()

 def raw_path(self, suffix):
  return next(rel for rel in tool.file_set(self.root) if '/calls/session-' in rel and rel.endswith(suffix))

 def test_pins_authenticate_before_walk(self):
  relative = 'provenance/stage47-renderer-api-oracle-02/report.json'
  self.put(relative, b'{}')
  with mock.patch.object(tool, 'file_set', side_effect=AssertionError('walk before authentication')):
   with self.assertRaises(ValueError):
    tool.verify(self.root)

 def test_valid_full_fixture(self):
  self.assertEqual(tool.verify(self.root)['javaApiCalls'], 860)

 def test_undeclared_directory_rejected_before_descent(self):
  unexpected = self.root / 'undeclared-cache'
  (unexpected / 'nested').mkdir(parents=True)
  (unexpected / 'nested' / 'payload').write_bytes(b'must not be traversed')
  visited = []
  original_walk = os.walk
  def observed_walk(*args, **kwargs):
   for entry in original_walk(*args, **kwargs):
    visited.append(Path(entry[0]))
    yield entry
  try:
   with mock.patch.object(tool.os, 'walk', side_effect=observed_walk):
    with self.assertRaisesRegex(ValueError, 'undeclared fixture directory'):
     tool.verify(self.root)
   self.assertTrue(visited)
   self.assertFalse(any(path == unexpected or unexpected in path.parents for path in visited))
  finally:
   shutil.rmtree(unexpected)

 def test_undeclared_empty_directory_rejected(self):
  unexpected = self.root / 'undeclared-empty'
  unexpected.mkdir()
  try:
   with self.assertRaisesRegex(ValueError, 'undeclared fixture directory'):
    tool.verify(self.root)
  finally:
   unexpected.rmdir()

 def test_root_symlink_rejected_before_read(self):
  link = self.root.parent / 'fixture-link'
  link.symlink_to(self.root, target_is_directory=True)
  try:
   with mock.patch.object(Path, 'read_bytes', side_effect=AssertionError('read through root link')):
    with self.assertRaisesRegex(ValueError, 'fixture root'):
     tool.verify(link)
  finally:
   link.unlink()

 def test_manifest_symlink_rejected_before_read(self):
  self.touched.add('manifest.json')
  manifest = self.root / 'manifest.json'
  manifest.unlink()
  manifest.symlink_to(GOLDEN / 'manifest.json')
  with mock.patch.object(Path, 'read_bytes', side_effect=AssertionError('read through leaf link')):
   with self.assertRaisesRegex(ValueError, 'regular file'):
    tool.verify(self.root)

 def test_parent_symlink_rejected_before_read(self):
  parent = self.root / 'provenance' / 'stage47-renderer-api-oracle-02'
  backup = parent.with_name(parent.name + '-saved')
  parent.rename(backup)
  parent.symlink_to(backup, target_is_directory=True)
  original_read = Path.read_bytes
  def observed_read(path):
   if parent in path.parents:
    raise AssertionError('read through parent link')
   return original_read(path)
  try:
   with mock.patch.object(Path, 'read_bytes', autospec=True, side_effect=observed_read):
    with self.assertRaisesRegex(ValueError, 'fixture parent'):
     tool.verify(self.root)
  finally:
   parent.unlink()
   backup.rename(parent)

 @unittest.skipUnless(hasattr(os, 'mkfifo'), 'FIFO is unavailable on this platform')
 def test_fifo_rejected_before_read(self):
  self.touched.add('manifest.json')
  manifest = self.root / 'manifest.json'
  manifest.unlink()
  os.mkfifo(manifest)
  with mock.patch.object(Path, 'read_bytes', side_effect=AssertionError('attempted FIFO read')):
   with self.assertRaisesRegex(ValueError, 'regular file'):
    tool.verify(self.root)

 def test_reblessed_expected_counter(self):
  self.fixture_mutation(lambda b: b['fixtures'][0]['expected'][0].update(readsAfter=999))

 def test_reblessed_raw_counter_and_expected(self):
  rel = self.raw_path('.reads-after.bin')
  self.put(rel, b'999')
  bundle = self.obj('fixtures.json')
  bundle['fixtures'][0]['expected'][0]['readsAfter'] = 999
  self.put_json('fixtures.json', bundle)
  self.reject()

 def test_reblessed_pinned_report_counters(self):
  rel = 'provenance/stage47-renderer-api-oracle-02/report.json'
  report = self.obj(rel)
  report['observations'][0]['calls'][0]['countersAfter'] = 'read=999\n'
  self.put_json(rel, report)
  self.reject()

 def test_reblessed_inventory_and_closure(self):
  base = 'provenance/stage47-renderer-api-oracle-02/'
  inv = self.obj(base + 'inventory.json')
  inv['files'][0]['sha256'] = '0' * 64
  self.put_json(base + 'inventory.json', inv)
  closure = self.obj(base + 'closure.json')
  closure['inventorySha256'] = tool.sha((self.root / (base + 'inventory.json')).read_bytes())
  self.put_json(base + 'closure.json', closure)
  self.reject()

 def test_reblessed_raw_utf16(self):
  rel = self.raw_path('.stdout.code-units.bin')
  self.put(rel, b'\x00X')
  self.reject()

 def test_reblessed_raw_utf8(self):
  rel = self.raw_path('.stdout.bin')
  self.put(rel, b'changed')
  self.reject()

 def test_status_whitespace_is_not_normalized(self):
  rel = self.raw_path('.status.bin')
  self.put(rel, b'success \n')
  self.reject()

 def test_reblessed_helper_source(self):
  rel = 'provenance/stage47-renderer-api-oracle-02/RendererProbe.java'
  self.put(rel, (self.root / rel).read_bytes() + b'\n// changed producer\n')
  self.reject()

 def test_reblessed_input_source(self):
  rel = next(rel for rel in tool.file_set(self.root) if rel.endswith('/main.pkl'))
  self.put(rel, b'stable = 999\n')
  self.reject()

 def test_reblessed_reader_return(self):
  rel = self.raw_path('.value.code-units.bin')
  self.put(rel, b'\x00Z')
  self.reject()

 def test_reblessed_reader_plan(self):
  def mutate(bundle):
   fixture = next(row for row in bundle['fixtures'] if row['readerPlan'])
   fixture['readerPlan'][0]['uri'] = 'res:/forged'
  self.fixture_mutation(mutate)

 def test_reblessed_expected_java_utf8(self):
  self.fixture_mutation(lambda b: b['fixtures'][0]['expected'][0].update(stdoutHex='ff'))

 def test_reblessed_expected_utf16(self):
  self.fixture_mutation(lambda b: b['fixtures'][0]['expected'][0].update(stdoutUnits=[65533]))

 def test_reblessed_expression_order(self):
  self.fixture_mutation(lambda b: b['fixtures'][0]['expressions'].reverse())

 def test_actual_extra_even_when_outer_manifest_updated(self):
  self.put('surprise.bin', b'not selected')
  self.reject()

 def test_actual_missing_even_when_outer_manifest_updated(self):
  rel = self.raw_path('.expression.bin')
  self.touched.add(rel)
  (self.root / rel).unlink()
  self.reject()

 def test_manifest_omitted_existing_file(self):
  manifest = self.obj('manifest.json')
  del manifest['files'][self.raw_path('.expression.bin')]
  self.put_json('manifest.json', manifest)
  with self.assertRaises(ValueError):
   tool.verify(self.root)

 def test_provenance_original_path_rewrite(self):
  manifest = self.obj('manifest.json')
  next(iter(manifest['sourceProvenance'].values()))['originalRelativePath'] = 'forged'
  self.put_json('manifest.json', manifest)
  with self.assertRaises(ValueError):
   tool.verify(self.root)

 def test_symlink_rejected(self):
  rel = self.raw_path('.expression.bin')
  self.touched.add(rel)
  (self.root / rel).unlink()
  (self.root / rel).symlink_to('/dev/null')
  with self.assertRaises(ValueError):
   tool.verify(self.root)

if __name__ == '__main__':
 unittest.main()
