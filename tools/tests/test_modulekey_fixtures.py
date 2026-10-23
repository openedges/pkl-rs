from pathlib import Path
import hashlib, importlib.util, json, os, shutil, tempfile, unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[2]
O=ROOT/'tools'
spec=importlib.util.spec_from_file_location('subject',O/'modulekey_fixtures.py')
tool=importlib.util.module_from_spec(spec);spec.loader.exec_module(tool)
GOLDEN=Path(os.environ.get('MODULEKEY_GOLDEN',ROOT/'tests/fixtures/m2-modulekey-spi'))
class Integrity(unittest.TestCase):
    def setUp(self):
        private=ROOT/'target/modulekey-fixture-tests';private.mkdir(parents=True,exist_ok=True)
        self.temp=tempfile.TemporaryDirectory(dir=private)
        self.root=Path(self.temp.name)/'fixture'
        shutil.copytree(GOLDEN,self.root,copy_function=os.link)
    def tearDown(self):
        self.temp.cleanup()
    def replace(self,rel,b):
        p=self.root/rel;q=p.with_name(p.name+'.replacement');q.write_bytes(b);q.replace(p)
    def resign(self):
        m=json.loads((self.root/'manifest.json').read_bytes())
        for rel in m['files']:
            p=self.root/rel
            if p.is_file():
                b=p.read_bytes();m['files'][rel]={'sha256':hashlib.sha256(b).hexdigest(),'bytes':len(b)}
        self.replace('manifest.json',tool.dump(m))
    def test_verifier_reads_only_copied_payloads(self):
        read = Path.read_bytes
        root = self.root.absolute()
        def guarded(path):
            self.assertTrue(path.absolute().is_relative_to(root), 'verifier read outside copied fixture')
            return read(path)
        with patch.object(Path, 'read_bytes', guarded):
            result = tool.verify(self.root)
        self.assertEqual(result['javaApiCalls'], 212)
    def test_expected_counter_forgery_resigned(self):
        b=json.loads((self.root/'fixtures.json').read_bytes());b['fixtures'][0]['expected'][0]['countersAfter']['factory']+=1
        self.replace('fixtures.json',tool.dump(b));self.resign()
        with self.assertRaisesRegex(ValueError,'manifest exact reconstructed coverage|reconstructed fixture bytes'):tool.verify(self.root)
    def test_raw_event_forgery_resigned(self):
        b=json.loads((self.root/'fixtures.json').read_bytes());p='original/'+b['fixtures'][0]['orderedEvents'][0]['rawPrefix']+'.bin'
        self.replace(p,b'forged');self.resign()
        with self.assertRaisesRegex(ValueError,'pinned original inventory'):tool.verify(self.root)
    def test_inventory_rewrite_rejected(self):
        p='provenance/inventory.json';b=json.loads((self.root/p).read_bytes());next(iter(b['files'].values()))['sha256']='0'*64
        self.replace(p,tool.dump(b));self.resign()
        with self.assertRaisesRegex(ValueError,'literal original'):tool.verify(self.root)
    def test_provenance_symlink_rejected(self):
        p=self.root/'provenance/report.json';p.unlink();p.symlink_to(GOLDEN/'provenance/report.json')
        with self.assertRaisesRegex(ValueError,'regular payload leaf'):tool.verify(self.root)
    def test_unknown_directory_rejected_before_descent(self):
        bad=self.root/'private-unlisted';bad.mkdir()
        real=os.scandir
        def guarded(p):
            self.assertNotEqual(Path(p),bad,'verifier descended unknown directory')
            return real(p)
        with patch('os.scandir',guarded):
            with self.assertRaisesRegex(ValueError,'before descent'):tool.verify(self.root)
    def test_missing_raw_rejected(self):
        b=json.loads((self.root/'fixtures.json').read_bytes());p=self.root/(b['fixtures'][0]['expected'][0]['javaRawPrefix']+'.stdout.bin');p.unlink()
        with self.assertRaises(FileNotFoundError):tool.verify(self.root)
    def test_extra_regular_payload_rejected(self):
        (self.root/'extra.bin').write_bytes(b'x')
        with self.assertRaisesRegex(ValueError,'exact actual payload coverage'):tool.verify(self.root)
if __name__=='__main__':unittest.main()
