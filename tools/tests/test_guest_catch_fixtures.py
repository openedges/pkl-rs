import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
import hashlib

TOOLS=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(TOOLS))
import guest_catch_fixtures as fixtures
SOURCE=TOOLS.parent/'tests/fixtures/m2-guest-catch'

def refresh(root):
    files={}
    for path in root.rglob('*'):
        if path.is_file()and path.name!='manifest.json':
            data=path.read_bytes();files[str(path.relative_to(root))]={'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
    (root/'manifest.json').write_text(json.dumps({'schema':1,'files':files}))

class GuestCatchFixtureTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name)/'fixtures';shutil.copytree(SOURCE,self.root)
    def test_complete_initial_and_corrected_scopes_are_distinct(self):
        result=fixtures.verify(self.root)
        self.assertEqual(result['cases'],21)
        self.assertEqual(result['orderedOuterCalls'],244)
        self.assertEqual(result['actualCallbacks'],14)
        self.assertEqual(result['scopeCounts'],{'full-ordered-api':12,'frontend-constructor-only':5,'terminal-partial-parity':4})
    def test_self_consistent_uri_and_missing_binding_forgery_rejected(self):
        selection=json.loads((self.root/'selection.json').read_text());row=selection['cases'][0];row['moduleUri']='file:///forged-guest-catch.pkl'
        uri=self.root/row['sourceFile'];uri.with_name('uri.txt').write_text(row['moduleUri'])
        provenance=json.loads((self.root/'provenance.json').read_text());provenance['bindings']=[r for r in provenance['bindings']if r['fixturePath']!='initial/cases.json']
        (self.root/'selection.json').write_text(json.dumps(selection));(self.root/'provenance.json').write_text(json.dumps(provenance));refresh(self.root)
        with self.assertRaises(ValueError):fixtures.verify(self.root)
    def test_equal_utf8_distinct_surrogate_unit_forgery_rejected(self):
        path=self.root/'initial/cases/renderer-profile-lines-strip-utf16/0/calls/outer-1.stdout.code-units.bin';data=path.read_bytes();self.assertIn(b'\xd8\x3d',data);changed=data.replace(b'\xd8\x3d',b'\xdc\x00',1)
        self.assertEqual(data.decode('utf-16-be','surrogatepass').encode('utf-8','replace'),changed.decode('utf-16-be','surrogatepass').encode('utf-8','replace'))
        path.write_bytes(changed);refresh(self.root)
        with self.assertRaises(ValueError):fixtures.verify(self.root)
    def test_initial_force_boundary_cannot_be_promoted(self):
        selection=json.loads((self.root/'selection.json').read_text())
        row=next(r for r in selection['cases'] if r['group']=='initial-force')
        row['comparisonScope']='full-ordered-api'
        (self.root/'selection.json').write_text(json.dumps(selection));refresh(self.root)
        with self.assertRaises(ValueError):fixtures.verify(self.root)
    def test_new_corrected_force_payload_forgery_rejected(self):
        path=self.root/'corrected-force/cases/nearest-guest-handler-inside-allow-undefined-force/1/calls/outer-3.stdout.bin'
        path.write_bytes(path.read_bytes()+b' ');refresh(self.root)
        with self.assertRaises(ValueError):fixtures.verify(self.root)
    def test_boundary_promotion_and_dropped_late_call_rejected(self):
        selection=json.loads((self.root/'selection.json').read_text());selection['cases'][0]['comparisonScope']='full-ordered-api';(self.root/'selection.json').write_text(json.dumps(selection));refresh(self.root)
        with self.assertRaises(ValueError):fixtures.verify(self.root)
        shutil.rmtree(self.root);shutil.copytree(SOURCE,self.root)
        path=self.root/'corrected/cases/stdlib-singleton-and-builtin-identity/1/calls/outer-9.stdout.bin';path.unlink();refresh(self.root)
        with self.assertRaises((ValueError,FileNotFoundError)):fixtures.verify(self.root)

if __name__=='__main__':unittest.main()
